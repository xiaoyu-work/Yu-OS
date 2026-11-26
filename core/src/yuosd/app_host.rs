//! Broker-owned App host context shared by ordinary App services and the
//! optional Agent supervisor.
//!
//! The context, host containment and reaping belong to the Framework so that
//! App hosts, App services and their retirement run without `yuos-agentd`.

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use crate::yuosd::state::DaemonState;
use crate::yuosd::transport::limits::Admission;

const REAP_GRACE: Duration = Duration::from_secs(5);

#[derive(Clone)]
pub struct BrokerContext {
    pub state: DaemonState,
    pub admission: Arc<Admission>,
    pub primary_socket: std::path::PathBuf,
    pub isolated_execution_gid: u32,
    extension_containment:
        Arc<OnceLock<Result<Arc<crate::extension_host::spawn::ContainmentRoot>, Arc<str>>>>,
    extension_identities: Arc<
        OnceLock<Result<Arc<crate::extension_host::identity::ExtensionIdentityPool>, Arc<str>>>,
    >,
}

impl BrokerContext {
    pub fn new(
        state: DaemonState,
        admission: Arc<Admission>,
        primary_socket: std::path::PathBuf,
    ) -> Result<Self, String> {
        Ok(Self {
            state,
            admission,
            primary_socket,
            isolated_execution_gid: crate::agentd::spawn::resolve_isolated_execution_gid()?,
            extension_containment: Arc::new(OnceLock::new()),
            extension_identities: Arc::new(OnceLock::new()),
        })
    }

    pub(crate) fn extension_containment(
        &self,
    ) -> Result<Arc<crate::extension_host::spawn::ContainmentRoot>, String> {
        match self.extension_containment.get_or_init(|| {
            crate::extension_host::spawn::ContainmentRoot::establish()
                .map(Arc::new)
                .map_err(Arc::<str>::from)
        }) {
            Ok(root) => Ok(root.clone()),
            Err(error) => Err(error.to_string()),
        }
    }

    pub(crate) fn prime_extension_containment(&self) {
        if let Err(error) = self.extension_containment() {
            tracing::error!(
                error = %error,
                "extension containment unavailable; App and agent tasks will fail closed"
            );
        }
    }

    #[cfg(target_os = "linux")]
    /// Initialize custody before spawning Hosts; reuse the returned containment root.
    pub fn initialize_app_custody(
        &self,
    ) -> Result<Arc<crate::extension_host::spawn::ContainmentRoot>, String> {
        crate::yuosd::app_execution::initialize(self);
        self.extension_containment()
    }

    pub(crate) fn extension_identity_pool(
        &self,
    ) -> Result<Arc<crate::extension_host::identity::ExtensionIdentityPool>, String> {
        match self.extension_identities.get_or_init(|| {
            crate::extension_host::identity::ExtensionIdentityPool::load(
                self.isolated_execution_gid,
            )
            .map_err(Arc::<str>::from)
        }) {
            Ok(pool) => Ok(pool.clone()),
            Err(error) => Err(error.to_string()),
        }
    }

    pub(crate) fn prime_extension_identities(&self) {
        if let Err(error) = self.extension_identity_pool() {
            tracing::error!(
                error = %error,
                "extension execution uid pool unavailable; App and agent tasks will fail closed"
            );
        }
    }

    #[cfg(debug_assertions)]
    #[doc(hidden)]
    pub fn with_test_identity_pool(
        self,
        pool: Arc<crate::extension_host::identity::ExtensionIdentityPool>,
    ) -> Self {
        let _ = self.extension_identities.set(Ok(pool));
        self
    }
}

pub(crate) fn extension_approved_paths(
    identity: &crate::agentd::spawn::WorkerIdentity,
    session: Option<&crate::proc::SessionInfo>,
) -> Result<Vec<crate::extension_host::protocol::ApprovedPath>, String> {
    let mut candidates = vec![identity.home.clone()];
    for path in ["/usr/lib/yuos/apps", "/usr/lib/yuos/python"] {
        let path = std::path::PathBuf::from(path);
        if path.exists() {
            candidates.push(path);
        }
    }
    if let Some(path) = std::env::var_os("YUOS_APPS_DIR").map(std::path::PathBuf::from) {
        if path.exists() {
            candidates.push(path);
        }
    }
    if let Some(path) = std::env::var_os("YUOS_AGENT_EXTENSIONS_DIR").map(std::path::PathBuf::from) {
        if path.exists() {
            candidates.push(path);
        }
    }
    if let Some(paths) = std::env::var_os("YUOS_SDK_PYTHON_DIR") {
        for path in std::env::split_paths(&paths).filter(|path| !path.as_os_str().is_empty()) {
            if path.exists() {
                candidates.push(path);
            }
        }
    }
    if let Some(caps) = session.and_then(|session| session.caps.as_ref()) {
        for cap in caps.iter() {
            if !matches!(
                cap.verb,
                crate::caps::Verb::FS_READ | crate::caps::Verb::FS_EXEC
            ) {
                continue;
            }
            let crate::caps::Scope::Path(path) = &cap.scope else {
                continue;
            };
            if path.contains('*') || path.contains('?') {
                continue;
            }
            let path = std::path::PathBuf::from(path);
            if path.is_absolute() && path.exists() {
                candidates.push(path);
            }
        }
    }
    candidates.sort();
    candidates.dedup();
    if candidates.len() > 64 {
        return Err("extension runtime has more than 64 exact approved paths".to_string());
    }
    candidates
        .iter()
        .map(|path| crate::extension_host::spawn::approve_runtime_path(path, identity.uid))
        .collect()
}

pub(crate) async fn reap_extension_host(
    host: &mut crate::extension_host::spawn::SpawnedExtensionHost,
) -> Result<(), String> {
    host.close_app_data();
    let _ = host.child.start_kill();
    let containment = host.cgroup.cleanup().await;
    let reaped = match tokio::time::timeout(REAP_GRACE, host.child.wait()).await {
        Ok(Ok(status)) => {
            tracing::debug!(host_pid = host.pid, %status, "extension host reaped");
            Ok(())
        }
        Ok(Err(error)) => Err(format!("reap extension host {}: {error}", host.pid)),
        Err(_) => Err(format!(
            "extension host {} did not exit before the reap deadline",
            host.pid
        )),
    };
    let mounts = if containment.is_ok() {
        host.cleanup_private_mounts()
    } else {
        Err("private mounts retained because containment cleanup was not verified".to_string())
    };
    let paths = if containment.is_ok() && mounts.is_ok() {
        host.paths.cleanup()
    } else {
        Err("task state retained because process or mount cleanup was not verified".to_string())
    };
    let mut errors = Vec::new();
    for cleanup in [containment, reaped, mounts, paths] {
        if let Err(error) = cleanup {
            errors.push(error);
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}
