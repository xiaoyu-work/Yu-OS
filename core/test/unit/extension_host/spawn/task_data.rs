use super::*;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;

use super::super::app_data::tests::{chown, copy_executable, mount_tmpfs};
use crate::yuosd::client_identity::AuthenticatedExtensionHost;
use crate::provenance::trust::TrustRootSpec;

const ACTOR_TEST: &str = "extension_host::spawn::task_data::tests::ordinary_task_app_actor";
const APP: &str = "task-storage";

#[cfg(target_os = "linux")]
mod store_custody {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/test/unit/extension_host/spawn/ordinary_store_custody.rs"
    ));
}

#[test]
fn task_app_data_bounds_distinct_views_without_replacing_existing_ones() {
    TaskAppData::require_capacity(0).unwrap();
    TaskAppData::require_capacity(63).unwrap();
    assert!(TaskAppData::require_capacity(64).is_err());
    assert!(TaskAppData::require_capacity(65).is_err());
}

const MAIN: &str = r#"
import os, pathlib, sqlite3

def run(command, args):
    root = pathlib.Path(os.environ["YUOS_DATA_DIR"])
    assert os.environ["YUOS_APP_ID"] == "task-storage"
    assert not (root.parent / "foreign-app").exists()
    with sqlite3.connect(root / "state.db") as db:
        db.execute("PRAGMA journal_mode=WAL")
        db.execute("CREATE TABLE IF NOT EXISTS counter (value INTEGER)")
        db.execute("INSERT INTO counter SELECT 0 WHERE NOT EXISTS (SELECT 1 FROM counter)")
        db.execute("UPDATE counter SET value=value+1")
        value = db.execute("SELECT value FROM counter").fetchone()[0]
    return {"value": value, "root": str(root)}
"#;

fn trust_roots() -> Vec<TrustRootSpec> {
    vec![TrustRootSpec {
        path: "/run/task-trust/publishers.d".into(),
        tier: crate::provenance::TrustTier::User,
        allowed_uids: vec![0],
        domain: crate::provenance::state::TrustDomain::System,
    }]
}

fn load_trust() {
    let roots = trust_roots();
    let trust = crate::provenance::TrustStore::load_roots(&roots);
    assert!(!trust.is_empty(), "{:?}", trust.diagnostics());
    crate::provenance::set_trust_store_for_roots(trust, roots);
}

#[test]
#[ignore = "private child of task_app_registration_binds_persistent_data_after_authorization"]
fn ordinary_task_app_actor() {
    assert_eq!(std::env::var("YUOS_TASK_DATA_ACTOR").unwrap(), "1");
    assert_ne!(unsafe { libc::geteuid() }, 0);
    load_trust();
    std::io::stdin().read_exact(&mut [0_u8; 1]).unwrap();
    let apps = PathBuf::from(std::env::var_os("YUOS_APPS_DIR").unwrap());
    let app = crate::apps::find_verified(&apps, APP).unwrap();
    let launch = crate::bridge::AppLaunch::new(app.require_verified().unwrap().clone()).unwrap();
    let refused = crate::bridge::run_app(&launch, "bump", &[], "/unused", apps.to_str().unwrap())
        .expect_err("a task host cannot run a CLI-only operation");
    assert!(refused.contains("require a direct owner launch"), "{refused}");
}

fn binding(
    owner: &WorkerIdentity,
    execution: &ExtensionIdentity,
    paths: &HostPaths,
    pid: u32,
) -> ExtensionBinding {
    ExtensionBinding {
        protocol: protocol::PROTOCOL_VERSION,
        purpose: protocol::HostPurpose::Task,
        task_id: format!("storage-{}", uuid::Uuid::new_v4().simple()),
        session_id: Some("task-parent".into()),
        app_id: None,
        owner_uid: owner.uid,
        extension_uid: execution.uid,
        owner_gid: execution.gid,
        capability_generation: "a".repeat(16),
        package: None,
        approved_paths: Vec::new(),
        agent_extensions: Vec::new(),
        controller_uid: owner.uid,
        controller_gid: execution.gid,
        controller_pid: std::process::id(),
        controller_start_time_ticks: crate::proc::read_start_time_ticks_pub(std::process::id()),
        host_pid: pid,
        host_start_time_ticks: crate::proc::read_start_time_ticks_pub(pid),
        lease_nonce: "b".repeat(32),
        expires_at_ms: crate::agentd::grant::now_ms() + 120_000,
        control_socket: paths.control_socket.to_string_lossy().into_owned(),
        broker_socket: paths.broker_socket.to_string_lossy().into_owned(),
    }
}

