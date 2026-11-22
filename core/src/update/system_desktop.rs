//! Root verification of the fixed desktop payload against its installed OS release.

use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::path::Path;

use serde::Deserialize;

use crate::provenance::envelope::{FileEntry, NodeKind, MAX_FILES};
use crate::provenance::fsec::{self, DirHandle, VerifiedFd};
use crate::provenance::runtime::SystemDesktopIdentity;
use crate::provenance::verify::verify_system_desktop_tree;

use super::floor::{Floor, FloorState, FloorStore};
use super::manifest::{Manifest, MAX_MANIFEST_BYTES};
use super::signature::{self, Signature};

const RELEASE_DIR: &str = "/usr/lib/yuos/release-security/yuos-desktop";
const TREE_DIR: &str = "/usr/lib/yuos/system-desktop";
const INVENTORY_PATH: &str = "/usr/lib/yuos/release-security/yuos-desktop/system-desktop-tree.json";
const COMPONENT: &str = "yuos-system-desktop-tree";
const INVENTORY_LIMIT: u64 = 8 * 1024 * 1024;
const SIGNATURE_LIMIT: u64 = 64 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Inventory {
    format: String,
    root_mode: u32,
    files: Vec<FileEntry>,
}

pub(crate) struct VerifiedSystemDesktop {
    /// Retain through checked startup (or failed-start retirement), not the whole session.
    pub(crate) package_lock: PackageManagerLock,
    pub(crate) handle: DirHandle,
    pub(crate) identity: (u64, u64),
    pub(crate) files: Vec<FileEntry>,
    pub(crate) release: SystemDesktopIdentity,
}

impl SystemDesktopIdentity {
    pub(crate) fn fingerprint(&self) -> Result<String, String> {
        let value = serde_json::to_value(self).map_err(|error| error.to_string())?;
        let bytes = super::canonical::to_bytes(&value)?;
        Ok(format!("sha256:{}", crate::crypto::sha256_hex(&bytes)))
    }
}

/// Held across admission so dpkg cannot start replacing the release during login.
pub(crate) struct PackageManagerLock {
    _files: Vec<OwnedFd>,
}

impl PackageManagerLock {
    pub(crate) fn release_after_start(lock: &mut Option<Self>, started: bool) {
        if started {
            drop(lock.take());
        }
    }
}

fn lock_package_directory(directory: &DirHandle, owner: u32) -> Result<PackageManagerLock, String> {
    let mut files = Vec::new();
    for name in [c"lock-frontend", c"lock"] {
        let raw = unsafe {
            libc::openat(
                directory.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDWR
                    | libc::O_CREAT
                    | libc::O_CLOEXEC
                    | libc::O_NOFOLLOW
                    | libc::O_NONBLOCK,
                0o640,
            )
        };
        if raw < 0 {
            return Err(format!(
                "cannot open dpkg admission lock: {}",
                std::io::Error::last_os_error()
            ));
        }
        let file = unsafe { OwnedFd::from_raw_fd(raw) };
        let meta =
            fsec::require_secure_fd(file.as_raw_fd(), &[owner]).map_err(|e| e.to_string())?;
        if !meta.is_file || meta.nlink != 1 {
            return Err("dpkg admission lock is not a protected regular file".into());
        }
        let lock = libc::flock {
            l_type: libc::F_RDLCK as _,
            l_whence: libc::SEEK_SET as _,
            l_start: 0,
            l_len: 0,
            l_pid: 0,
        };
        if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_OFD_SETLK, &lock) } != 0 {
            return Err(format!(
                "OS package transaction prevents desktop login: {}",
                std::io::Error::last_os_error()
            ));
        }
        files.push(file);
    }
    Ok(PackageManagerLock { _files: files })
}

pub(crate) fn lock_package_manager() -> Result<PackageManagerLock, String> {
    if unsafe { libc::geteuid() } != 0 {
        return Err("system desktop package admission requires Root".into());
    }
    let path = Path::new("/var/lib/dpkg");
    let meta = fsec::require_secure_location(path, &[0]).map_err(|e| e.to_string())?;
    let directory = DirHandle::open(path).map_err(|e| e.to_string())?;
    let opened = fsec::require_secure_fd(directory.as_raw_fd(), &[0]).map_err(|e| e.to_string())?;
    if !opened.is_dir || (opened.dev, opened.ino) != (meta.dev, meta.ino) {
        return Err("dpkg database directory changed during desktop admission".into());
    }
    lock_package_directory(&directory, 0)
}

fn protected_file(directory: &DirHandle, name: &str) -> Result<VerifiedFd, String> {
    let file = directory
        .open_file(name)
        .map_err(|error| error.to_string())?;
    let meta =
        fsec::require_secure_fd(file.as_raw_fd(), &[0]).map_err(|error| error.to_string())?;
    if !meta.is_file || meta.nlink != 1 {
        return Err(format!(
            "system desktop release file `{name}` is not protected"
        ));
    }
    Ok(file)
}

