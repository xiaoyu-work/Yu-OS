use super::*;

struct Environment {
    directory: tempfile::TempDir,
    previous: Option<std::ffi::OsString>,
    previous_config: Option<std::ffi::OsString>,
    _guard: std::sync::MutexGuard<'static, ()>,
}

impl Environment {
    fn new() -> Self {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let guard = LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let directory = tempfile::tempdir().unwrap();
        let previous = std::env::var_os("YUOS_DATA_DIR");
        std::env::set_var("YUOS_DATA_DIR", directory.path().join("state"));
        Self {
            directory,
            previous,
            previous_config: std::env::var_os("YUOS_CONFIG_PATH"),
            _guard: guard,
        }
    }

    fn model(&self, name: &str) -> InstallModel {
        let path = self.directory.path().join("config.json");
        std::fs::write(&path, serde_json::json!({
            "agent": {"provider": "mock", "model": name},
        }).to_string()).unwrap();
        std::env::set_var("YUOS_CONFIG_PATH", path);
        InstallModel::for_owner(crate::provenance::fsec::effective_uid()).unwrap().unwrap()
    }

    fn package(&self, version: &str, network: bool) -> VerifiedPackage {
        let path = self
            .directory
            .path()
            .join(format!("package-{version}-{network}"));
        std::fs::create_dir_all(&path).unwrap();
        let mut manifest = serde_json::json!({
            "id": "review-app",
            "version": version,
            "name": {"en": "Review App"},
        });
        if network {
            manifest["operations"] = serde_json::json!({
                "fetch": {
                    "label": {"en": "Fetch"},
                    "needs": [{
                        "verb": "net.dial",
                        "scope": {"kind": "fixed", "scope": {"kind": "host", "value": "example.com"}},
                        "why": {"en": "Fetch the selected information"}
                    }]
                }
            });
        }
        self.package_from_manifest(&path, manifest)
    }

    fn package_from_manifest(&self, path: &Path, manifest: serde_json::Value) -> VerifiedPackage {
        std::fs::create_dir_all(path).unwrap();
        std::fs::write(path.join("app.json"), manifest.to_string()).unwrap();
        std::fs::write(
            path.join("main.py"),
            "# no App code is executed by review\n",
        )
        .unwrap();
        crate::test_env::sign_test_package(path, PackageKind::App, "review-app");
        crate::provenance::verify::verify_package(
            path,
            &crate::provenance::VerifyOptions::new(PackageKind::App).signature_only(),
            &crate::provenance::trust_store(),
        )
        .unwrap()
    }
}

impl Drop for Environment {
    fn drop(&mut self) {
        match self.previous.take() {
            Some(value) => std::env::set_var("YUOS_DATA_DIR", value),
            None => std::env::remove_var("YUOS_DATA_DIR"),
        }
        match self.previous_config.take() {
            Some(value) => std::env::set_var("YUOS_CONFIG_PATH", value),
            None => std::env::remove_var("YUOS_CONFIG_PATH"),
        }
    }
}

