use super::*;

#[test]
fn key_ids_are_normalized_to_uppercase_hex() {
    assert_eq!(
        normalize_key_id("abcdef0123456789").unwrap(),
        "ABCDEF0123456789"
    );
    assert_eq!(
        normalize_key_id("ABCD EF01 2345 6789").unwrap(),
        "ABCDEF0123456789"
    );
    assert!(normalize_key_id("short").is_err());
    assert!(normalize_key_id("zzzzzzzzzzzzzzzz").is_err());
}

#[test]
fn a_missing_signature_file_is_reported_as_absent_not_verified() {
    let dir = crate::update::tests::scratch_root("signature-absent");
    let document = dir.join("manifest.json");
    std::fs::write(&document, b"{}\n").unwrap();
    let verdict = verify_detached(&document, &dir.join("manifest.json.asc"), &[]);
    assert_eq!(verdict, Signature::Absent);
    assert!(!verdict.is_verified());
}

#[test]
fn a_signature_with_no_keyring_is_unverifiable_never_verified() {
    let dir = crate::update::tests::scratch_root("signature-no-keyring");
    let document = dir.join("manifest.json");
    let detached = dir.join("manifest.json.asc");
    std::fs::write(&document, b"{}\n").unwrap();
    std::fs::write(&detached, b"-----BEGIN PGP SIGNATURE-----\n").unwrap();
    let verdict = verify_detached(&document, &detached, &[]);
    assert!(matches!(verdict, Signature::Unverifiable { .. }));
    assert!(!verdict.is_verified());
}

#[test]
fn only_a_status_line_with_a_good_signature_yields_a_key() {
    assert_eq!(
        good_signature_key("[GNUPG:] VALIDSIG ABCDEF0123456789ABCDEF0123456789ABCDEF01 x y\n"),
        Some("ABCDEF0123456789ABCDEF0123456789ABCDEF01".to_string())
    );
    assert_eq!(
        good_signature_key("[GNUPG:] GOODSIG ABCDEF0123456789 Yu OS\n"),
        Some("ABCDEF0123456789".to_string())
    );
    assert_eq!(
        good_signature_key("[GNUPG:] EXPKEYSIG ABCDEF0123456789\n"),
        None
    );
    assert_eq!(good_signature_key("gpgv: Good signature\n"), None);
}

#[test]
fn keyring_discovery_prefers_operator_roots_then_the_apt_keyring() {
    let root = crate::update::tests::scratch_root("signature-keyrings");
    let operator = joined(&root, crate::update::OPERATOR_KEYRING_DIR);
    std::fs::create_dir_all(&operator).unwrap();
    std::fs::write(operator.join("10-rotation.gpg"), b"x").unwrap();
    std::fs::write(operator.join("notes.txt"), b"x").unwrap();
    let apt = joined(&root, crate::update::APT_KEYRING);
    std::fs::create_dir_all(apt.parent().unwrap()).unwrap();
    std::fs::write(&apt, b"x").unwrap();

    let found = keyrings(&root, crate::update::APT_KEYRING);
    assert_eq!(found, vec![operator.join("10-rotation.gpg"), apt]);
}

#[cfg(target_os = "linux")]
#[test]
fn detached_fd_verification_uses_the_pinned_files() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path();
    let document = home.join("manifest");
    let signature = home.join("manifest.asc");
    let keyring = home.join("keyring.gpg");
    std::fs::write(&document, b"authorized release\n").unwrap();
    let run = |arguments: &[&str]| {
        let output = Command::new("gpg")
            .env("GNUPGHOME", home)
            .args([
                "--batch",
                "--yes",
                "--pinentry-mode",
                "loopback",
                "--passphrase",
                "",
            ])
            .args(arguments)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        output.stdout
    };
    run(&[
        "--quick-generate-key",
        "Desktop test <desktop@example.invalid>",
        "ed25519",
        "sign",
        "0",
    ]);
    std::fs::write(&keyring, run(&["--export"])).unwrap();
    run(&[
        "--detach-sign",
        "--output",
        signature.to_str().unwrap(),
        document.to_str().unwrap(),
    ]);

    let dir = crate::provenance::fsec::DirHandle::open(home).unwrap();
    let document_fd = dir.open_file("manifest").unwrap();
    let signature_fd = dir.open_file("manifest.asc").unwrap();
    let keyring_fd = dir.open_file("keyring.gpg").unwrap();
    let keyrings = [PinnedKeyring {
        path: keyring,
        file: keyring_fd,
    }];
    let keybox = [PinnedKeyring {
        path: home.join("pubring.kbx"),
        file: dir.open_file("pubring.kbx").unwrap(),
    }];
    std::fs::rename(&document, home.join("original")).unwrap();
    std::fs::write(&document, b"replacement\n").unwrap();
    std::fs::rename(home.join("keyring.gpg"), home.join("original-keyring")).unwrap();
    std::fs::write(home.join("keyring.gpg"), b"replacement\n").unwrap();
    assert!(verify_detached_fds(&document_fd, &signature_fd, &keyrings).is_verified());
    assert!(verify_detached_fds(&document_fd, &signature_fd, &keybox).is_verified());
    std::fs::write(home.join("unrelated.gpg"), b"").unwrap();
    let fallback = [
        PinnedKeyring {
            path: home.join("unrelated.gpg"),
            file: dir.open_file("unrelated.gpg").unwrap(),
        },
        PinnedKeyring {
            path: home.join("original-keyring"),
            file: dir.open_file("original-keyring").unwrap(),
        },
    ];
    assert!(verify_detached_fds(&document_fd, &signature_fd, &fallback).is_verified());
    std::fs::write(home.join("original"), b"modified\n").unwrap();
    assert!(!verify_detached_fds(&document_fd, &signature_fd, &keyrings).is_verified());
    let status = Command::new("gpgconf")
        .env("GNUPGHOME", home)
        .args(["--kill", "gpg-agent"])
        .status()
        .unwrap();
    assert!(status.success());
}
