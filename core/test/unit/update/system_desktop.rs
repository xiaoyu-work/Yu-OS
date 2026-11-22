use super::*;
use crate::provenance::envelope::NodeKind;

fn file(path: &str, mode: u32) -> FileEntry {
    FileEntry {
        path: path.to_string(),
        kind: NodeKind::File,
        mode,
        size: 1,
        digest: format!("sha256:{}", "a".repeat(64)),
    }
}

fn encoded(files: &[FileEntry]) -> Vec<u8> {
    super::super::canonical::to_bytes(&serde_json::json!({
        "format": "yuos.system-desktop-tree/v1",
        "root_mode": 0o755,
        "files": files,
    }))
    .unwrap()
}

#[test]
fn inventory_requires_closed_sorted_executable_contract() {
    let files = vec![
        file("app.json", 0o644),
        file("bin/yuos-desktop-shell", 0o755),
        file("resources/usr/share/yuos-desktop-shell/shell.qml", 0o644),
    ];
    assert!(parse_inventory(&encoded(&files)).is_ok());
    let mut invalid = files.clone();
    invalid[1].mode = 0o644;
    assert!(parse_inventory(&encoded(&invalid)).is_err());
    invalid = files.clone();
    invalid.push(files[0].clone());
    assert!(parse_inventory(&encoded(&invalid)).is_err());
    assert!(parse_inventory(&encoded(&files[..2])).is_err());
}

#[test]
fn release_identity_requires_the_exact_floor_and_live_signer() {
    use crate::update::floor::ComponentFloor;
    use crate::update::tests::{manifest_bytes, ManifestSpec};
    use std::collections::{BTreeMap, BTreeSet};

    let spec = ManifestSpec {
        package: "yuos-desktop",
        ..ManifestSpec::default()
    };
    let manifest = Manifest::parse(&manifest_bytes(&spec)).unwrap();
    let signer = "A".repeat(40);
    let inventory = "b".repeat(64);
    let component = ComponentFloor {
        path: INVENTORY_PATH.to_string(),
        sha256: inventory.clone(),
        size: 1,
        dev: 1,
        ino: 1,
    };
    let floor = Floor::bootstrap(
        &manifest,
        BTreeSet::from([signer.clone()]),
        BTreeMap::from([(COMPONENT.to_string(), component)]),
        chrono::Utc::now(),
    );
    let identity = SystemDesktopIdentity {
        version: manifest.version.clone(),
        security_epoch: manifest.security_epoch,
        abi: manifest.abi,
        manifest_sha256: manifest.digest.clone(),
        inventory_sha256: inventory,
        signer,
    };
    assert!(require_floor_identity(&floor, &identity).is_ok());

    let mut changed = floor.clone();
    changed.packages.remove("yuos-desktop");
    assert!(require_floor_identity(&changed, &identity).is_err());
    changed = floor.clone();
    changed.components.remove(COMPONENT);
    assert!(require_floor_identity(&changed, &identity).is_err());
    changed = floor.clone();
    changed.packages.get_mut("yuos-desktop").unwrap().version = "1:9".into();
    assert!(require_floor_identity(&changed, &identity).is_err());
    changed = floor.clone();
    changed.packages.get_mut("yuos-desktop").unwrap().security_epoch += 1;
    assert!(require_floor_identity(&changed, &identity).is_err());
    changed = floor.clone();
    changed.packages.get_mut("yuos-desktop").unwrap().abi += 1;
    assert!(require_floor_identity(&changed, &identity).is_err());
    changed = floor.clone();
    changed.packages.get_mut("yuos-desktop").unwrap().manifest_sha256 = "c".repeat(64);
    assert!(require_floor_identity(&changed, &identity).is_err());
    changed = floor.clone();
    changed.components.get_mut(COMPONENT).unwrap().path = "/wrong".into();
    assert!(require_floor_identity(&changed, &identity).is_err());
    changed = floor.clone();
    changed.components.get_mut(COMPONENT).unwrap().sha256 = "c".repeat(64);
    assert!(require_floor_identity(&changed, &identity).is_err());
    changed = floor.clone();
    changed
        .revoked_digests
        .insert(identity.manifest_sha256.clone());
    assert!(require_floor_identity(&changed, &identity).is_err());
    changed = floor.clone();
    changed
        .revoked_digests
        .insert(identity.inventory_sha256.clone());
    assert!(require_floor_identity(&changed, &identity).is_err());
    changed = floor;
    changed.trusted_keys.clear();
    assert!(require_floor_identity(&changed, &identity).is_err());
}