#[cfg(unix)]
#[test]
fn install_ai_is_hidden_without_an_optional_request_and_bound_to_verified_delivery() {
    let environment = Environment::new();
    let owner = crate::provenance::fsec::effective_uid();
    let model = environment.model("owner-model");
    let mut manifest = serde_json::json!({
        "id": "review-app", "version": "1.0.0", "schema_version": 2,
        "name": {"en": "Review App"},
        "mcp": {"tools": [{"name": "review.inspect", "summary": {"en": "Inspect"}}]},
        "ai": {"budget": {"monthly_units": 1000}, "origins": ["external-content"]}
    });
    let mcp_only = environment.package_from_manifest(&environment.directory.path().join("only-mcp"), manifest.clone());
    let no_ai_choice = submit(owner, ReviewKind::AppInstall, &mcp_only, "install".into()).unwrap();
    assert!(no_ai_choice.ai_model.is_none());
    assert!(no_ai_choice.permission_review().unwrap().permissions.iter().all(|permission|
        no_ai_choice.ai_choice(permission, &mcp_only.ceiling()).unwrap().is_none()));
    assert!(!has_install_gui_ai(owner, &mcp_only, &model, Verb::AI_CHAT_UNTRUSTED).unwrap());

    manifest["desktop"] = serde_json::json!({
        "needs": [],
        "optional_needs": [{
            "verb": "ai.chat.untrusted",
            "scope": {"kind": "fixed", "scope": {"kind": "name", "value": "$configured-ai-model"}},
            "why": {"en": "Suggest edits using the owner's model"}
        }]
    });
    let package = environment.package_from_manifest(&environment.directory.path().join("optional-ai"), manifest);
    let delivery = DeliveryReview {
        digest: "a".repeat(64),
        packages: vec![DeliveryPackage { package: "yuos-app-review".into(),
            version: "1.0.0".into(), architecture: "all".into() }],
    };
    let review = submit_delivery(owner, &package, "install".into(), delivery.clone()).unwrap();
    let disclosure = review.permission_review().unwrap();
    let (choice, cap) = review.ai_choice(&disclosure.permissions[0], &package.ceiling()).unwrap().unwrap();
    assert_eq!(cap.scope, crate::caps::Scope::name("owner-model"));
    assert!(decide(owner, &review.id, true, Some(&package)).is_err());
    decide_with_permissions(owner, &review.id, true, Some(&package), [(choice.clone(), true)].into()).unwrap();
    consume_delivery(owner, &review.id, &package, &delivery.digest).unwrap();
    let allows = |model: &InstallModel| has_install_gui_ai(owner, &package, model, Verb::AI_CHAT_UNTRUSTED).unwrap();
    assert!(!allows(&model));
    assert!(complete_install(owner, &review.id, &PackageRef::of(&package), Some(&"b".repeat(64))).is_err());
    complete_install(owner, &review.id, &PackageRef::of(&package), Some(&delivery.digest)).unwrap();
    assert!(allows(&model));
    assert!(!has_install_gui_ai(owner.wrapping_add(1), &package, &model, Verb::AI_CHAT_UNTRUSTED).unwrap());
    assert!(!allows(&environment.model("changed-model")));
    environment.model("owner-model");
    revoke_installed_gui_ai(owner, &package, &model, &choice).unwrap();
    assert!(!allows(&model));
}

#[test]
fn system_review_is_owner_scoped_and_pending_submissions_are_deduplicated() {
    let environment = Environment::new();
    let package = environment.package("1.0.0", false);
    let first = submit(1000, ReviewKind::AppInstall, &package, "terminal".into()).unwrap();
    let repeated = submit(1000, ReviewKind::AppInstall, &package, "terminal".into()).unwrap();
    let other = submit(1001, ReviewKind::AppInstall, &package, "terminal".into()).unwrap();
    assert_eq!(first.id, repeated.id);
    assert_ne!(first.id, other.id);
    assert!(get(1001, &first.id).is_err());
    assert_eq!(pending(1000, 100).unwrap().len(), 1);
    assert_eq!(pending(1001, 100).unwrap().len(), 1);
    assert!(pending(1000, 0).unwrap().is_empty());
}

#[test]
fn system_review_digest_is_not_an_approval_or_a_capability_grant() {
    let environment = Environment::new();
    let package = environment.package("1.0.0", true);
    let review = submit(1000, ReviewKind::AppInstall, &package, "terminal".into()).unwrap();
    assert!(consume(1000, &review.id, &package).is_err());
    assert!(get(1000, &review.contract_digest).is_err());
    assert!(decide(1000, &review.id, true, None).is_err());
    assert!(!review.permission_review().unwrap().permissions_granted);
    assert!(super::super::list_pending_for_owner(Some(1000)).is_empty());
}

