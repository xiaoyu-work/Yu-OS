//! Owner-configured monthly App inference limit, measured in units (not money).
//! The protected Settings window writes it; the ledger tracks usage even
//! when the file is missing or the limit is zero.

use std::fs;
use std::io::{ErrorKind, Read};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::budget::Store;
use crate::paths;

const NEAR_LIMIT_PERCENT: u64 = 80;
const CONFIG_BYTES: u64 = 4096;

/// On-disk shape of `$HOME/.config/yuos/ai/budget.json`.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct UserBudget {
    /// Limit across all Apps per UTC calendar month. Zero means unlimited.
    #[serde(default)]
    pub monthly_units: u64,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub(crate) struct Status {
    pub period: String,
    pub units_used: u64,
    pub monthly_units: u64,
}

/// File the loader checks. Missing file is normal and means
/// "unlimited" — there is no implicit cap.
pub fn config_path() -> PathBuf {
    paths::user_budget_config_path()
}

/// Read the user budget config. Returns `UserBudget::default()`
/// (i.e. unlimited) when the file does not exist; `Err` only when a
/// present file fails to read or parse.
pub fn load() -> Result<UserBudget, String> {
    let path = config_path();
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = match options.open(&path) {
        Ok(file) => file,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(UserBudget::default()),
        Err(e) => return Err(format!("read {}: {e}", path.display())),
    };
    if !file.metadata().map_err(|error| error.to_string())?.is_file() {
        return Err("AI budget settings must be a regular file".into());
    }
    let mut raw = String::new();
    file.take(CONFIG_BYTES + 1).read_to_string(&mut raw)
        .map_err(|e| format!("read {}: {e}", path.display()))?;
    if raw.len() as u64 > CONFIG_BYTES {
        return Err("AI budget settings exceed the allowed size".into());
    }
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(UserBudget::default());
    }
    let parsed: UserBudget = serde_json::from_str(trimmed)
        .map_err(|e| format!("parse {}: {e}", path.display()))?;
    validate_limit(parsed.monthly_units)?;
    Ok(parsed)
}

fn validate_limit(units: u64) -> Result<(), String> {
    i64::try_from(units)
        .map(|_| ())
        .map_err(|_| "Monthly limit exceeds the supported unit range".into())
}

pub(super) fn limit(owner: Option<u32>) -> Result<u64, String> {
    let _identity = owner
        .map(crate::yuosd::client_identity::FsIdentityGuard::enter)
        .transpose()?;
    Ok(load()?.monthly_units)
}

pub(crate) fn status() -> Result<Status, String> {
    status_with(&mut Store::open()?, None, None)
}

pub(crate) fn settings(owner: u32, monthly_units: Option<u64>) -> Result<Status, String> {
    let home = paths::verified_home_for_uid(owner)?;
    let status = paths::RoutedPathContext::for_owner(owner, home).scope_sync(|| {
        status_with(&mut Store::open_for_owner(owner)?, Some(owner), monthly_units)
    })?;
    if monthly_units.is_some() {
        publish_alert(owner, &status, false);
    }
    Ok(status)
}

fn status_with(
    store: &mut Store,
    owner: Option<u32>,
    set_limit: Option<u64>,
) -> Result<Status, String> {
    store.owner_app_status(|snapshot| {
        let _identity = owner
            .map(crate::yuosd::client_identity::FsIdentityGuard::enter)
            .transpose()?;
        let monthly_units = if let Some(monthly_units) = set_limit {
            validate_limit(monthly_units)?;
            let bytes = serde_json::to_vec(&UserBudget { monthly_units }).map_err(|e| e.to_string())?;
            crate::agent::util::atomic_write_with_fsync(&config_path(), &bytes)
                .map_err(|e| format!("Could not save the AI budget: {e}"))?;
            monthly_units
        } else {
            load()?.monthly_units
        };
        Ok(Status { period: snapshot.period, units_used: snapshot.units_used, monthly_units })
    }).map_err(|e| e.to_string())
}

pub(super) fn notify(owner: u32, period: &str, refused_limit: Option<u64>) {
    match settings(owner, None) {
        Ok(status) if status.period == period => {
            // A refusal for an already changed limit is no longer informative.
            if refused_limit.is_none_or(|limit| limit == status.monthly_units) {
                publish_alert(owner, &status, refused_limit.is_some());
            }
        }
        Ok(_) => {}
        Err(error) => tracing::warn!(owner, %error, "could not check the App AI budget alert"),
    }
}

fn publish_alert(owner: u32, status: &Status, refused: bool) {
    use crate::notifications::{NotificationDraft, Severity};

    let limit = status.monthly_units;
    if limit == 0 {
        return;
    }
    let used = status.units_used;
    let (threshold, title) = if used >= limit {
        ("limit", "App AI budget reached")
    } else if refused {
        ("limit", "Not enough App AI budget")
    } else if used as u128 * 100 >= limit as u128 * NEAR_LIMIT_PERCENT as u128 {
        ("near", "App AI budget is nearly used")
    } else {
        return;
    };
    let draft = NotificationDraft::new(
        "system.ai-budget",
        format!("ai-budget.{threshold}"),
        Severity::Warning,
        title,
        format!("Yu OS recorded {used} of {limit} App AI units this month (UTC). Manage your limit in Settings > AI Budget."),
    ).dedupe(format!("app-ai-budget:{}:{threshold}", status.period));
    if let Err(error) = crate::yuosd::notifications::publish_once_for_owner(owner, draft) {
        tracing::warn!(owner, %error, "could not publish the App AI budget alert");
    }
}

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/test/unit/ai/user_budget.rs"
    ));
}
