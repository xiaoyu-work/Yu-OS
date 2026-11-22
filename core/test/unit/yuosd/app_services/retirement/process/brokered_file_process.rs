use super::*;
use crate::caps::{Cap, CapSet, Scope, Verb};
use crate::yuosd::client_identity::ClientIdentity;
use crate::yuosd::transport::peer::PeerProcess;
use serde_json::json;
use std::os::unix::process::CommandExt;

#[tokio::test]
#[ignore = "requires Root, private namespaces and the built CLI, Host and runner"]
async fn public_sdk_reads_and_replaces_as_the_owner() {
    let _lock = crate::test_env::lock_env();
    assert_eq!(unsafe { libc::geteuid() }, 0);
    for namespace in ["mnt", "net"] {
        assert_ne!(
            std::fs::read_link(format!("/proc/self/ns/{namespace}")).unwrap(),
            std::fs::read_link(format!("/proc/1/ns/{namespace}")).unwrap()
        );
    }
    let owner =
        crate::agentd::spawn::resolve_identity(std::env::var("SUDO_UID").unwrap().parse().unwrap())
            .unwrap();
    assert_ne!(owner.uid, 0);
    let host_source = std::env::var("YUOS_SERVICE_RETIREMENT_HOST").unwrap();
    mount_tmpfs(c"/run");
    mount_tmpfs(c"/var/lib");
    std::fs::create_dir("/run/lock").unwrap();
    std::fs::set_permissions("/run/lock", std::fs::Permissions::from_mode(0o1777)).unwrap();
    let root = Path::new("/run/bf");
    std::fs::create_dir(root).unwrap();
    for dir in ["apps", "runtime", "state", "docs", "sdk", "owner-data"] {
        std::fs::create_dir(root.join(dir)).unwrap();
    }
    own(&root.join("docs"), owner.uid, owner.gid);
    own(&root.join("owner-data"), owner.uid, owner.gid);
    std::fs::set_permissions(root.join("owner-data"), std::fs::Permissions::from_mode(0o700))
        .unwrap();
    let docs = root.join("docs");
    let target = docs.join("owner.bin");
    let original = vec![0xdb; 32 * 1024 * 1024];
    std::fs::write(&target, &original).unwrap();
    own(&target, owner.uid, owner.gid);
    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o640)).unwrap();
    std::os::unix::fs::symlink(&target, docs.join("link.bin")).unwrap();
    std::fs::write(docs.join("foreign.bin"), b"foreign").unwrap();
    std::fs::set_permissions(
        docs.join("foreign.bin"),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    let stale = docs.join("stale.bin");
    std::fs::write(&stale, b"baseline").unwrap();
    own(&stale, owner.uid, owner.gid);
    let host = root.join("yuos-extension-host");
    executable(Path::new(&host_source), &host);
    let sdk_source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../yuos-sdk/python/src/yuos_sdk");
    let sdk = root.join("sdk/yuos_sdk");
    std::fs::create_dir(&sdk).unwrap();
    for entry in std::fs::read_dir(sdk_source).unwrap().map(Result::unwrap) {
        if entry
            .path()
            .extension()
            .is_some_and(|extension| extension == "py")
        {
            std::fs::copy(entry.path(), sdk.join(entry.file_name())).unwrap();
        }
    }
    let group = CgroupRoot::create();
    let _environment = [
        crate::test_env::TestEnvVarGuard::set("YUOS_EXTENSION_HOST_BIN", &host),
        crate::test_env::TestEnvVarGuard::set("YUOSD_EXTENSION_CGROUP_ROOT", &group.0),
        crate::test_env::TestEnvVarGuard::set("YUOS_EXTENSION_EXEC_GROUP", "nogroup"),
        crate::test_env::TestEnvVarGuard::set("YUOS_RUNTIME_DIR", root.join("runtime")),
        crate::test_env::TestEnvVarGuard::set("YUOS_DATA_DIR", root.join("state")),
        crate::test_env::TestEnvVarGuard::set("YUOS_CAPS_DATA_DIR", root.join("caps")),
        crate::test_env::TestEnvVarGuard::set("YUOS_USER_DATA_DIR", root.join("owner-data")),
        crate::test_env::TestEnvVarGuard::set("YUOS_APPS_DIR", root.join("apps")),
        crate::test_env::TestEnvVarGuard::set("YUOS_SDK_PYTHON_DIR", root.join("sdk")),
        crate::test_env::TestEnvVarGuard::set("YUOS_BIN", "/usr/local/bin/yu"),
    ];
    let key = private_trust();
    let app_id = "binary-file-probe";
    install_probe(&root.join("apps"), &key, owner.uid, app_id);

    let primary = root.join("yuosd.sock");
    let _listener = std::os::unix::net::UnixListener::bind(&primary).unwrap();
    own(&primary, 0, owner.gid);
    std::fs::set_permissions(&primary, std::fs::Permissions::from_mode(0o660)).unwrap();
    let state = crate::yuosd::state::DaemonState::new().unwrap();
    let broker = crate::agentd::supervisor::BrokerContext::new(
        state,
        crate::yuosd::transport::limits::Admission::new(Default::default()),
        primary,
    )
    .unwrap();
    let pool = ExtensionIdentityPool::for_test_with_quarantine(
        broker.isolated_execution_gid,
        root.join("quarantine"),
    )
    .unwrap();
    let manager = AppServiceManager::new(broker.with_test_identity_pool(pool));

    // The real ordinary Linux process supplies the launcher's identity and
    // scoped delegation. The service Host, sandbox and relay are not mocked.
    let mut launcher = tokio::process::Command::new("/usr/bin/sleep");
    launcher.as_std_mut().uid(owner.uid).gid(owner.gid);
    let launcher = launcher.kill_on_drop(true).arg("180").spawn().unwrap();
    let pid = launcher.id().unwrap();
    let start = crate::proc::read_start_time_ticks_pub(pid).unwrap();
    let session = "brokered-file-test-launcher";
    let scopes = Scope::path(format!("{}/**", docs.display()));
    crate::proc::register_session_for_owner(
        crate::proc::SessionInfo {
            session_id: session.into(),
            pid,
            start_time_ticks: Some(start),
            command: vec!["sleep".into()],
            started_at: chrono::Utc::now().to_rfc3339(),
            stdout_path: String::new(),
            stderr_path: String::new(),
            group: None,
            parent: None,
            workdir: None,
            exit_code: None,
            ended_at: None,
            tier: None,
            scope: None,
            priority: None,
            caps: Some(CapSet::from_caps([
                Cap::new(Verb::FS_READ, scopes.clone()),
                Cap::new(Verb::FS_WRITE, scopes),
            ])),
            transient_caps: None,
            role: None,
            app_id: None,
            pending_bind: false,
            client: crate::session::SessionClient::new(
                crate::session::SessionSource::LocalCli,
                false,
                true,
            ),
        },
        owner.uid,
    )
    .unwrap();
    let client = ClientIdentity::from_peer(PeerProcess {
        pid,
        uid: owner.uid,
        gid: owner.gid,
        start_time_ticks: start,
    });
    let args = |tool: &str, path: &Path, mode: &str| json!({"app_id": app_id, "tool": tool, "arguments": {"path": path, "mode": mode}});
    let prepare =
        |params| crate::yuosd::app_sessions::prepare_cli_app_service_call(params, &client);
    let read = prepare(args("probe.read", &target, "read")).await.unwrap();
    assert!(read.authorized_mounts.is_empty());
    let result = manager.call(read).await.unwrap();
    assert_ne!(result.is_error, Some(true), "{result:?}");
    assert_eq!(std::fs::read(&target).unwrap(), original);

    for invalid in [
        docs.clone(),
        docs.join("absent.bin"),
        docs.join("link.bin"),
        docs.join("foreign.bin"),
    ] {
        let read = prepare(args("probe.read", &invalid, "reject"))
            .await
            .unwrap();
        let result = manager.call(read).await.unwrap();
        assert_ne!(result.is_error, Some(true), "{invalid:?}: {result:?}");
    }
    assert!(prepare(args("probe.read", &docs.join("*.bin"), "reject"))
        .await
        .is_err());

    for (mode, file) in [("partial", &stale), ("stale", &stale), ("apply", &target)] {
        let params = args("probe.apply", file, mode);
        let denied = prepare(params.clone()).await.unwrap_err();
        assert_eq!(denied.audit_class, Some("approval_required"), "{denied:?}");
        assert_eq!(std::fs::read(&target).unwrap(), original);
        let pending = crate::approvals::list_pending_for_owner(Some(owner.uid));
        let [approval] = pending.as_slice() else {
            panic!("expected exactly one write approval: {pending:?}");
        };
        assert_eq!(approval.verb, "fs.write");
        assert_eq!(approval.scope, Scope::path(file.to_str().unwrap()));
        crate::approvals::approve_for_owner(
            &approval.id,
            crate::approvals::GrantDuration::Once,
            Some("isolated-fixture".into()),
            None,
            Some(owner.uid),
        )
        .unwrap();
        let prepared = prepare(params).await.unwrap();
        let result = if mode == "stale" {
            let scratch = root.join("owner-data/apps").join(app_id);
            let (result, ()) = tokio::join!(manager.call(prepared), async {
                tokio::time::timeout(Duration::from_secs(20), async {
                    while !scratch.join("uploaded").exists() {
                        tokio::time::sleep(Duration::from_millis(20)).await;
                    }
                })
                .await
                .expect("App did not finish its upload");
                let _owner =
                    crate::yuosd::client_identity::FsIdentityGuard::enter(owner.uid).unwrap();
                std::fs::write(&stale, b"owner edit").unwrap();
                std::fs::write(scratch.join("owner-edited"), b"").unwrap();
            });
            result.unwrap()
        } else {
            manager.call(prepared).await.unwrap()
        };
        assert_ne!(result.is_error, Some(true), "{mode}: {result:?}");
        assert_eq!(std::fs::read_dir(&docs).unwrap().count(), 4);
        if mode == "partial" {
            assert_eq!(std::fs::read(&stale).unwrap(), b"baseline");
        } else if mode == "stale" {
            assert_eq!(std::fs::read(&stale).unwrap(), b"owner edit");
        }
    }
    let contents = std::fs::read(&target).unwrap();
    assert_eq!(contents[0], 0);
    assert_eq!(&contents[1..], &original[1..]);
    let stat = std::fs::metadata(&target).unwrap();
    assert_eq!(
        (stat.uid(), stat.gid(), stat.mode() & 0o777),
        (owner.uid, owner.gid, 0o640)
    );
    manager
        .retire_app_for_owner(
            app_id,
            Some(owner.uid),
            Instant::now() + Duration::from_secs(10),
        )
        .await
        .unwrap();
}