#[test]
fn system_review_confirmation_is_consumed_once_without_execution_authority() {
    let environment = Environment::new();
    let package = environment.package("1.0.0", true);
    let review = submit(1000, ReviewKind::AppInstall, &package, "terminal".into()).unwrap();
    let approved = decide(1000, &review.id, true, Some(&package)).unwrap();
    assert_eq!(approved.state, ReviewState::Approved);
    assert_eq!(approved.decided_by, Some(1000));
    let retry = submit(
        1000,
        ReviewKind::AppInstall,
        &package,
        "retried terminal".into(),
    )
    .unwrap();
    assert_eq!(retry.id, review.id);
    assert_eq!(retry.state, ReviewState::Approved);
    assert!(!has_accepted(1000, &package).unwrap());
    assert!(decide(1000, &review.id, false, None).is_err());
    assert_eq!(
        consume(1000, &review.id, &package).unwrap().state,
        ReviewState::Consumed
    );
    assert!(consume(1000, &review.id, &package).is_err());
    assert!(has_accepted(1000, &package).unwrap());
    assert!(
        !get(1000, &review.id)
            .unwrap()
            .permission_review()
            .unwrap()
            .permissions_granted
    );
}

#[test]
fn delivery_confirmation_is_plan_bound_and_cannot_inherit_app_permission_acceptance() {
    let environment = Environment::new();
    let package = environment.package("1.0.0", false);
    let previous = submit(1000, ReviewKind::AppInstall, &package, "terminal".into()).unwrap();
    decide(1000, &previous.id, true, Some(&package)).unwrap();
    consume(1000, &previous.id, &package).unwrap();
    let plan = DeliveryReview {
        digest: "a".repeat(64),
        packages: vec![DeliveryPackage {
            package: "yuos-app-review".into(),
            version: "1.0.0".into(),
            architecture: "all".into(),
        }],
    };
    let review = submit_delivery(1000, &package, "App Store".into(), plan.clone()).unwrap();
    assert_eq!(review.state, ReviewState::Pending);
    assert!(review.inherited_from.is_none());
    decide(1000, &review.id, true, Some(&package)).unwrap();
    assert!(consume(1000, &review.id, &package).is_err());
    assert!(consume_delivery(1000, &review.id, &package, &"b".repeat(64)).is_err());
    consume_delivery(1000, &review.id, &package, &plan.digest).unwrap();
    assert!(consume_delivery(1000, &review.id, &package, &plan.digest).is_err());
    let changed = DeliveryReview { digest: "b".repeat(64), ..plan };
    assert_eq!(
        submit_delivery(1000, &package, "App Store".into(), changed).unwrap().state,
        ReviewState::Pending,
    );
}

