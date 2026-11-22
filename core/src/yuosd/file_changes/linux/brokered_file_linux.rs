use tokio::sync::{Semaphore, SemaphorePermit};

use crate::yuosd::brokered_file::{Lease, CHUNK_BYTES, MAX_BYTES};
use crate::yuosd::client_identity::FsIdentityGuard;
use crate::yuosd::wire::requests::{AppFileCommit, AppFileTransfer};

use super::*;

// At most 128 MiB of snapshot/upload buffers globally, and no bytes
// retained in the lease between a completed download and upload.
static BUFFERS: Semaphore = Semaphore::const_new(4);

pub(in crate::yuosd) struct Revision {
    id: String,
    target: Target,
    stat: Metadata,
    hash: String,
}

#[derive(Default)]
pub(in crate::yuosd) enum Transfer {
    #[default]
    Idle,
    Download {
        revision: Revision,
        bytes: Vec<u8>,
        offset: usize,
        _permit: SemaphorePermit<'static>,
    },
    Read(Revision),
    Upload {
        revision: Revision,
        id: String,
        bytes: Vec<u8>,
        size: usize,
        hash: String,
        _permit: SemaphorePermit<'static>,
    },
    Finished,
}

fn token() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

fn capacity() -> Result<SemaphorePermit<'static>, BrokerError> {
    BUFFERS
        .try_acquire()
        .map_err(|_| BrokerError::unavailable("App file staging capacity is exhausted"))
}

fn buffer(size: usize) -> Result<Vec<u8>, BrokerError> {
    if size > MAX_BYTES {
        return Err(unsupported("brokered file exceeds 32 MiB"));
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(size)
        .map_err(|_| BrokerError::unavailable("cannot reserve App file transfer buffer"))?;
    Ok(bytes)
}

fn owner(decision: &Decision) -> Result<FsIdentityGuard, BrokerError> {
    FsIdentityGuard::enter(decision.owner_uid()).map_err(BrokerError::authorization)
}

fn literal_target(path: &str, home: &Path) -> Result<Target, BrokerError> {
    validate_path(path)?;
    let literal = Path::new(path);
    crate::worker::derive::reject_forbidden_for_owner(literal, home)
        .map_err(BrokerError::authorization)?;
    let parent_path = literal.parent().ok_or_else(conflict)?.to_path_buf();
    let name = cstring(literal.file_name().ok_or_else(conflict)?.as_bytes())?;
    let parent = open_directory(&parent_path)?;
    Ok(Target {
        path: literal.to_path_buf(),
        requested_path: path.to_owned(),
        parent_path,
        parent,
        name,
    })
}

/// Open the exact pinned inode with the owner's current permissions, without
/// ever opening a substituted symlink or special file for IO.
fn open_owner_file(
    target: &Target,
    uid: u32,
    write: bool,
) -> Result<(File, Metadata), BrokerError> {
    target.verify_parent()?;
    let held = target.open_target()?.ok_or_else(conflict)?;
    let stat = metadata(&held)?;
    supported_bounded(&stat, MAX_BYTES)?;
    if stat.uid() != uid {
        return Err(BrokerError::authorization(
            "brokered file must belong to the authenticated owner",
        ));
    }
    let file = OpenOptions::new()
        .read(true)
        .write(write)
        .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(format!("/proc/self/fd/{}", held.as_raw_fd()))
        .map_err(|error| io_error("open brokered file as owner", error))?;
    if !same_stat(&stat, &metadata(&file)?) {
        return Err(conflict());
    }
    if write {
        no_attributes(&file)?;
    }
    Ok((file, stat))
}

fn metadata(file: &File) -> Result<Metadata, BrokerError> {
    file.metadata()
        .map_err(|error| io_error("inspect brokered file", error))
}

fn read_digest(
    file: &mut File,
    stat: &Metadata,
    mut content: Option<&mut Vec<u8>>,
) -> Result<String, BrokerError> {
    let mut digest = Sha256::new();
    let mut remaining = stat.size();
    let mut chunk = [0; 32 * 1024];
    while remaining > 0 {
        let count = remaining.min(chunk.len() as u64) as usize;
        file.read_exact(&mut chunk[..count])
            .map_err(|error| io_error("read bounded brokered file", error))?;
        digest.update(&chunk[..count]);
        if let Some(content) = content.as_deref_mut() {
            content.extend_from_slice(&chunk[..count]);
        }
        remaining -= count as u64;
    }
    if !same_stat(stat, &metadata(file)?) {
        return Err(conflict());
    }
    Ok(format!("sha256:{}", hex::encode(digest.finalize())))
}

impl Revision {
    fn recheck(&self, decision: &Decision, write: bool, strong: bool) -> Result<(), BrokerError> {
        let (mut current, stat) = open_owner_file(&self.target, decision.owner_uid(), write)?;
        if !same_stat(&self.stat, &stat)
            || (strong && read_digest(&mut current, &stat, None)? != self.hash)
        {
            return Err(conflict());
        }
        Ok(())
    }
}

fn invalid_transfer() -> BrokerError {
    refusal(
        "file_transfer_invalid",
        "brokered file transfer is incomplete or out of sequence",
    )
}

fn lock(lease: &Lease) -> Result<std::sync::MutexGuard<'_, Transfer>, BrokerError> {
    lease
        .transfer
        .lock()
        .map_err(|_| BrokerError::unavailable("App file staging is unavailable"))
}