fn private_trust() -> sign::SigningKeyFile {
    // Both the controller and the exec'd Host resolve production trust paths.
    // Preserve NSS in the private mount, without touching host trust or accounts.
    let nss = ["/etc/passwd", "/etc/group", "/etc/nsswitch.conf"]
        .map(|path| (path, std::fs::read(path).unwrap()));
    mount_tmpfs(c"/etc");
    for (path, bytes) in nss {
        std::fs::write(path, bytes).unwrap();
    }
    let publishers = Path::new(crate::provenance::trust::SYSTEM_TRUST_ROOT);
    std::fs::create_dir_all(publishers).unwrap();
    let key = sign::SigningKeyFile::generate(None).unwrap();
    std::fs::write(
        publishers.join("fixture.json"),
        serde_json::to_vec(&key.trust_entry(&[PackageKind::App])).unwrap(),
    )
    .unwrap();
    let roots: Vec<_> = TrustStore::default_roots()
        .into_iter()
        .filter(|root| root.domain == crate::provenance::state::TrustDomain::System)
        .collect();
    crate::test_env::record_trust_state(&roots);
    key
}

fn install_probe(root: &Path, key: &sign::SigningKeyFile, owner: u32, id: &str) {
    let dir = root.join(id);
    std::fs::create_dir(&dir).unwrap();
    let need = |verb| {
        json!({"verb": verb, "scope": {
        "kind": "from-arg", "arg": "path", "transform": "brokered-file"
    }, "why": {"en": "Test the exact file"}})
    };
    let tool = |name, needs| {
        json!({"name": name, "summary": {"en": "Probe the file broker"},
        "args": [{"name": "path", "kind": "path", "required": true},
                 {"name": "mode", "kind": "text", "required": true}], "needs": needs})
    };
    let manifest = json!({
        "schema_version": 2, "id": id, "version": "0.1.0", "runtime": "python",
        "name": {"en": "Binary file probe"}, "summary": {"en": "Private test fixture"},
        "operations": {}, "mcp": {"transport": "stdio", "entry": "server.py",
            "tools": [tool("probe.read", vec![need("fs.read")]),
                      tool("probe.apply", vec![need("fs.read"), need("fs.write")])]}
    });
    std::fs::write(dir.join("app.json"), serde_json::to_vec(&manifest).unwrap()).unwrap();
    std::fs::write(
        dir.join("server.py"),
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/test/support/brokered_file_mcp.py",
        )),
    )
    .unwrap();
    sign::sign_directory(
        &dir,
        &sign::SignRequest {
            kind: PackageKind::App,
            id: id.into(),
            version: "0.1.0".into(),
            manifest_schema: "yuos.app/v2".into(),
            manifest_path: "app.json".into(),
            entrypoints: vec!["server.py".into()],
            resources: vec![],
        },
        key,
    )
    .unwrap();
    let app = crate::apps::find_verified_fresh(root, id).unwrap();
    let package = app.require_verified().unwrap();
    let review = crate::approvals::system_review::submit(
        owner,
        crate::approvals::system_review::ReviewKind::AppActivation,
        package,
        "fixture".into(),
    )
    .unwrap();
    crate::approvals::system_review::decide(owner, &review.id, true, Some(package)).unwrap();
    crate::approvals::system_review::consume(owner, &review.id, package).unwrap();
}