fn protected_keyrings() -> Result<Vec<signature::PinnedKeyring>, String> {
    let operator = Path::new(super::OPERATOR_KEYRING_DIR);
    let mut paths = Vec::new();
    match std::fs::symlink_metadata(operator) {
        Ok(_) => {
            let meta = fsec::require_secure_location(operator, &[0]).map_err(|e| e.to_string())?;
            if !meta.is_dir {
                return Err("system desktop release keyring directory is not protected".into());
            }
            for (index, entry) in std::fs::read_dir(operator)
                .map_err(|e| e.to_string())?
                .enumerate()
            {
                let entry = entry.map_err(|e| e.to_string())?;
                if index >= 1024 {
                    return Err("system desktop release keyring directory is too large".into());
                }
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "gpg" || ext == "kbx") {
                    paths.push(path);
                }
            }
            paths.sort();
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("cannot inspect release keyrings: {error}")),
    }
    let apt = Path::new(super::APT_KEYRING);
    match std::fs::symlink_metadata(apt) {
        Ok(_) => paths.push(apt.to_path_buf()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("cannot inspect APT release keyring: {error}")),
    }
    paths
        .into_iter()
        .map(|path| {
            let parent = path.parent().ok_or("release keyring has no parent")?;
            let meta = fsec::require_secure_location(parent, &[0]).map_err(|e| e.to_string())?;
            let directory = DirHandle::open(parent).map_err(|e| e.to_string())?;
            let opened =
                fsec::require_secure_fd(directory.as_raw_fd(), &[0]).map_err(|e| e.to_string())?;
            if !opened.is_dir || (opened.dev, opened.ino) != (meta.dev, meta.ino) {
                return Err("release keyring directory changed during verification".to_string());
            }
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or("release keyring has an invalid filename")?;
            let file = protected_file(&directory, name)?;
            Ok(signature::PinnedKeyring { path, file })
        })
        .collect()
}

fn parse_inventory(bytes: &[u8]) -> Result<Inventory, String> {
    if bytes.len() as u64 > INVENTORY_LIMIT {
        return Err("system desktop inventory exceeds its byte limit".to_string());
    }
    let value = super::canonical::parse_canonical(bytes)?;
    let inventory: Inventory = serde_json::from_value(value)
        .map_err(|error| format!("invalid system desktop inventory: {error}"))?;
    if inventory.format != "yuos.system-desktop-tree/v1"
        || inventory.root_mode & !0o777 != 0
        || inventory.root_mode & 0o022 != 0
        || inventory.files.is_empty()
        || inventory.files.len() > MAX_FILES
    {
        return Err("invalid system desktop inventory format or size".to_string());
    }
    if !inventory
        .files
        .windows(2)
        .all(|pair| pair[0].path < pair[1].path)
    {
        return Err("system desktop inventory paths are not unique and sorted".to_string());
    }
    for required in [
        "app.json",
        "bin/yuos-desktop-shell",
        "resources/usr/share/yuos-desktop-shell/shell.qml",
    ] {
        let entry = inventory
            .files
            .iter()
            .find(|entry| entry.path == required)
            .ok_or_else(|| format!("system desktop inventory lacks `{required}`"))?;
        if entry.kind != NodeKind::File
            || (required == "bin/yuos-desktop-shell" && entry.mode & 0o111 == 0)
        {
            return Err(format!("system desktop inventory has invalid `{required}`"));
        }
    }
    Ok(inventory)
}

fn require_installed_version(version: &str) -> Result<(), String> {
    let executable = Path::new("/usr/bin/dpkg-query");
    let meta = fsec::require_secure_location(executable, &[0]).map_err(|e| e.to_string())?;
    if !meta.is_file || meta.nlink != 1 || meta.mode & 0o111 == 0 {
        return Err("system package query executable is not protected".to_string());
    }
    let output = std::process::Command::new(executable)
        .env_clear()
        .env("LC_ALL", "C")
        .args(["-W", "-f=${db:Status-Status} ${Version}", "yuos-desktop"])
        .output()
        .map_err(|error| format!("cannot inspect installed Desktop package: {error}"))?;
    if !output.status.success() || output.stdout != format!("installed {version}").as_bytes() {
        return Err("installed Desktop package does not match the authorized release".to_string());
    }
    Ok(())
}

fn require_floor_identity(floor: &Floor, identity: &SystemDesktopIdentity) -> Result<(), String> {
    let package = floor
        .packages
        .get("yuos-desktop")
        .ok_or("system desktop package is absent from the security floor")?;
    let component = floor
        .components
        .get(COMPONENT)
        .ok_or("system desktop inventory is absent from the security floor")?;
    if package.version != identity.version
        || package.security_epoch != identity.security_epoch
        || package.abi != identity.abi
        || package.manifest_sha256 != identity.manifest_sha256
        || component.path != INVENTORY_PATH
        || component.sha256 != identity.inventory_sha256
        || floor.revoked_digests.contains(&identity.manifest_sha256)
        || floor.revoked_digests.contains(&identity.inventory_sha256)
        || !floor.trusted_keys.contains(&identity.signer)
    {
        return Err("system desktop identity is no longer authorized by the security floor".into());
    }
    Ok(())
}

