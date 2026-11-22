use std::ffi::CString;
use std::fs::{File, OpenOptions};
use std::io::Read;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};

use crate::caps::CapSet;
use crate::worker::native_fs::picker::{PICKER_PATH, fingerprint, reserve_picker_bytes};
use crate::worker::native_fs::{MetadataRoot, ReadLeaseCheck};
use crate::yuosd::authority::{self, Audience, GrantId, GrantView, Presentation};
use crate::yuosd::client_identity::FsIdentityGuard;

use super::*;

impl Window {
    pub(super) fn finish_open(
        &self,
        uri: &Text<8192>,
        binding: &Binding,
    ) -> Result<(Text<8192>, OpenAccess), String> {
        let path = local_file(uri.as_str())?;
        let home = &self.registration.home;
        let relative = path
            .strip_prefix(home)
            .map_err(|_| "System Open currently supports files in your Home folder only")?;
        MetadataRoot::validate_relative(relative)?;
        if relative.as_os_str().is_empty() || home.join(relative).as_os_str() != path.as_os_str() {
            return Err("The selected file has no literal Home-relative path".into());
        }
        refuse_private(&path, home, &crate::paths::user_data_dir())?;
        let owner = self.registration.owner();
        let pin = pin_source(owner, home, relative)?;
        let metadata = pin
            .metadata()
            .map_err(|_| "Cannot inspect the selected file")?;
        let stamp = fingerprint(&metadata);
        if let Some(native) = self.native_files.get() {
            if let Some(local) = native.provider.picker_local(relative)? {
                if self
                    .registration
                    .declared_selected_file_capability(relative, Verb::FS_READ)?
                    .is_some()
                {
                    // Observe the target through the same FUSE lookup the App would use.
                    let node = std::fs::symlink_metadata(native.mountpoint.join(local))
                        .map_err(|_| "The selected native file is unavailable")?
                        .ino();
                    let gui = self.accept_open(binding)?;
                    let selected = PickerTarget::File { pin, stamp };
                    native
                        .endpoint
                        .upgrade()
                        .ok_or("The native file view has retired")?
                        .open_from_picker(
                            node,
                            relative,
                            &selected,
                            PickerConsent {
                                expires: binding.deadline,
                            },
                            &gui,
                        )?;
                    return Ok((uri.clone(), OpenAccess::NativeRead));
                }
            }
        }
        if !self.snapshots.bound() {
            return Err("The system's private file snapshot provider is unavailable".into());
        }
        let path_text = path.to_str().ok_or("The selected filename is not UTF-8")?;
        if path_text.contains('*') {
            return Err("The selected path is not supported by the exact file policy".into());
        }
        // Keep common file extensions for native apps without publishing the host directory.
        let suffix = path
            .extension()
            .and_then(|ext| ext.to_str())
            .filter(|ext| {
                !ext.is_empty() && ext.len() <= 16 && ext.bytes().all(|b| b.is_ascii_alphanumeric())
            })
            .map(|ext| format!(".{ext}"))
            .unwrap_or_default();
        let name = format!("{}{suffix}", uuid::Uuid::new_v4().simple());
        let access_uri = url::Url::from_file_path(Path::new(PICKER_PATH).join(&name))
            .map_err(|_| "Cannot represent the app's snapshot path")?;
        let access_uri = Text::parse(access_uri.as_str()).map_err(|_| "Invalid snapshot path")?;
        let gui = self.accept_open(binding)?;
        let process = self.process.get().ok_or("The picker is not bound")?;
        let cap = Cap::new(Verb::FS_READ, Scope::path(path_text));
        let caps = CapSet::from_caps([cap.clone()]);
        if self.registration.launch.package().ceiling().clamp(&caps).0 != caps {
            return Err("This package cannot receive the selected file".into());
        }
        let lifetime = binding
            .deadline
            .saturating_duration_since(Instant::now())
            .min(gui.expires_in);
        let digest = crate::crypto::sha256_hex(name.as_bytes());
        let (grant, _spent) = authority::authorize_picker_file(
            authority::GuiReadApproval {
                process,
                gui: &gui,
                capability: &cap,
                approval_session: &self.registration.session_id,
                operation_digest: &digest,
                lifetime,
            },
            PickerConsent {
                expires: binding.deadline,
            },
            "gui.file.read.status",
        )?;
        let deadline = binding.deadline.min(Instant::now() + grant.expires_in);
        let lease = Arc::new(SnapshotLease {
            gui: gui.id,
            gui_generation: gui.generation,
            grant: grant.id,
            process: process.clone(),
            owner,
            session: self.registration.session_id.clone(),
            cap,
            deadline,
            home: home.clone(),
            generation: grant.generation,
        });
        let length = u32::try_from(metadata.len())
            .map_err(|_| "Selected file exceeds the snapshot limit")?;
        let budget = reserve_picker_bytes(length)?;
        let bytes = copy_source(owner, home, relative, &pin, stamp, length)?;
        self.accept_open(binding)?;
        lease.require()?;
        self.snapshots
            .publish(name.into(), bytes, deadline, lease, budget)?;
        Ok((access_uri, OpenAccess::ReadOnlySnapshot))
    }

    fn accept_open(&self, binding: &Binding) -> Result<GrantView, String> {
        let view = self.live()?;
        if binding.cancelled.load(Ordering::Acquire)
            || Instant::now() >= binding.deadline
            || view.id.audit_ref() != binding.grant
            || view.generation != binding.generation
        {
            return Err("The requesting window or file picker has retired".into());
        }
        Ok(view)
    }
}

