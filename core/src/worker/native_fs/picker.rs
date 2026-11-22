//! Window-local, read-only snapshots. This filesystem never walks a host path.

use std::ffi::{CString, OsStr, OsString};
use std::fs::OpenOptions;
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime};

use fuser::{FileAttr, FileType, Filesystem, ReplyAttr, ReplyData, ReplyEntry, ReplyOpen, Request};
use yuos_display_control::{ProcessIdentity, workload::InstanceProcess};

pub(crate) use super::backend::fingerprint;
use super::{ReadLeaseCheck, backend::request_leader, mount_at};

pub(crate) const PICKER_PATH: &str = "/run/yuos/opened-files";
pub(crate) const PICKER_FILE_MAX: u32 = 32 * 1024 * 1024;
const WINDOW_BYTES: usize = 64 * 1024 * 1024;
const WINDOW_FILES: usize = 8;
static TOTAL_BYTES: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(256 * 1024 * 1024);

pub(crate) fn reserve_picker_bytes(
    bytes: u32,
) -> Result<tokio::sync::SemaphorePermit<'static>, String> {
    if bytes > PICKER_FILE_MAX {
        return Err("Selected file exceeds the 32 MiB snapshot limit".into());
    }
    TOTAL_BYTES
        .try_acquire_many(bytes)
        .map_err(|_| "The system file snapshot budget is full".into())
}

struct FileSnapshot {
    name: OsString,
    bytes: Vec<u8>,
    deadline: Instant,
    lease: Arc<dyn ReadLeaseCheck>,
    _budget: tokio::sync::SemaphorePermit<'static>,
}

#[derive(Default)]
pub(crate) struct PickerFiles {
    instance: OnceLock<InstanceProcess>,
    stopped: AtomicBool,
    files: Mutex<Vec<Arc<FileSnapshot>>>,
}

impl PickerFiles {
    pub(crate) fn bind(&self, process: &ProcessIdentity, group: OwnedFd) -> Result<(), String> {
        let instance =
            InstanceProcess::receive(process.pidfd().map_err(|error| error.to_string())?, group)
                .map_err(|error| error.to_string())?;
        self.instance
            .set(instance)
            .map_err(|_| "File snapshots are already bound".into())
    }

    pub(crate) fn stop(&self) {
        self.stopped.store(true, Ordering::Release);
        if let Ok(retired) = self
            .files
            .lock()
            .map(|mut files| std::mem::take(&mut *files))
        {
            drop(retired);
        }
    }

    pub(crate) fn bound(&self) -> bool {
        self.instance.get().is_some()
    }

    pub(crate) fn refresh(&self) -> Result<(), String> {
        let files = self
            .files
            .lock()
            .map_err(|_| "File snapshot custody is poisoned")?
            .clone();
        for file in files {
            if Instant::now() >= file.deadline || file.lease.require().is_err() {
                self.stop();
                return Err("A selected file's window lease was revoked or expired".into());
            }
        }
        Ok(())
    }

    pub(crate) fn publish(
        &self,
        name: OsString,
        bytes: Vec<u8>,
        deadline: Instant,
        lease: Arc<dyn ReadLeaseCheck>,
        budget: tokio::sync::SemaphorePermit<'static>,
    ) -> Result<(), String> {
        let mut files = self
            .files
            .lock()
            .map_err(|_| "File snapshot custody is poisoned")?;
        if self.stopped.load(Ordering::Acquire)
            || Instant::now() >= deadline
            || !self
                .instance
                .get()
                .is_some_and(|instance| instance.active().unwrap_or(false))
        {
            return Err("The requesting window has retired".into());
        }
        if files.len() >= WINDOW_FILES
            || files.iter().map(|file| file.bytes.len()).sum::<usize>() + bytes.len() > WINDOW_BYTES
        {
            return Err(
                "Window file snapshot limit reached; reopen the App to select more files".into(),
            );
        }
        if files.iter().any(|file| file.name == name) {
            return Err("File snapshot name is already in use".into());
        }
        files.push(Arc::new(FileSnapshot {
            name,
            bytes,
            deadline,
            lease,
            _budget: budget,
        }));
        Ok(())
    }