pub(in crate::yuosd) fn transfer(
    lease: &Lease,
    proof: &Authorized,
    request: AppFileTransfer,
    decision: &Decision,
) -> Result<Value, BrokerError> {
    use AppFileTransfer::*;
    let mut state = lock(lease)?;
    let prior = std::mem::replace(&mut *state, Transfer::Finished);
    let (next, answer) = match (request, prior) {
        (ReadStart { path, .. }, Transfer::Idle | Transfer::Read(_)) => {
            let permit = capacity()?;
            let home = crate::paths::verified_home_for_uid(decision.owner_uid())
                .map_err(BrokerError::authorization)?;
            let _identity = owner(decision)?;
            let target = literal_target(path.as_str(), &home)?;
            let (mut file, stat) = open_owner_file(&target, decision.owner_uid(), false)?;
            let mut bytes = buffer(stat.size() as usize)?;
            let hash = read_digest(&mut file, &stat, Some(&mut bytes))?;
            let revision = Revision {
                id: token(),
                target,
                stat,
                hash,
            };
            revision.recheck(decision, false, false)?;
            let answer =
                json!({"revision": revision.id, "size": bytes.len(), "sha256": revision.hash});
            let next = if bytes.is_empty() {
                Transfer::Read(revision)
            } else {
                Transfer::Download {
                    revision,
                    bytes,
                    offset: 0,
                    _permit: permit,
                }
            };
            (next, answer)
        }
        (
            ReadChunk { id, offset, .. },
            Transfer::Download {
                revision,
                bytes,
                offset: cursor,
                _permit,
            },
        ) if id.as_str() == revision.id && offset == cursor => {
            {
                let _identity = owner(decision)?;
                revision.recheck(decision, false, false)?;
            }
            let end = (offset + CHUNK_BYTES).min(bytes.len());
            let answer = json!({"offset": offset, "content_base64":
                base64::engine::general_purpose::STANDARD.encode(&bytes[offset..end])});
            let next = if end == bytes.len() {
                Transfer::Read(revision)
            } else {
                Transfer::Download {
                    revision,
                    bytes,
                    offset: end,
                    _permit,
                }
            };
            (next, answer)
        }
        (
            WriteStart {
                revision: id,
                size,
                sha256,
                ..
            },
            Transfer::Read(revision),
        ) if id.as_str() == revision.id => {
            {
                let _identity = owner(decision)?;
                revision.recheck(decision, true, false)?;
            }
            let _permit = capacity()?;
            let bytes = buffer(size)?;
            let id = token();
            let answer = json!({"id": id});
            (
                Transfer::Upload {
                    revision,
                    id,
                    bytes,
                    size,
                    hash: sha256,
                    _permit,
                },
                answer,
            )
        }
        (
            WriteChunk {
                id: supplied,
                offset,
                content_base64,
                ..
            },
            Transfer::Upload {
                revision,
                id,
                mut bytes,
                size,
                hash,
                _permit,
            },
        ) if supplied.as_str() == id && offset == bytes.len() && offset < size => {
            let data = base64::engine::general_purpose::STANDARD
                .decode(content_base64.as_str())
                .map_err(|_| invalid_transfer())?;
            if data.len() != (size - offset).min(CHUNK_BYTES) {
                return Err(invalid_transfer());
            }
            bytes.extend_from_slice(&data);
            (
                Transfer::Upload {
                    revision,
                    id,
                    bytes,
                    size,
                    hash,
                    _permit,
                },
                json!({"offset": offset + data.len()}),
            )
        }
        (Abort { id, .. }, Transfer::Download { revision, .. } | Transfer::Read(revision))
            if id.as_str() == revision.id =>
        {
            (Transfer::Finished, json!({"aborted": true}))
        }
        (Abort { id, .. }, Transfer::Upload { id: current, .. }) if id.as_str() == current => {
            (Transfer::Finished, json!({"aborted": true}))
        }
        _ => return Err(invalid_transfer()),
    };
    // Permission/retirement can change while the provider reads or copies bytes.
    let _fresh = decision
        .require_all(proof.spent())
        .map_err(BrokerError::authorization)?;
    *state = next;
    Ok(answer)
}