/// Recheck the retained release identity without treating an owner-readable projection as authority.
pub(crate) fn assert_release_current(identity: &SystemDesktopIdentity) -> Result<(), String> {
    if unsafe { libc::geteuid() } != 0 {
        return Err("system desktop release verification requires Root".into());
    }
    let FloorState::Present { floor, .. } = FloorStore::system()
        .load()
        .map_err(|error| error.to_string())?
    else {
        return Err("system desktop requires an established security floor".into());
    };
    require_floor_identity(&floor, identity)
}

fn read_release() -> Result<(SystemDesktopIdentity, Inventory), String> {
    if unsafe { libc::geteuid() } != 0 {
        return Err("system desktop release verification requires Root".to_string());
    }
    let FloorState::Present { floor, .. } =
        FloorStore::system().load().map_err(|e| e.to_string())?
    else {
        return Err("system desktop requires an established security floor".to_string());
    };
    let package = floor
        .packages
        .get("yuos-desktop")
        .ok_or("system desktop package is absent from the security floor")?;
    require_installed_version(&package.version)?;
    fsec::require_secure_location(Path::new(RELEASE_DIR), &[0]).map_err(|e| e.to_string())?;
    let directory = DirHandle::open(Path::new(RELEASE_DIR)).map_err(|e| e.to_string())?;
    fsec::require_secure_fd(directory.as_raw_fd(), &[0]).map_err(|e| e.to_string())?;
    let manifest_file = protected_file(&directory, "manifest.json")?;
    let signature_file = protected_file(&directory, "manifest.json.asc")?;
    let inventory_file = protected_file(&directory, "system-desktop-tree.json")?;
    let manifest = Manifest::parse(
        &manifest_file
            .read_bounded(MAX_MANIFEST_BYTES)
            .map_err(|e| e.to_string())?,
    )?;
    if manifest.package != "yuos-desktop"
        || manifest.version != package.version
        || manifest.digest != package.manifest_sha256
        || manifest.security_epoch != package.security_epoch
        || manifest.abi != package.abi
        || floor.revoked_digests.contains(&manifest.digest)
    {
        return Err("system desktop release does not match the security floor".to_string());
    }
    signature_file
        .read_bounded(SIGNATURE_LIMIT)
        .map_err(|e| e.to_string())?;
    let keyrings = protected_keyrings()?;
    let key_id = match signature::verify_detached_fds(&manifest_file, &signature_file, &keyrings) {
        Signature::Verified { key_id, .. } => key_id,
        Signature::Unverifiable { reason } => {
            return Err(format!(
                "system desktop release signature did not verify: {reason}"
            ));
        }
        Signature::Absent => return Err("system desktop release signature is absent".to_string()),
    };
    if !floor.trusted_keys.contains(&key_id) || manifest.revoked_keys.contains(&key_id) {
        return Err(
            "system desktop release signer is not authorized by the security floor".to_string(),
        );
    }
    let component = manifest
        .component_digest(COMPONENT)
        .ok_or("system desktop release lacks its tree inventory")?;
    let recorded = floor
        .components
        .get(COMPONENT)
        .ok_or("system desktop inventory is absent from the security floor")?;
    let bytes = inventory_file
        .read_bounded(INVENTORY_LIMIT)
        .map_err(|e| e.to_string())?;
    let digest = crate::crypto::sha256_hex(&bytes);
    if component.path != INVENTORY_PATH
        || recorded.path != INVENTORY_PATH
        || component.sha256 != digest
        || recorded.sha256 != digest
        || floor.revoked_digests.contains(&digest)
    {
        return Err("system desktop inventory does not match the signed release".to_string());
    }
    let release = SystemDesktopIdentity {
        version: manifest.version.clone(),
        security_epoch: manifest.security_epoch,
        abi: manifest.abi,
        manifest_sha256: manifest.digest.clone(),
        inventory_sha256: digest,
        signer: key_id,
    };
    require_floor_identity(&floor, &release)?;
    let inventory = parse_inventory(&bytes)?;
    Ok((release, inventory))
}

/// This is the Root-only authority; a readable runtime projection is not a substitute.
pub(crate) fn verify(lock: PackageManagerLock) -> Result<VerifiedSystemDesktop, String> {
    let (release, inventory) = read_release()?;
    let (handle, identity) =
        verify_system_desktop_tree(Path::new(TREE_DIR), inventory.root_mode, &inventory.files)
            .map_err(|e| e.to_string())?;
    Ok(VerifiedSystemDesktop {
        package_lock: lock,
        handle,
        identity,
        files: inventory.files,
        release,
    })
}

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/test/unit/update/system_desktop.rs"
    ));
}