struct SnapshotLease {
    gui: GrantId,
    gui_generation: u64,
    grant: GrantId,
    process: ProcessIdentity,
    owner: u32,
    session: String,
    cap: Cap,
    deadline: Instant,
    home: PathBuf,
    generation: u64,
}

impl ReadLeaseCheck for SnapshotLease {
    fn require(&self) -> Result<(), String> {
        if Instant::now() >= self.deadline {
            return Err("File snapshot lease expired".into());
        }
        crate::paths::RoutedPathContext::for_owner(self.owner, self.home.clone()).scope_sync(|| {
            if u64::from(crate::approvals::generations::current(
                Some(self.owner),
                &self.session,
            )?) != self.generation
            {
                return Err("File snapshot lease was revoked".into());
            }
            let presentation = Presentation {
                uid: self.owner,
                pid: self.process.pid() as u32,
                start_time_ticks: Some(self.process.start_ticks()),
                audience: Audience::GuiResource,
                route: "gui.file.read.status",
                session_id: Some(self.session.clone()),
            };
            let gui = authority::authority()
                .resolve_session(&self.session, &presentation)
                .map_err(|error| error.to_string())?;
            if gui.id != self.gui || gui.generation != self.gui_generation {
                return Err("The requesting window's authority changed".into());
            }
            authority::exercise_gui_file_read(self.grant, &self.cap, presentation).map(|_| ())
        })
    }
}

impl Drop for SnapshotLease {
    fn drop(&mut self) {
        authority::authority().revoke(self.grant);
    }
}

pub(super) fn local_file(uri: &str) -> Result<PathBuf, String> {
    let parsed = url::Url::parse(uri).map_err(|_| "Invalid file-picker URI")?;
    if parsed.scheme() != "file" || parsed.query().is_some() || parsed.fragment().is_some() {
        return Err("System Open requires a local file".into());
    }
    parsed
        .to_file_path()
        .ok()
        .filter(|path| {
            path.is_absolute()
                && path
                    .to_str()
                    .is_some_and(|name| name.len() <= 4096 && !name.chars().any(char::is_control))
        })
        .ok_or_else(|| "System Open requires a valid local filename".into())
}

pub(super) fn refuse_private(path: &Path, home: &Path, data: &Path) -> Result<(), String> {
    crate::worker::derive::reject_forbidden_for_owner(path, home)?;
    let state = home.join(".local/share/yuos");
    for root in [&state as &Path, data] {
        let aliased = match root.canonicalize() {
            Ok(actual) => path.starts_with(actual),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(_) => return Err("Cannot verify protected Yu OS data paths".into()),
        };
        if path.starts_with(root) || aliased {
            return Err("System Open cannot expose private App or system data".into());
        }
    }
    Ok(())
}

pub(super) fn pin_source(owner: u32, home: &Path, relative: &Path) -> Result<File, String> {
    let _owner = FsIdentityGuard::enter(owner)?;
    let home = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_PATH | libc::O_NOFOLLOW | libc::O_DIRECTORY)
        .open(home)
        .map_err(|_| "Cannot access the authenticated Home folder")?;
    if home
        .metadata()
        .map_err(|_| "Cannot inspect the Home folder")?
        .uid()
        != owner
    {
        return Err("Home folder does not belong to the authenticated owner".into());
    }
    let name = CString::new(relative.as_os_str().as_encoded_bytes())
        .map_err(|_| "Invalid selected filename")?;
    #[repr(C)]
    struct OpenHow {
        flags: u64,
        mode: u64,
        resolve: u64,
    }
    let how = OpenHow {
        flags: (libc::O_PATH | libc::O_CLOEXEC) as u64,
        mode: 0,
        // RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS | RESOLVE_NO_XDEV.
        resolve: 0x08 | 0x04 | 0x01,
    };
    let fd = unsafe {
        libc::syscall(
            libc::SYS_openat2,
            home.as_raw_fd(),
            name.as_ptr(),
            &how,
            std::mem::size_of::<OpenHow>(),
        )
    };
    if fd < 0 {
        return Err(
            "Cannot pin the selected file (links and nested mounts are not supported)".into(),
        );
    }
    let pin = unsafe { File::from_raw_fd(fd as i32) };
    let info = pin
        .metadata()
        .map_err(|_| "Cannot inspect the selected file")?;
    if !info.is_file() || info.nlink() != 1 {
        return Err("System Open requires one regular file without hard links".into());
    }
    Ok(pin)
}

pub(super) fn copy_source(
    owner: u32,
    home: &Path,
    relative: &Path,
    pin: &File,
    stamp: [u64; 8],
    length: u32,
) -> Result<Vec<u8>, String> {
    let descriptor = format!("/proc/self/fd/{}", pin.as_raw_fd());
    let path = home.join(relative);
    let actual_path = || {
        std::fs::read_link(&descriptor)
            .ok()
            .is_some_and(|actual| actual.as_os_str() == path.as_os_str())
    };
    if !actual_path() {
        return Err("The selected file moved; choose it again".into());
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length as usize)
        .map_err(|_| "Cannot allocate a file snapshot")?;
    bytes.resize(length as usize, 0);
    {
        let _owner = FsIdentityGuard::enter(owner)?;
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(&descriptor)
            .map_err(|_| "The owner cannot read the selected file")?;
        file.read_exact(&mut bytes)
            .map_err(|_| "Reading the selected file failed")?;
    }
    let current = pin_source(owner, home, relative)?;
    if !actual_path()
        || fingerprint(
            &current
                .metadata()
                .map_err(|_| "Cannot verify the selected file")?,
        ) != stamp
    {
        return Err("The selected file changed while opening; choose it again".into());
    }
    refuse_private(&path, home, &crate::paths::user_data_dir())?;
    Ok(bytes)
}
