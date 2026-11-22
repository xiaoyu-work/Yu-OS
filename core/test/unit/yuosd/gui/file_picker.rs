use super::*;

#[test]
fn picker_requires_retirement_and_projects_only_installed_open_access() {
    let id = "a".repeat(32);
    let uri = "file:///nonexistent-human-selection/track.webm";
    let mut entry = Entry {
        id: id.clone(),
        options: Options {
            title: Text::parse("Open media").unwrap(),
            modal: true,
            mode: Mode::File,
            name: None,
            filter: None,
        },
        active: true,
        retired: false,
        report: Some(Report {
            outcome: Outcome::Selected {
                uri: Text::parse(uri).unwrap(),
            },
            portal_retired: true,
        }),
        open_access: None,
        selection: None,
        cancelled: Arc::new(AtomicBool::new(false)),
        slot: None,
    };
    assert_eq!(projection(&entry)["status"], "cleanup_pending");
    entry.active = false;
    assert_eq!(projection(&entry)["status"], "cleanup_pending");
    entry.retired = true;
    let failed = projection(&entry);
    assert_eq!(failed["status"], "failed");
    assert!(failed.get("uri").is_none());
    // Folder and Save still return metadata, not an Open grant.
    for mode in [Mode::Folder, Mode::Save] {
        entry.options.mode = mode;
        let result = projection(&entry);
        assert_eq!(result["uri"], uri);
        assert!(result.get("access").is_none());
    }
    entry.options.mode = Mode::File;
    entry.open_access = Some(OpenAccess::ReadOnlySnapshot);
    let snapshot = "file:///run/yuos/opened-files/generated.webm";
    entry.report.as_mut().unwrap().outcome = Outcome::Selected {
        uri: Text::parse(snapshot).unwrap(),
    };
    let result = projection(&entry);
    assert_eq!(result["status"], "selected");
    assert_eq!(result["uri"], snapshot);
    assert_eq!(result["access"], "read_only_snapshot");
    assert!(selected_open::local_file("smb://other/documents/track.webm").is_err());
    assert!(selected_open::local_file("file:///tmp/track.webm?pick=1").is_err());
    let request = json!({"session":"window","action":"open","request_id":id,"title":"Open media","modal":true});
    for field in [
        "owner_uid",
        "program",
        "bus",
        "path",
        "caps",
        "approved",
        "grant",
    ] {
        let mut invalid = request.clone();
        invalid[field] = json!("caller-selected");
        assert!(
            serde_json::from_value::<Request>(invalid).is_err(),
            "{field}"
        );
    }
    assert_snapshot_source_checks();
}

fn assert_snapshot_source_checks() {
    use crate::worker::native_fs::picker::fingerprint;
    use selected_open::{copy_source, pin_source, refuse_private};
    use std::fs;
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().canonicalize().unwrap();
    let relative = Path::new("Récit.txt");
    let source = home.join(relative);
    let private = home.join("configured-app-data");
    fs::create_dir(&private).unwrap();
    fs::write(&source, b"selected").unwrap();
    let owner = unsafe { libc::getuid() };
    let pin = pin_source(owner, &home, relative).unwrap();
    let stamp = fingerprint(&pin.metadata().unwrap());
    assert_eq!(
        copy_source(owner, &home, relative, &pin, stamp, 8).unwrap(),
        b"selected"
    );

    symlink(&relative, home.join("shortcut")).unwrap();
    assert!(pin_source(owner, &home, Path::new("shortcut")).is_err());
    fs::hard_link(&source, home.join("second-link")).unwrap();
    assert!(pin_source(owner, &home, relative).is_err());
    fs::remove_file(home.join("second-link")).unwrap();
    fs::rename(&source, home.join("old")).unwrap();
    fs::write(&source, b"switched").unwrap();
    assert!(copy_source(owner, &home, relative, &pin, stamp, 8).is_err());
    assert!(refuse_private(&private.join("another-app/content"), &home, &private).is_err());
    assert!(
        refuse_private(
            &home.join(".local/share/yuos/apps/another-app/content"),
            &home,
            &private
        )
        .is_err()
    );
}