fn task_client(data: &Arc<TaskAppData>) -> ClientIdentity {
    ClientIdentity::from_verified_delegation(
        data.binding.host_pid,
        data.owner.uid,
        data.execution.uid,
        data.execution.gid,
        data.binding.host_start_time_ticks.unwrap(),
        AuthenticatedExtensionHost {
            purpose: data.binding.purpose,
            lease_id: data.binding.task_id.clone(),
            authority_session_id: data.binding.session_id.clone(),
            host_session_id: Some("task-host".into()),
            owner_uid: data.owner.uid,
            extension_uid: data.execution.uid,
            capability_generation: data.binding.capability_generation.clone(),
            host_pid: data.binding.host_pid,
            host_start_time_ticks: data.binding.host_start_time_ticks,
            task_session: Some(Arc::new(session(
                data.binding.session_id.as_deref().unwrap(),
                data.binding.controller_pid,
                crate::caps::CapSet::from_caps([crate::caps::Cap::new(
                    crate::caps::Verb::DATA_KV_WRITE, crate::caps::Scope::name(APP),
                )]),
            ))),
        },
    )
}

fn session(id: &str, pid: u32, caps: crate::caps::CapSet) -> crate::proc::SessionInfo {
    crate::proc::SessionInfo {
        session_id: id.into(),
        pid,
        command: vec!["yuos-extension-host".into(), "task-data-fixture".into()],
        started_at: chrono::Utc::now().to_rfc3339(),
        stdout_path: String::new(),
        stderr_path: String::new(),
        group: Some(protocol::EXTENSION_HOST_GROUP.into()),
        parent: None,
        workdir: None,
        exit_code: None,
        ended_at: None,
        tier: Some(crate::caps::Role::Worker.credential_tier()),
        scope: None,
        priority: None,
        caps: Some(caps),
        transient_caps: None,
        role: Some(crate::caps::Role::Worker.name().into()),
        app_id: None,
        pending_bind: false,
        start_time_ticks: crate::proc::read_start_time_ticks_pub(pid),
        client: crate::session::SessionClient::new(
            crate::session::SessionSource::BrokerTask,
            false,
            true,
        ),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires Root in private mount/network namespaces and YUOS_APP_DATA_TEST_YU/RUNNER"]
async fn task_app_registration_binds_persistent_data_after_authorization() {
    use crate::caps::CapSet;
    use crate::yuosd::transport::{frame::PeerStream, peer, ReadOutcome};
    use crate::yuosd::wire::Request;

    let _lock = crate::test_env::lock_env();
    assert_eq!(unsafe { libc::geteuid() }, 0);
    assert_ne!(
        std::fs::read_link("/proc/self/ns/mnt").unwrap(),
        std::fs::read_link("/proc/1/ns/mnt").unwrap()
    );
    let namespace = open_namespace(c"/proc/self/ns/mnt").unwrap();
    let failed = in_namespace(&namespace, || {
        Err(std::io::Error::from_raw_os_error(libc::EIO))
    });
    assert!(failed.unwrap_err().contains("mount helper"));
    let started = Instant::now();
    let timed_out = in_namespace(&namespace, || {
        unsafe { libc::poll(std::ptr::null_mut(), 0, 30_000) };
        Ok(())
    });
    assert!(timed_out.unwrap_err().contains("timed out"));
    assert!(started.elapsed() >= MOUNT_HELPER_TIMEOUT);
    assert!(started.elapsed() < Duration::from_secs(8));
    for task in std::fs::read_dir("/proc/self/task").unwrap() {
        let children = task.unwrap().path().join("children");
        match std::fs::read_to_string(children) {
            Ok(children) => assert!(children.trim().is_empty(), "mount helper was not reaped"),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => panic!("inspect namespace-helper cleanup: {error}"),
        }
    }
    let owner = crate::agentd::spawn::resolve_identity(
        std::fs::metadata(env!("CARGO_MANIFEST_DIR")).unwrap().uid(),
    )
    .unwrap();
    let backing = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
    chown(backing.path(), owner.uid, owner.gid);
    mount_tmpfs(c"/run");
    mount_tmpfs(c"/root");
    mount_tmpfs(c"/var/lib");
    std::fs::create_dir("/run/lock").unwrap();
    std::fs::set_permissions("/run/lock", std::fs::Permissions::from_mode(0o1777)).unwrap();
    mount_tmpfs(c"/usr/local/bin");
    copy_executable(
        Path::new(&std::env::var("YUOS_APP_DATA_TEST_YU").unwrap()),
        Path::new("/usr/local/bin/yu"),
    );
    copy_executable(
        Path::new(&std::env::var("YUOS_APP_DATA_TEST_RUNNER").unwrap()),
        Path::new("/usr/local/bin/yuos-app-runner"),
    );
    copy_executable(
        &std::env::current_exe().unwrap(),
        Path::new("/run/task-data-actor"),
    );
    for path in [
        "/run/task-runtime",
        "/run/task-state",
        "/run/task-log",
        "/run/task-trust/publishers.d",
        "/run/task-apps",
    ] {
        std::fs::create_dir_all(path).unwrap();
    }
    let _runtime = crate::test_env::TestEnvVarGuard::set("YUOS_RUNTIME_DIR", "/run/task-runtime");
    let _data = crate::test_env::TestEnvVarGuard::set("YUOS_DATA_DIR", "/run/task-state");
    let _caps = crate::test_env::TestEnvVarGuard::set("YUOS_CAPS_DATA_DIR", "/run/task-state");
    let _logs = crate::test_env::TestEnvVarGuard::set("YUOS_LOG_DIR", "/run/task-log");
    let _user_data = crate::test_env::TestEnvVarGuard::set("YUOS_USER_DATA_DIR", backing.path());
    let _apps = crate::test_env::TestEnvVarGuard::set("YUOS_APPS_DIR", "/run/task-apps");
    let key =
        crate::provenance::sign::SigningKeyFile::generate(Some("private Task data fixture".into()))
            .unwrap();
    let key_file = Path::new("/run/task-trust/publishers.d/key.json");
    std::fs::write(
        key_file,
        serde_json::to_vec(&key.trust_entry(&[crate::provenance::PackageKind::App])).unwrap(),
    )
    .unwrap();
    std::fs::set_permissions(key_file, std::fs::Permissions::from_mode(0o444)).unwrap();
    crate::test_env::record_trust_state(&trust_roots());
    load_trust();
    let app_dir = Path::new("/run/task-apps").join(APP);
    std::fs::create_dir(&app_dir).unwrap();
    std::fs::write(app_dir.join("main.py"), MAIN).unwrap();
    let manifest = serde_json::json!({
        "schema_version": 2, "id": APP, "version": "0.1.0", "runtime": "python",
        "name": {"en": "Task storage fixture"},
        "operations": {
            "bump": {"label": {"en": "Bump"}, "args": [], "needs": []},
            "denied": {"label": {"en": "Denied"}, "args": [], "needs": [{
                "verb": "sys.package",
                "scope": {"kind": "fixed", "scope": {"kind": "name", "value": "fixture"}},
                "why": {"en": "Fixture denial"}
            }]}
        },
    });
    std::fs::write(app_dir.join("app.json"), manifest.to_string()).unwrap();
    crate::provenance::sign::sign_directory(
        &app_dir,
        &crate::provenance::sign::SignRequest {
            kind: crate::provenance::PackageKind::App,
            id: APP.into(),
            version: "0.1.0".into(),
            manifest_schema: "2".into(),
            manifest_path: "app.json".into(),
            entrypoints: vec!["main.py".into()],
            resources: Vec::new(),
        },
        &key,
    )
    .unwrap();
    let app = crate::apps::find_verified(Path::new("/run/task-apps"), APP).unwrap();
    let package = app.require_verified().unwrap();
    let review = crate::approvals::system_review::submit(
        owner.uid,
        crate::approvals::system_review::ReviewKind::AppActivation,
        package,
        "private fixture".into(),
    )
    .unwrap();
    crate::approvals::system_review::decide(owner.uid, &review.id, true, Some(package)).unwrap();
    crate::approvals::system_review::consume(owner.uid, &review.id, package).unwrap();
    let caps = CapSet::new();
    let group = Path::new("/sys/fs/cgroup")
        .join(format!("yuos-task-data-fixture-{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir(&group).unwrap();
    let _group = store_custody::RootGroup(group.clone());
    let _containment = crate::test_env::TestEnvVarGuard::set("YUOSD_EXTENSION_CGROUP_ROOT", &group);
    let _execution_group = crate::test_env::TestEnvVarGuard::set("YUOS_EXTENSION_EXEC_GROUP", "nogroup");
    let state = crate::yuosd::state::DaemonState::try_new().unwrap();
    let admission = crate::yuosd::transport::limits::Admission::new(
        crate::yuosd::transport::limits::Limits::default(),
    );
    let primary = PathBuf::from("/run/task-data-broker.sock");
    let _listener = std::os::unix::net::UnixListener::bind(&primary).unwrap();
    let root = crate::agentd::supervisor::BrokerContext::new(
        state.clone(), admission.clone(), primary,
    ).unwrap().initialize_app_custody().unwrap();
    for execution_uid in [61010, 61011] {
        let execution = ExtensionIdentity {
            uid: execution_uid,
            gid: 65534,
            username: "task-data-fixture".into(),
        };
        let paths = HostPaths::create(&owner).unwrap();
        let listener =
            crate::extension_host::broker::bind_listener(&paths, execution.uid, execution.gid)
                .unwrap();
        let mut command = std::process::Command::new("/run/task-data-actor");
        command
            .args([
                "--exact",
                ACTOR_TEST,
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env_clear()
            .env("PATH", "/usr/local/bin:/usr/bin:/bin")
            .env("HOME", &paths.control_dir)
            .env("YUOS_DATA_DIR", paths.control_dir.join("data"))
            .env("YUOS_LOG_DIR", paths.control_dir.join("log"))
            .env("YUOS_CACHE_DIR", paths.control_dir.join("cache"))
            .env("YUOS_BIN", "/usr/local/bin/yu")
            .env("YUOS_APPS_DIR", "/run/task-apps")
            .env("YUOS_PROC_DATA_DIR", format!("/run/yuos/caps/{}", owner.uid))
            .env("YUOS_SESSION", "task-host")
            .env("YUOS_EXTENSION_BROKER_SOCKET", &paths.broker_socket)
            .env("XDG_RUNTIME_DIR", "/tmp")
            .env("YUOS_TASK_DATA_ACTOR", "1")
            .current_dir(&paths.control_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
        let task_path = CString::new(paths.dir.as_os_str().as_bytes()).unwrap();
        unsafe {
            command.pre_exec(move || {
                setup_private_mount_namespace(&task_path)?;
                if libc::setgroups(0, std::ptr::null()) != 0
                    || libc::setgid(65534) != 0
                    || libc::setuid(execution_uid) != 0
                    || libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0
                {
                    return Err(std::io::Error::last_os_error());
                }
                crate::agentd::spawn::mark_inherited_descriptors_cloexec(3);
                Ok(())
            });
        }
        let mut child = command.spawn().unwrap();
        drop(command);
        let mut host_group = crate::worker::cgroup::create_app(
            &root, &format!("task-data-{execution_uid}"),
            &crate::worker::Limits::operation(), None,
        ).unwrap();
        host_group.attach(child.id()).unwrap();
        let mut namespace = PrivateMountNamespace::capture(child.id(), &paths.dir, None).unwrap();
        let data = Arc::new(TaskAppData::new(
            owner.clone(),
            execution.clone(),
            binding(&owner, &execution, &paths, child.id()),
            paths.clone(),
            namespace.fd.clone(),
        ));
        namespace.task_app_data = Some(data.clone());
        let client = task_client(&data);
        let diagnostic = format!("{client:?}");
        assert!(!diagnostic.contains(&data.binding.lease_nonce));
        assert!(!diagnostic.contains(paths.dir.to_str().unwrap()));
        assert!(data.views.lock().unwrap().is_empty());
        for mutate in [
            |client: &mut ClientIdentity| client.uid = Some(0),
            |client: &mut ClientIdentity| client.pid = Some(std::process::id()),
            |client: &mut ClientIdentity| {
                client.extension_host.as_mut().unwrap().lease_id = "different-task".into()
            },
        ] {
            let mut forged = client.clone();
            mutate(&mut forged);
            assert!(data.bind_app(&forged, &PackageRef {
                kind: crate::provenance::PackageKind::App, id: APP.into(),
                content_digest: format!("sha256:{}", "a".repeat(64)),
                publisher_key_id: None, tier: "vendor".into(), developer_path: None, system_desktop: None,
            }).is_err());
            assert!(data.views.lock().unwrap().is_empty());
        }
        let mut transport = session("task-host", child.id(), caps.clone());
        transport.parent = data.binding.session_id.clone();
        transport.command[1] = data.binding.task_id.clone();
        crate::proc::register_session_for_owner(transport, owner.uid).unwrap();
        crate::storage::install_routed_extension_reader(owner.uid, execution.uid).unwrap();
        for operation in ["absent", "denied", "bump"] {
            let request = serde_json::json!({
                "app_id": APP, "kind": "operation", "operation": operation, "args": [],
                "package": PackageRef::of(package),
            });
            assert!(crate::yuosd::app_sessions::register(request, &client)
                .await
                .is_err());
            assert!(
                data.views.lock().unwrap().is_empty(),
                "a rejected operation mounted data"
            );
        }
        let invalid = serde_json::json!({
            "app_id": APP, "kind": "operation", "operation": "bump", "args": [],
            "package": PackageRef::of(package), "task_app_data_dir": "/caller/path",
        });
        assert!((crate::yuosd::routes::Command::AppSessionRegister
            .route()
            .decode)(invalid)
        .is_err());
        let state = state.clone();
        let admission = admission.clone();
        let (stop, mut stopped) = tokio::sync::oneshot::channel();
        let broker = tokio::spawn(async move {
            loop {
                let accepted = tokio::select! { _ = &mut stopped => break, accepted = listener.accept() => accepted };
                let mut stream = PeerStream::new(accepted.unwrap().0).unwrap();
                let ReadOutcome::Frame(frame) = stream
                    .read_request(crate::yuosd::wire::MAX_REQUEST_BYTES)
                    .await
                    .unwrap()
                else {
                    continue;
                };
                let process = peer::verify(frame.credentials).unwrap();
                assert_eq!(process.pid, client.pid.unwrap());
                assert_eq!(process.uid, client.execution_uid.unwrap());
                assert_eq!(process.start_time_ticks, client.start_time_ticks.unwrap());
                let request: Request = serde_json::from_slice(&frame.body).unwrap();
                assert_ne!(request.command, crate::yuosd::routes::Command::AppSessionBind);
                let response = crate::yuosd::server::dispatch_verified_request(
                    request, &client, &state, &admission,
                )
                .await;
                stream
                    .write_response(&crate::yuosd::protocol::encode_response(&response).unwrap())
                    .await
                    .unwrap();
            }
        });
        child.stdin.take().unwrap().write_all(b"1").unwrap();
        let deadline = Instant::now() + Duration::from_secs(30);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                break child.wait().unwrap();
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        };
        data.close();
        assert!(data.bind_app(&task_client(&data), &PackageRef {
            kind: crate::provenance::PackageKind::App, id: APP.into(),
            content_digest: format!("sha256:{}", "a".repeat(64)),
            publisher_key_id: None, tier: "vendor".into(), developer_path: None, system_desktop: None,
        }).is_err());
        let stopped = stop.send(());
        let served = broker.await;
        let count = data.views.lock().unwrap().len();
        namespace.cleanup().unwrap();
        paths.cleanup().unwrap();
        host_group.retire_checked(Instant::now() + Duration::from_secs(5)).unwrap();
        host_group.remove_checked().unwrap();
        crate::proc::deregister_session_for_owner("task-host", owner.uid);
        crate::storage::remove_routed_extension_reader(owner.uid, execution.uid).unwrap();
        assert!(
            stopped.is_ok(),
            "private broker stopped before the fixture completed"
        );
        served.unwrap();
        assert!(status.success(), "ordinary App task failed: {status}");
        assert_eq!(count, 0, "the task host did not mount App data");
        assert!(data.views.lock().unwrap().is_empty());
        let db_path = backing.path().join("apps").join(APP).join("state.db");
        assert!(!db_path.exists());
    }
}
