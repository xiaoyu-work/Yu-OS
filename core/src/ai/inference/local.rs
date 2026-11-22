use std::io::{Read, Write};
use std::path::Path;
use std::process::Stdio;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use yuos_embed::{EmbedRequest, Embedder};

use crate::config::{DecisionConfig, EmbedConfig};
use crate::model::tasks::{
    decision::{DecisionModel, DecisionRequest},
    qwen3_genai,
};

const WORKER: &str = "/usr/local/bin/yu";
const WORKER_ARG: &str = "__app-local-model";
const REQUEST_BYTES: u64 = 64 * 1024;
const RESPONSE_BYTES: u64 = 512 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(tag = "task", rename_all = "lowercase", deny_unknown_fields)]
pub(super) enum Task {
    Embed {
        config: EmbedConfig,
        text: String,
    },
    Decide {
        config: DecisionConfig,
        request: DecisionRequest,
    },
}

pub(super) async fn run(owner: u32, home: &Path, task: &Task) -> Result<Value, &'static str> {
    if owner == 0 || unsafe { libc::geteuid() } != 0 {
        return Err("local App models require a non-root owner and the installed broker");
    }
    crate::agentd::spawn::validate_root_owned_executable(Path::new(WORKER))
        .map_err(|_| "local App model worker is not installed")?;
    crate::update::runtime::enforce_component_binary("yu", Path::new(WORKER))
        .map_err(|_| "local App model worker failed runtime integrity checks")?;
    let (gid, _) = crate::yuosd::client_identity::owner_groups(owner)
        .map_err(|_| "local model owner is unavailable")?;
    let input = serde_json::to_vec(task).map_err(|_| "could not encode local model request")?;
    if input.len() as u64 > REQUEST_BYTES {
        return Err("local model request exceeds its byte limit");
    }
    let parent = unsafe { libc::getpid() };
    let mut command = tokio::process::Command::new(WORKER);
    command
        .arg(WORKER_ARG)
        .env_clear()
        .env("HOME", home)
        .current_dir("/")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    unsafe {
        command.pre_exec(move || {
            crate::agentd::spawn::mark_inherited_descriptors_cloexec(3);
            if libc::setgroups(0, std::ptr::null()) != 0
                || libc::setresgid(gid, gid, gid) != 0
                || libc::setresuid(owner, owner, owner) != 0
                || libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0
                || libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) != 0
            {
                return Err(std::io::Error::last_os_error());
            }
            if libc::getppid() != parent {
                return Err(std::io::Error::from_raw_os_error(libc::EPIPE));
            }
            libc::umask(0o077);
            Ok(())
        });
    }
    let mut child = command
        .spawn()
        .map_err(|_| "could not start local model worker")?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or("local model worker has no stdin")?;
    let stdout = child
        .stdout
        .take()
        .ok_or("local model worker has no stdout")?;
    let mut reply = Vec::new();
    tokio::try_join!(async move { stdin.write_all(&input).await }, async {
        stdout
            .take(RESPONSE_BYTES + 1)
            .read_to_end(&mut reply)
            .await
    },)
    .map_err(|_| "local model worker I/O failed")?;
    if reply.len() as u64 > RESPONSE_BYTES {
        return Err("local model reply exceeds its byte limit");
    }
    let status = child
        .wait()
        .await
        .map_err(|_| "could not reap local model worker")?;
    if !status.success() {
        return Err("local model unavailable; check the owner's model configuration");
    }
    serde_json::from_slice(&reply).map_err(|_| "invalid local model reply")
}

#[doc(hidden)]
pub fn serve_local() -> Result<(), String> {
    use std::os::unix::fs::MetadataExt;
    let parent = unsafe { libc::getppid() };
    if unsafe { libc::geteuid() } == 0
        || !std::fs::metadata(format!("/proc/{parent}")).is_ok_and(|meta| meta.uid() == 0)
    {
        return Err("local model worker requires Root parent and non-root credentials".into());
    }
    let mut bytes = Vec::new();
    std::io::stdin()
        .lock()
        .take(REQUEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > REQUEST_BYTES {
        return Err("oversized local model request".into());
    }
    let task: Task = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())?;
    let reply = runtime.block_on(async move {
        match task {
            Task::Embed { config, text } => {
                let provider = qwen3_genai::build_from_config(&config);
                let reply = provider
                    .embed(EmbedRequest { inputs: vec![text] })
                    .await
                    .map_err(|error| error.to_string())?;
                serde_json::to_vec(&reply).map_err(|error| error.to_string())
            }
            Task::Decide { config, request } => {
                let provider =
                    DecisionModel::from_config(&config)?.ok_or("decision is not configured")?;
                let reply = provider.decide(&request).await?;
                serde_json::to_vec(&reply).map_err(|error| error.to_string())
            }
        }
    })?;
    if reply.len() as u64 > RESPONSE_BYTES {
        return Err("oversized local model reply".into());
    }
    std::io::stdout()
        .lock()
        .write_all(&reply)
        .map_err(|error| error.to_string())
}
