//! Closed machine settings adapter for the OS-started Settings instance.

use std::process::Stdio;
use std::time::Duration;

use serde_json::Value;
use tokio::io::AsyncWriteExt;

use crate::yuosd::protocol::BrokerError;
use crate::yuosd::wire::requests::{SystemSettings, SystemSettingsAction};

static CHANGE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub(super) async fn request(params: Value, owner: u32) -> Result<Value, BrokerError> {
    let request: SystemSettings = serde_json::from_value(params)
        .map_err(|_| BrokerError::execution("Invalid system settings action"))?;
    let action = request.action;
    if matches!(
        action,
        SystemSettingsAction::DecisionStatus
            | SystemSettingsAction::DecisionApply
            | SystemSettingsAction::DecisionTest
            | SystemSettingsAction::DecisionReset
    ) {
        if request.account.is_some()
            || request.decision_model.is_some() != matches!(action, SystemSettingsAction::DecisionApply)
        {
            return Err(BrokerError::execution("Invalid decision model settings request"));
        }
        return decision_settings(action, request.decision_model.as_ref().map(|model| model.as_str()), owner).await;
    }
    let has_account = matches!(
        action,
        SystemSettingsAction::CreateUser | SystemSettingsAction::SetUserPassword
    );
    if request.account.is_some() != has_account || request.decision_model.is_some() {
        return Err(BrokerError::execution("Invalid account settings request"));
    }
    let _identity = if has_account {
        Some(crate::yuosd::users::lock_mutations().await?)
    } else {
        None
    };
    let _change = if matches!(action, SystemSettingsAction::Status) {
        None
    } else {
        Some(CHANGE.try_lock().map_err(|_| {
            BrokerError::unavailable("Another system settings change is in progress")
        })?)
    };
    let name = serde_json::to_value(action).expect("system settings action is serializable");
    let name = name.as_str().expect("system settings action is a string");
    let output = tokio::time::timeout(Duration::from_secs(105), async {
        let mut child = tokio::process::Command::new("/usr/libexec/yuos-system-settings")
            .arg(name)
            .arg(owner.to_string())
            .env_clear()
            .env("LANG", "C.UTF-8")
            .env("PATH", "/usr/sbin:/usr/bin")
            .stdin(if has_account { Stdio::piped() } else { Stdio::null() })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;
        if let Some(account) = request.account {
            let bytes = zeroize::Zeroizing::new(serde_json::to_vec(&account).map_err(std::io::Error::other)?);
            child.stdin.take().expect("account stdin").write_all(&bytes).await?;
        }
        child.wait_with_output().await
    })
    .await
    .map_err(|_| {
        BrokerError::unavailable("System settings result is unconfirmed; refresh the page")
    })?
    .map_err(|_| BrokerError::unavailable("Install the matching Yu OS Desktop package"))?;
    if output.status.code() == Some(3) {
        return Err(BrokerError::authorization(
            String::from_utf8_lossy(&output.stderr).trim(),
        ));
    }
    if !output.status.success() {
        return Err(BrokerError::execution(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }
    let state = serde_json::from_slice(&output.stdout)
        .map_err(|_| BrokerError::unavailable("Could not read the confirmed system settings"))?;
    if !matches!(action, SystemSettingsAction::Status) {
        crate::provenance::audit(
            "desktop.system_settings",
            serde_json::json!({"owner_uid": owner, "action": name}),
        );
    }
    Ok(state)
}

async fn decision_settings(
    action: SystemSettingsAction,
    model: Option<&str>,
    owner: u32,
) -> Result<Value, BrokerError> {
    use crate::model::tasks::decision::{validate_model_alias, LOCAL_PROVIDER};

    let mut args = vec!["agent", "setup", "decision"];
    let mutation = match (action, model) {
        (SystemSettingsAction::DecisionStatus, None) => {
            args.push("--status");
            false
        }
        (SystemSettingsAction::DecisionTest, None) => {
            args.push("test");
            false
        }
        (SystemSettingsAction::DecisionApply, Some(model)) => {
            validate_model_alias(model).map_err(BrokerError::execution)?;
            args.extend(["apply", "--provider", LOCAL_PROVIDER, "--model", model]);
            true
        }
        (SystemSettingsAction::DecisionReset, None) => {
            args.push("--reset");
            true
        }
        _ => return Err(BrokerError::execution("Invalid decision model settings action")),
    };
    let _change = if mutation {
        Some(CHANGE.try_lock().map_err(|_| {
            BrokerError::unavailable("Another system settings change is in progress")
        })?)
    } else {
        None
    };
    let home = crate::paths::verified_home_for_uid(owner).map_err(BrokerError::authorization)?;
    let (group, _) = crate::yuosd::client_identity::owner_groups(owner)
        .map_err(BrokerError::authorization)?;
    let parent = unsafe { libc::getpid() };
    let mut command = tokio::process::Command::new("/usr/local/bin/yu");
    command
        .args(args)
        .env_clear()
        .env("PATH", "/usr/sbin:/usr/bin")
        .env("LANG", "C.UTF-8")
        .env("HOME", home)
        .current_dir("/")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    unsafe {
        command.pre_exec(move || {
            crate::agentd::spawn::mark_inherited_descriptors_cloexec_except(3, -1);
            if owner == 0
                || libc::setgroups(0, std::ptr::null()) != 0
                || libc::setgid(group) != 0
                || libc::setuid(owner) != 0
                || libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0
                || libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) != 0
                || libc::getppid() != parent
            {
                return Err(std::io::Error::other("Could not isolate owner model setup"));
            }
            Ok(())
        });
    }
    let output = tokio::time::timeout(Duration::from_secs(35), command.output())
        .await
        .map_err(|_| BrokerError::unavailable("Decision setup result is unconfirmed; refresh the page"))?
        .map_err(|_| BrokerError::unavailable("Could not start the owner Decision setup"))?;
    if !output.status.success() || output.stdout.len() > 4096 {
        return Err(BrokerError::execution("Could not complete Decision setup"));
    }
    let state: Value = serde_json::from_slice(&output.stdout)
        .map_err(|_| BrokerError::unavailable("Could not read the Decision setup result"))?;
    if state["modality"] != "decision" || mutation && state["ok"] != true {
        return Err(BrokerError::unavailable("Decision setup did not confirm the requested state"));
    }
    if mutation {
        crate::provenance::audit(
            "desktop.system_settings",
            serde_json::json!({"owner_uid": owner, "action": action}),
        );
    }
    Ok(state)
}