pub(in crate::yuosd) async fn commit(
    lease: &Lease,
    proof: &Authorized,
    request: AppFileCommit,
    decision: &Decision,
) -> Result<Value, BrokerError> {
    let _writer = REPLACEMENTS.lock().await;
    let Transfer::Upload {
        revision,
        id,
        bytes,
        size,
        hash,
        _permit,
    } = std::mem::replace(&mut *lock(lease)?, Transfer::Finished)
    else {
        return Err(invalid_transfer());
    };
    if request.id.as_str() != id || size != bytes.len() || hash != sha256(&bytes) {
        return Err(invalid_transfer());
    }
    let _fresh = decision
        .require_all(proof.spent())
        .map_err(BrokerError::authorization)?;
    let id = RequestId::parse(&id).map_err(BrokerError::unavailable)?;
    let guard = crate::yuosd::journal::begin(
        Command::SystemAppFileCommit.route(),
        &id,
        Some(decision),
        &ClientIdentity::unknown(),
    )
    .map_err(BrokerError::fault)?
    .ok_or_else(|| BrokerError::unavailable("App file commit is not journal-bracketed"))?;
    let result = apply(&revision, &bytes, &hash, proof, decision);
    let response = match &result {
        Ok(value) => Response::ok(id.clone(), value.clone()),
        Err(error) => Response::handler_error(id.clone(), error.clone()),
    };
    if let Some(unresolved) = crate::yuosd::journal::finish(guard, &id, &response) {
        return Err(indeterminate(unresolved.error.map_or_else(
            || "brokered file outcome is unresolved".to_string(),
            |error| error.message,
        )));
    }
    result
}