#[cfg(unix)]
#[test]
fn official_apt_confirmation_needs_no_permission_choices_and_binds_exact_installation() {
    use crate::provenance::{state::TrustDomain, trust::TrustRootSpec, TrustStore, VerifyOptions};

    let environment = Environment::new();
    let source = environment.directory.path().join("review");
    std::fs::create_dir(&source).unwrap();
    std::fs::write(source.join("app.json"), serde_json::json!({
        "id": "review-app", "version": "1.0.0", "schema_version": 2,
        "name": {"en": "Review App"},
        "desktop": {"exec": "--gui", "needs": [{
            "verb": "sys.observe",
            "scope": {"kind": "fixed", "scope": {"kind": "name", "value": "hardware"}},
            "why": {"en": "Show a hardware summary"}
        }]}
    }).to_string()).unwrap();
    std::fs::write(source.join("main.py"), "# fixture only\n").unwrap();
    crate::test_env::sign_test_package(&source, PackageKind::App, "review-app");
    let uid = crate::provenance::fsec::effective_uid();
    let root = environment.directory.path().join("vendor");
    std::fs::create_dir(&root).unwrap();
    let key = crate::test_env::test_signing_key().trust_entry(&[PackageKind::App]);
    std::fs::write(root.join("publisher.json"), serde_json::to_vec(&key).unwrap()).unwrap();
    let roots = [TrustRootSpec {
        path: root,
        tier: TrustTier::Vendor,
        allowed_uids: vec![uid],
        domain: TrustDomain::Owner(uid),
    }];
    crate::test_env::record_trust_state(&roots);
    let trust = TrustStore::load_roots(&roots);
    let package =
        crate::provenance::verify::verify_package(&source, &VerifyOptions::new(PackageKind::App), &trust)
            .unwrap();
    let unbound = submit(1000, ReviewKind::AppInstall, &package, "local directory".into()).unwrap();
    assert!(decide(1000, &unbound.id, true, Some(&package)).is_err());
    let plan = DeliveryReview {
        digest: "a".repeat(64),
        packages: vec![DeliveryPackage {
            package: "yuos-app-review".into(),
            version: "1.0.0".into(),
            architecture: "all".into(),
        }],
    };
    let unconfirmed = submit_delivery(1000, &package, "App Store".into(), plan.clone()).unwrap();
    assert_eq!(unconfirmed.state, ReviewState::Pending);
    decide(1000, &unconfirmed.id, true, Some(&package)).unwrap();
    let plan = DeliveryReview { digest: "c".repeat(64), ..plan };
    let review = submit_confirmed_official_install(
        1000, &package, "Official Store selection".into(), plan.clone(),
    ).unwrap();
    assert_eq!(review.state, ReviewState::Approved);
    assert!(official_installation(1000, &package).unwrap().is_none());
    assert!(consume_delivery(1000, &review.id, &package, &"b".repeat(64)).is_err());
    assert!(consume_delivery(1001, &review.id, &package, &plan.digest).is_err());
    consume_delivery(1000, &review.id, &package, &plan.digest).unwrap();
    assert!(consume_delivery(1000, &review.id, &package, &plan.digest).is_err());
    assert_eq!(official_installation(1000, &package).unwrap(), Some(review.id));
    assert!(official_installation(1001, &package).unwrap().is_none());
    assert!(permission_choices(1000, &PackageRef::of(&package)).unwrap().is_empty());
}

#[test]
fn system_review_refuses_a_different_package_after_the_user_saw_the_request() {
    let environment = Environment::new();
    let old = environment.package("1.0.0", false);
    let changed = environment.package("2.0.0", true);
    let review = submit(1000, ReviewKind::AppInstall, &old, "terminal".into()).unwrap();
    assert!(decide(1000, &review.id, true, Some(&changed)).is_err());
    decide(1000, &review.id, true, Some(&old)).unwrap();
    assert!(consume(1000, &review.id, &changed).is_err());
    assert_eq!(get(1000, &review.id).unwrap().state, ReviewState::Approved);
}

#[test]
fn system_review_reuses_only_an_applied_same_publisher_permission_contract() {
    let environment = Environment::new();
    let old = environment.package("1.0.0", false);
    let review = submit(1000, ReviewKind::AppInstall, &old, "terminal".into()).unwrap();
    decide(1000, &review.id, true, Some(&old)).unwrap();
    let before_use = environment.package("1.0.1", false);
    assert_eq!(
        submit(1000, ReviewKind::AppInstall, &before_use, "update".into())
            .unwrap()
            .state,
        ReviewState::Pending
    );
    consume(1000, &review.id, &old).unwrap();
    let compatible = environment.package("1.0.2", false);
    let update = submit(1000, ReviewKind::AppInstall, &compatible, "update".into()).unwrap();
    assert_eq!(update.state, ReviewState::Approved);
    assert_eq!(update.inherited_from.as_deref(), Some(review.id.as_str()));
    let expanded = environment.package("2.0.0", true);
    assert_eq!(
        submit(1000, ReviewKind::AppInstall, &expanded, "update".into())
            .unwrap()
            .state,
        ReviewState::Pending
    );
}