    fn peer(&self, request: &Request<'_>) -> Result<(), i32> {
        if self.stopped.load(Ordering::Acquire) || request.pid() <= 1 {
            return Err(libc::EACCES);
        }
        let instance = self.instance.get().ok_or(libc::EACCES)?;
        let peer = crate::worker::gui_transport::kernel::sender(libc::ucred {
            pid: request_leader(request.pid())?,
            uid: request.uid(),
            gid: request.gid(),
        })
        .map_err(|_| libc::EACCES)?;
        if !instance.is_peer(&peer).map_err(|_| libc::EACCES)? {
            return Err(libc::EACCES);
        }
        Ok(())
    }
}

struct SnapshotFs(Arc<PickerFiles>);

fn attr(ino: u64, size: u64, kind: FileType, perm: u16) -> FileAttr {
    FileAttr {
        ino,
        size,
        blocks: size.div_ceil(512),
        kind,
        perm,
        uid: 0,
        gid: 0,
        nlink: 1,
        rdev: 0,
        blksize: 4096,
        flags: 0,
        atime: SystemTime::UNIX_EPOCH,
        mtime: SystemTime::UNIX_EPOCH,
        ctime: SystemTime::UNIX_EPOCH,
        crtime: SystemTime::UNIX_EPOCH,
    }
}

impl SnapshotFs {
    fn inspect<T>(
        &self,
        req: &Request<'_>,
        read: impl FnOnce(&[Arc<FileSnapshot>]) -> Result<T, i32>,
    ) -> Result<T, i32> {
        self.0.peer(req)?;
        let files = self.0.files.lock().map_err(|_| libc::EIO)?;
        read(&files)
    }
}

fn snapshot(files: &[Arc<FileSnapshot>], ino: u64) -> Result<Arc<FileSnapshot>, i32> {
    files
        .get(usize::try_from(ino.checked_sub(2).ok_or(libc::ENOENT)?).map_err(|_| libc::ENOENT)?)
        .filter(|file| Instant::now() < file.deadline)
        .cloned()
        .ok_or(libc::ENOENT)
}

impl Filesystem for SnapshotFs {
    fn lookup(&mut self, req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEntry) {
        let found = self.inspect(req, |files| {
            if parent != 1 {
                return Err(libc::ENOENT);
            }
            let (index, file) = files
                .iter()
                .enumerate()
                .find(|(_, file)| file.name == name && Instant::now() < file.deadline)
                .ok_or(libc::ENOENT)?;
            Ok(attr(
                index as u64 + 2,
                file.bytes.len() as u64,
                FileType::RegularFile,
                0o444,
            ))
        });
        match found {
            Ok(attr) => reply.entry(&Duration::ZERO, &attr, 1),
            Err(error) => reply.error(error),
        }
    }

    fn getattr(&mut self, req: &Request<'_>, ino: u64, _fh: Option<u64>, reply: ReplyAttr) {
        // Bubblewrap must be able to bind the empty mount before the App gate opens.
        let info = if ino == 1 {
            Ok(attr(1, 0, FileType::Directory, 0o511))
        } else {
            self.inspect(req, |files| {
                Ok(attr(
                    ino,
                    snapshot(files, ino)?.bytes.len() as u64,
                    FileType::RegularFile,
                    0o444,
                ))
            })
        };
        match info {
            Ok(attr) => reply.attr(&Duration::ZERO, &attr),
            Err(error) => reply.error(error),
        }
    }

    fn open(&mut self, req: &Request<'_>, ino: u64, flags: i32, reply: ReplyOpen) {
        match self
            .inspect(req, |files| {
                if flags & (libc::O_ACCMODE | libc::O_TRUNC) != libc::O_RDONLY {
                    return Err(libc::EROFS);
                }
                snapshot(files, ino)
            })
            .and_then(|file| file.lease.require().map_err(|_| libc::EACCES))
        {
            Ok(()) => reply.opened(ino, fuser::consts::FOPEN_DIRECT_IO),
            Err(error) => reply.error(error),
        }
    }