fn apply(
    revision: &Revision,
    bytes: &[u8],
    hash: &str,
    proof: &Authorized,
    decision: &Decision,
) -> Result<Value, BrokerError> {
    let target = &revision.target;
    let (mut stage, staged) = {
        let _identity = owner(decision)?;
        revision.recheck(decision, true, true)?;
        if revision.hash == hash {
            return Ok(json!({"committed": true, "changed": false}));
        }
        let mut stage = Stage::create(proof, &target.parent)?;
        stage
            .file
            .write_all(bytes)
            .map_err(|error| io_error("write App file stage", error))?;
        stage.preserve(
            revision.stat.uid(),
            revision.stat.gid(),
            revision.stat.mode() & 0o777,
        )?;
        stage
            .file
            .sync_all()
            .map_err(|error| io_error("sync App file stage", error))?;
        let staged = metadata(&stage.file)?;
        if staged.size() != bytes.len() as u64
            || (staged.uid(), staged.gid(), staged.mode() & 0o777)
                != (
                    revision.stat.uid(),
                    revision.stat.gid(),
                    revision.stat.mode() & 0o777,
                )
        {
            return Err(conflict());
        }
        stage
            .file
            .rewind()
            .map_err(|error| io_error("rewind App file stage", error))?;
        if read_digest(&mut stage.file, &staged, None)? != hash {
            return Err(conflict());
        }
        (stage, staged)
    };
    {
        // Check the live permission after staging, then again after the
        // final owner-credential hash check, just before the exchange.
        let _fresh = decision
            .require_all(proof.spent())
            .map_err(BrokerError::authorization)?;
        {
            let _identity = owner(decision)?;
            revision.recheck(decision, true, true)?;
        }
        let _fresh = decision
            .require_all(proof.spent())
            .map_err(BrokerError::authorization)?;
        let _identity = owner(decision)?;
        revision.recheck(decision, true, false)?;
        if !same_stat(&staged, &metadata(&stage.file)?) {
            return Err(conflict());
        }
        stage.verify_name()?;
        exchange_existing(&mut stage, target, &revision.stat)?;
        target
            .parent
            .sync_all()
            .map_err(|error| indeterminate(format!("sync App file parent: {error}")))?;
    }
    Ok(json!({"committed": true, "changed": true}))
}

fn exchange_existing(
    stage: &mut Stage<'_>,
    target: &Target,
    baseline: &Metadata,
) -> Result<(), BrokerError> {
    let dir = target.parent.as_raw_fd();
    // Unlike renameat, EXCHANGE cannot turn a concurrent removal into a
    // creation. Linux has no rename-if-this-inode-and-revision primitive.
    if unsafe {
        libc::renameat2(
            dir,
            stage.name.as_ptr(),
            dir,
            target.name.as_ptr(),
            libc::RENAME_EXCHANGE,
        )
    } != 0
    {
        let error = std::io::Error::last_os_error();
        return Err(match error.raw_os_error() {
            Some(libc::ENOENT) => conflict(),
            Some(libc::ENOSYS | libc::EOPNOTSUPP | libc::EINVAL) => {
                unsupported("filesystem does not support exchange of existing files")
            }
            _ => indeterminate(format!("App file exchange returned: {error}")),
        });
    }
    // The pinned stage is now the target. Its old name may contain a
    // different owner's data in a race: never clean it up as our stage.
    stage.removed = true;
    let displaced = open_at(dir, &stage.name, libc::O_PATH | libc::O_NOFOLLOW, 0)
        .and_then(|file| file.metadata())
        .map_err(|error| indeterminate(format!("inspect displaced App file: {error}")))?;
    if (
        displaced.dev(),
        displaced.ino(),
        displaced.mode(),
        displaced.mtime(),
        displaced.mtime_nsec(),
        displaced.size(),
        displaced.nlink(),
        displaced.uid(),
        displaced.gid(),
    ) != (
        baseline.dev(),
        baseline.ino(),
        baseline.mode(),
        baseline.mtime(),
        baseline.mtime_nsec(),
        baseline.size(),
        baseline.nlink(),
        baseline.uid(),
        baseline.gid(),
    ) {
        return Err(indeterminate(
            "App file changed during exchange; the displaced entry was preserved",
        ));
    }
    if unsafe { libc::unlinkat(dir, stage.name.as_ptr(), 0) } != 0 {
        return Err(indeterminate(format!(
            "remove displaced App file: {}",
            std::io::Error::last_os_error()
        )));
    }
    Ok(())
}