#[test]
fn system_review_revocation_survives_restoring_an_old_review_record() {
    let environment = Environment::new();
    let package = environment.package("1.0.0", false);
    let review = submit(1000, ReviewKind::AppInstall, &package, "terminal".into()).unwrap();
    decide(1000, &review.id, true, Some(&package)).unwrap();
    consume(1000, &review.id, &package).unwrap();
    let path = record_path(&review.id).unwrap();
    let backup = std::fs::read(&path).unwrap();
    super::super::generations::revoke(&super::super::RevocationScope::Owner { uid: Some(1000) })
        .unwrap();
    std::fs::write(&path, backup).unwrap();
    assert_eq!(
        get(1000, &review.id).unwrap().effective_state().unwrap(),
        ReviewState::Stale
    );
    let update = environment.package("1.0.1", false);
    assert_eq!(
        submit(1000, ReviewKind::AppInstall, &update, "update".into())
            .unwrap()
            .state,
        ReviewState::Pending
    );
}

#[test]
fn system_review_expiration_and_denial_cannot_be_consumed() {
    let environment = Environment::new();
    let package = environment.package("1.0.0", false);
    let mut expired = submit(1000, ReviewKind::AppInstall, &package, "terminal".into()).unwrap();
    expired.expires_at = 0;
    save(&expired).unwrap();
    assert_eq!(
        get(1000, &expired.id).unwrap().effective_state().unwrap(),
        ReviewState::Expired
    );
    assert!(pending(1000, 100).unwrap().is_empty());
    assert!(decide(1000, &expired.id, true, Some(&package)).is_err());
    let denied = submit(1000, ReviewKind::AppInstall, &package, "terminal".into()).unwrap();
    decide(1000, &denied.id, false, None).unwrap();
    assert!(consume(1000, &denied.id, &package).is_err());
}

#[test]
fn system_review_rejects_corrupted_permission_snapshots() {
    let environment = Environment::new();
    let package = environment.package("1.0.0", true);
    let mut review = submit(1000, ReviewKind::AppInstall, &package, "terminal".into()).unwrap();
    review.contract_digest = "not-the-reviewed-contract".into();
    save(&review).unwrap();
    assert!(get(1000, &review.id).is_err());
    assert!(pending(1000, 100).is_err());
}

#[test]
fn system_review_confirmation_does_not_restore_a_denied_app_permission() {
    let environment = Environment::new();
    let package = environment.package("1.0.0", false);
    let cap = crate::caps::Cap::new(
        crate::caps::Verb::DEVICE_CAMERA,
        crate::caps::Scope::name("capture"),
    );
    super::super::app_policy::revoke(1000, "review-app", cap.clone()).unwrap();
    let review = submit(1000, ReviewKind::AppInstall, &package, "terminal".into()).unwrap();
    decide(1000, &review.id, true, Some(&package)).unwrap();
    consume(1000, &review.id, &package).unwrap();
    assert!(super::super::app_policy::require(
        1000,
        "review-app",
        &crate::caps::CapSet::from_caps([cap]),
    )
    .is_err());
}

#[test]
fn system_review_pending_limit_is_exact_and_owner_scoped() {
    let environment = Environment::new();
    for index in 0..MAX_PENDING_PER_OWNER {
        let package = environment.package(&format!("1.0.{index}"), false);
        submit(1000, ReviewKind::AppInstall, &package, "terminal".into()).unwrap();
    }
    assert_eq!(pending(1000, 100).unwrap().len(), MAX_PENDING_PER_OWNER);
    let extra = environment.package("2.0.0", false);
    assert!(submit(1000, ReviewKind::AppInstall, &extra, "terminal".into()).is_err());
    assert!(submit(1001, ReviewKind::AppInstall, &extra, "terminal".into()).is_ok());
}

#[cfg(unix)]
#[test]
fn system_review_rejects_symlinked_and_public_records() {
    use std::os::unix::fs::PermissionsExt;

    let environment = Environment::new();
    let package = environment.package("1.0.0", false);
    let review = submit(1000, ReviewKind::AppInstall, &package, "terminal".into()).unwrap();
    let path = record_path(&review.id).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(get(1000, &review.id).is_err());
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    let other = environment.directory.path().join("replacement");
    std::fs::rename(&path, &other).unwrap();
    std::os::unix::fs::symlink(&other, &path).unwrap();
    assert!(get(1000, &review.id).is_err());
}