    fn read(
        &mut self,
        req: &Request<'_>,
        ino: u64,
        fh: u64,
        offset: i64,
        size: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyData,
    ) {
        if fh != ino || offset < 0 {
            reply.error(libc::EINVAL);
            return;
        }
        match self
            .inspect(req, |files| snapshot(files, ino))
            .and_then(|file| {
                file.lease.require().map_err(|_| libc::EACCES)?;
                Ok(file)
            }) {
            Ok(file) => {
                if self.0.stopped.load(Ordering::Acquire) || Instant::now() >= file.deadline {
                    reply.error(libc::EACCES);
                    return;
                }
                let start = usize::try_from(offset)
                    .unwrap_or(usize::MAX)
                    .min(file.bytes.len());
                let end = start
                    .saturating_add(size.min(128 * 1024) as usize)
                    .min(file.bytes.len());
                reply.data(&file.bytes[start..end]);
            }
            Err(error) => reply.error(error),
        }
    }
}

pub(crate) struct PickerMount {
    pub(crate) path: PathBuf,
    created: bool,
    mounted: Option<u64>,
    worker: Option<JoinHandle<std::io::Result<()>>>,
}

impl PickerMount {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self {
            path,
            created: false,
            mounted: None,
            worker: None,
        }
    }

    pub(crate) fn mount(&mut self, files: Arc<PickerFiles>) -> Result<(), String> {
        let device = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open("/dev/fuse")
            .map_err(|error| format!("System Open requires /dev/fuse: {error}"))?;
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&self.path)
            .map_err(|error| error.to_string())?;
        self.created = true;
        let path =
            CString::new(self.path.as_os_str().as_encoded_bytes()).map_err(|e| e.to_string())?;
        let options = CString::new(format!(
            "fd={},rootmode=40000,user_id=0,group_id=0,allow_other,default_permissions,max_read=131072",
            device.as_raw_fd(),
        )).expect("fixed snapshot mount options");
        if unsafe {
            libc::mount(
                c"yuos-open-files".as_ptr(),
                path.as_ptr(),
                c"fuse".as_ptr(),
                libc::MS_RDONLY | libc::MS_NOSUID | libc::MS_NODEV | libc::MS_NOEXEC,
                options.as_ptr().cast(),
            )
        } != 0
        {
            return Err(format!(
                "Mount private Open files: {}",
                std::io::Error::last_os_error()
            ));
        }
        self.mounted = mount_at(&self.path)?
            .filter(|(_, kind)| kind == "fuse")
            .map(|(id, _)| id);
        if self.mounted.is_none() {
            return Err("Private Open mount identity is unavailable".into());
        }
        self.worker = Some(
            std::thread::Builder::new()
                .name("yuos-open-files".into())
                .spawn(move || {
                    fuser::Session::from_fd(
                        SnapshotFs(files),
                        device.into(),
                        fuser::SessionACL::All,
                    )
                    .run()
                })
                .map_err(|error| error.to_string())?,
        );
        Ok(())
    }

    pub(crate) fn health(&self) -> Result<(), String> {
        if self
            .worker
            .as_ref()
            .is_some_and(|worker| worker.is_finished())
        {
            return Err("Private Open file provider stopped".into());
        }
        Ok(())
    }

    pub(crate) fn retire(&mut self, deadline: Instant) -> Result<(), String> {
        if let Some(expected) = self.mounted {
            if mount_at(&self.path)? != Some((expected, "fuse".into())) {
                return Err("Private Open mount identity changed; custody retained".into());
            }
            let path = CString::new(self.path.as_os_str().as_encoded_bytes())
                .map_err(|e| e.to_string())?;
            if unsafe { libc::umount2(path.as_ptr(), 0) } != 0 {
                return Err(format!(
                    "Unmount private Open files: {}",
                    std::io::Error::last_os_error()
                ));
            }
            self.mounted = None;
        }
        if mount_at(&self.path)?.is_some() {
            return Err("Private Open mount remains after retirement".into());
        }
        while self
            .worker
            .as_ref()
            .is_some_and(|worker| !worker.is_finished())
        {
            if Instant::now() >= deadline {
                return Err("Private Open worker is still retiring".into());
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        if let Some(worker) = self.worker.take() {
            if !matches!(worker.join(), Ok(Ok(()))) {
                tracing::warn!("Private Open file worker did not exit normally");
            }
        }
        if self.created {
            std::fs::remove_dir(&self.path).map_err(|error| error.to_string())?;
            self.created = false;
        }
        Ok(())
    }
}