#[test]
fn verified_system_desktop_cannot_be_delegated_as_an_extension() {
    use crate::provenance::runtime::PackageRef;
    use crate::provenance::VerifiedPackage;

    let temp = tempfile::tempdir().unwrap();
    let directory = DirHandle::open(temp.path()).unwrap();
    let package_lock = lock_package_directory(&directory, unsafe { libc::geteuid() }).unwrap();
    let release = SystemDesktopIdentity {
        version: "1:1.0".into(),
        security_epoch: 1,
        abi: 1,
        manifest_sha256: "a".repeat(64),
        inventory_sha256: "b".repeat(64),
        signer: "A".repeat(40),
    };
    let snapshot = VerifiedSystemDesktop {
        package_lock,
        handle: directory,
        identity: (1, 2),
        files: vec![file("app.json", 0o644)],
        release: release.clone(),
    };
    let (package, _lock) = VerifiedPackage::from_system_desktop(snapshot).unwrap();
    let reference = PackageRef::of(&package);
    assert_eq!(reference.id, "desktop-shell");
    assert_eq!(reference.tier, "system-desktop");
    assert_eq!(reference.system_desktop, Some(release));
    assert!(reference.publisher_key_id.is_none());
    assert_eq!(
        package.verification_receipt().unwrap_err(),
        "system desktop cannot be delegated as an extension package"
    );
}

#[test]
fn package_admission_retains_both_dpkg_locks() {
    let temp = tempfile::tempdir().unwrap();
    let directory = DirHandle::open(temp.path()).unwrap();
    let guard = lock_package_directory(&directory, unsafe { libc::geteuid() }).unwrap();
    let probe = |name: &str, blocked: bool| {
        let code = r#"
import errno, fcntl, sys
with open(sys.argv[1], "r+") as lock:
    try:
        fcntl.lockf(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except OSError as error:
        if error.errno not in (errno.EACCES, errno.EAGAIN):
            raise
        sys.exit(0 if sys.argv[2] == "blocked" else 1)
    sys.exit(1 if sys.argv[2] == "blocked" else 0)
"#;
        let result = std::process::Command::new("python3")
            .arg("-c")
            .arg(code)
            .arg(temp.path().join(name))
            .arg(if blocked { "blocked" } else { "free" })
            .status()
            .unwrap();
        assert!(result.success(), "unexpected state for dpkg {name}");
    };
    probe("lock-frontend", true);
    probe("lock", true);
    drop(guard);
    probe("lock-frontend", false);
    probe("lock", false);
}

#[test]
fn package_admission_releases_after_start_but_retains_failed_start() {
    let temp = tempfile::tempdir().unwrap();
    let directory = DirHandle::open(temp.path()).unwrap();
    let mut guard = Some(lock_package_directory(&directory, unsafe { libc::geteuid() }).unwrap());
    PackageManagerLock::release_after_start(&mut guard, false);
    assert!(exclusive_lock_is_blocked(&temp.path().join("lock")));
    drop(guard.take());
    assert!(!exclusive_lock_is_blocked(&temp.path().join("lock")));

    guard = Some(lock_package_directory(&directory, unsafe { libc::geteuid() }).unwrap());
    PackageManagerLock::release_after_start(&mut guard, true);
    assert!(!exclusive_lock_is_blocked(&temp.path().join("lock")));
}

fn exclusive_lock_is_blocked(path: &std::path::Path) -> bool {
    let output = std::process::Command::new("python3")
        .arg("-c")
        .arg(
            r#"
import errno, fcntl, sys
with open(sys.argv[1], "r+") as lock:
    try:
        fcntl.lockf(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except OSError as error:
        if error.errno not in (errno.EACCES, errno.EAGAIN):
            raise
        sys.exit(1)
"#,
        )
        .arg(path)
        .status()
        .unwrap();
    assert!(matches!(output.code(), Some(0 | 1)));
    output.code() == Some(1)
}

#[test]
fn package_admission_rejects_redirected_lock_files() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("target");
    std::fs::write(&target, b"unchanged").unwrap();
    std::os::unix::fs::symlink(&target, temp.path().join("lock")).unwrap();
    let directory = DirHandle::open(temp.path()).unwrap();
    assert!(lock_package_directory(&directory, unsafe { libc::geteuid() }).is_err());
    assert_eq!(std::fs::read(target).unwrap(), b"unchanged");
}

#[test]
fn package_admission_refuses_an_active_dpkg_transaction() {
    use std::io::{BufRead, BufReader};
    use std::os::unix::fs::PermissionsExt;
    use std::process::Stdio;

    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("lock-frontend");
    std::fs::write(&path, b"").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();
    let mut child = std::process::Command::new("python3")
        .arg("-c")
        .arg("import fcntl,sys; f=open(sys.argv[1], 'r+'); fcntl.lockf(f, fcntl.LOCK_EX); print('ready', flush=True); sys.stdin.readline()")
        .arg(&path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    assert_eq!(line.trim(), "ready");
    let directory = DirHandle::open(temp.path()).unwrap();
    let error = lock_package_directory(&directory, unsafe { libc::geteuid() })
        .err()
        .unwrap();
    assert!(error.contains("transaction prevents desktop login"));
    drop(child.stdin.take());
    assert!(child.wait().unwrap().success());
}
