//! An exact existing-file grant for one authenticated App MCP invocation.

use std::sync::Arc;

use serde_json::Value;

use super::authority::{Audience, Decision, Issuer};
use super::protocol::BrokerError;
use super::wire::requests::{AppFileCommit, AppFileTransfer};
use crate::caps::manifest::Need;
use crate::caps::{Cap, Scope, Verb};

pub(crate) const MAX_BYTES: usize = 32 * 1024 * 1024;
pub(crate) const CHUNK_BYTES: usize = 512 * 1024;
pub(crate) const CHUNK_BASE64_BYTES: usize = CHUNK_BYTES.div_ceil(3) * 4;

#[derive(Clone, Debug)]
pub(crate) struct Access {
    path: String,
    write: bool,
}

impl Access {
    pub(crate) fn caps(&self, write: bool) -> Vec<Cap> {
        let scope = Scope::path(&self.path);
        let mut caps = vec![Cap::new(Verb::FS_READ, scope.clone())];
        if write {
            caps.push(Cap::new(Verb::FS_WRITE, scope));
        }
        caps
    }

    pub(crate) fn writable(&self) -> bool {
        self.write
    }

    fn covers(&self, cap: &Cap) -> bool {
        matches!(&cap.scope, Scope::Path(path) if path == &self.path)
            && (cap.verb == Verb::FS_READ || (self.write && cap.verb == Verb::FS_WRITE))
    }
}

/// Keep managed needs out of the ordinary capability and mount sets. In v1
/// a marked call cannot obtain an alternate raw filesystem scope.
pub(crate) fn split(
    needs: &[Need],
    resolved: &[Vec<Cap>],
) -> Result<(Vec<Cap>, Option<Access>), String> {
    if needs.len() != resolved.len() {
        return Err("inconsistent App tool needs".into());
    }
    let mut raw = Vec::new();
    let mut path = None;
    let mut read = false;
    let mut write = false;
    for (need, caps) in needs.iter().zip(resolved) {
        if need.scope.brokered_file_arg().is_none() {
            raw.extend(caps.iter().cloned());
            continue;
        }
        for cap in caps {
            let Scope::Path(value) = &cap.scope else {
                return Err("brokered file scope must name an absolute literal file".into());
            };
            if !crate::caps::args::is_literal_file_path(value)
                || path.is_some_and(|prior| prior != value)
            {
                return Err("a brokered App call must name one absolute literal file".into());
            }
            path = Some(value);
            match cap.verb {
                Verb::FS_READ if !read => read = true,
                Verb::FS_WRITE if !write => write = true,
                _ => {
                    return Err(
                        "brokered file needs must be one fs.read and optional fs.write".into(),
                    )
                }
            }
        }
    }
    let Some(path) = path else {
        return Ok((raw, None));
    };
    if !read
        || raw.iter().any(|cap| {
            matches!(cap.scope, Scope::Path(_))
                || (matches!(cap.scope, Scope::Wild) && cap.verb.as_str().starts_with("fs."))
        })
    {
        return Err(
            "brokered file calls require fs.read and cannot also have raw path needs".into(),
        );
    }
    Ok((
        raw,
        Some(Access {
            path: path.clone(),
            write,
        }),
    ))
}

/// Transfer buffers belong to the grant, not the Host process, SDK or user
/// home. Dropping the grant on re-scope, revocation or expiry drops them.
pub(crate) struct Lease {
    access: Access,
    #[cfg(target_os = "linux")]
    pub(super) transfer: std::sync::Mutex<super::file_changes::brokered::Transfer>,
}

impl Lease {
    pub(crate) fn new(access: Access) -> Arc<Self> {
        Arc::new(Self {
            access,
            #[cfg(target_os = "linux")]
            transfer: std::sync::Mutex::default(),
        })
    }

    pub(crate) fn covers(&self, cap: &Cap) -> bool {
        self.access.covers(cap)
    }

    pub(crate) fn caps(&self) -> Vec<Cap> {
        self.access.caps(self.access.write)
    }
}

impl std::fmt::Debug for Lease {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrokeredFileLease").finish_non_exhaustive()
    }
}

impl PartialEq for Lease {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl Eq for Lease {}

fn require<'a>(
    decision: &'a Decision,
    session: &str,
    path: &str,
    write: bool,
) -> Result<(&'a Lease, super::authority::Authorized), BrokerError> {
    let lease = decision
        .brokered_file()
        .filter(|lease| {
            decision.issuer() == Issuer::AppGateway
                && decision.audience() == Audience::AppFile
                && decision.session_id() == Some(session)
                && decision.app_id().is_some()
                && lease.access.path == path
                && (!write || lease.access.write)
        })
        .ok_or_else(|| {
            BrokerError::authorization("no live brokered file grant for this call and path")
        })?;
    let proof = decision
        .require_all(&lease.access.caps(write))
        .map_err(|error| {
            #[cfg(target_os = "linux")]
            if let Ok(mut transfer) = lease.transfer.lock() {
                *transfer = super::file_changes::brokered::Transfer::Finished;
            }
            BrokerError::authorization(error)
        })?;
    Ok((lease, proof))
}

pub(super) fn transfer(params: Value, decision: &Decision) -> Result<Value, BrokerError> {
    let request: AppFileTransfer = serde_json::from_value(params)
        .map_err(|_| BrokerError::execution("invalid App file transfer request"))?;
    let (session, path) = request.subject();
    let (lease, proof) = require(decision, session, path, request.is_write())?;
    #[cfg(target_os = "linux")]
    {
        super::file_changes::brokered::transfer(lease, &proof, request, decision)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (lease, proof, request);
        Err(BrokerError::unavailable("brokered files require Linux"))
    }
}

pub(super) async fn commit(params: Value, decision: &Decision) -> Result<Value, BrokerError> {
    let request: AppFileCommit = serde_json::from_value(params)
        .map_err(|_| BrokerError::execution("invalid App file commit request"))?;
    let (lease, proof) = require(
        decision,
        request.session.as_str(),
        request.path.as_str(),
        true,
    )?;
    #[cfg(target_os = "linux")]
    {
        super::file_changes::brokered::commit(lease, &proof, request, decision).await
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (lease, proof, request);
        Err(BrokerError::unavailable("brokered files require Linux"))
    }
}
