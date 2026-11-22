use super::*;
use crate::yuosd::authority::{
    authority, Audience, AudienceSet, Binding, Issuance, Issuer, Presentation, Principal,
    Requirement, Subject, Uses,
};

const ACCOUNT: &str = "00000000-0000-4000-8000-000000000001:account1";
const OTHER: &str = "00000000-0000-4000-8000-000000000001:account2";

fn reference(account: &str) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
        serde_json::to_vec(&json!([account, "native-folder", "public@example.invalid"])).unwrap(),
    )
}

fn request(action: &str, account: Option<&str>, args: Value) -> MailExchange {
    serde_json::from_value(json!({
        "action":action, "account":account,
        "args":args, "deadline_unix_ms":123456,
    }))
    .unwrap()
}

fn decision(caps: Vec<Cap>) -> Decision {
    let route = crate::yuosd::routes::Command::PrivateMailExchange.as_str();
    let uid = unsafe { libc::geteuid() };
    let session = format!("mail-{}", uuid::Uuid::new_v4().simple());
    let (_, view) = authority()
        .issue(Issuance {
            issuer: Issuer::AppSessionAuthority,
            principal: Principal::of_process(uid, std::process::id()).unwrap(),
            binding: Binding::ProcessTree,
            subject: Subject::session(session.clone()),
            audience: AudienceSet::one(Audience::SystemService),
            caps: crate::caps::CapSet::from_caps(caps),
            lifetime: Duration::from_secs(60),
            uses: Uses::Budget(1),
            index_session: true,
        })
        .unwrap();
    Decision::for_test(
        view,
        route,
        Audience::SystemService,
        Presentation {
            uid,
            pid: std::process::id(),
            start_time_ticks: crate::proc::read_start_time_ticks_pub(std::process::id()),
            audience: Audience::SystemService,
            route,
            session_id: Some(session),
        },
        None,
        &Requirement::RouteDerived,
    )
}

#[test]
fn mail_attachment_spends_only_an_exact_write() {
    let _lock = crate::test_env::lock_env();
    let root = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
    let _data = crate::test_env::TestEnvVarGuard::set("YUOS_DATA_DIR", root.path().join("data"));
    let cap = Cap::new(Verb::FS_WRITE, Scope::path("/tmp/mail-fixture.pdf"));
    let wide = decision(vec![Cap::new(Verb::FS_WRITE, Scope::path("/tmp/**"))]);
    assert!(authorize_attachment(&cap, &wide).is_err());
    let allowed = decision(vec![cap.clone()]);
    authorize_attachment(&cap, &allowed).unwrap();
    assert!(authorize_attachment(&cap, &allowed).is_err());
}

#[test]
fn mail_routes_reject_direct_peers_and_caller_identity_fields() {
    use crate::yuosd::{routes::Command, wire::Fault};
    assert!(Command::parse("system.mail.control").is_none());
    let route = Command::PrivateMailExchange.route();
    for uid in [0, 1000] {
        let mut client = ClientIdentity::unknown();
        client.uid = Some(uid);
        assert_eq!(route.authorize(&client), Err(Fault::NotAuthorized));
    }
    let body = json!({"action":"accounts", "args":{}, "deadline_unix_ms":123456});
    assert!((route.decode)(body.clone()).is_ok());
    for field in ["app_id", "session", "owner_uid", "tool", "socket", "profile"] {
        let mut body = body.clone();
        body[field] = json!("claimed");
        assert!((route.decode)(body).is_err(), "{field}");
    }
}

#[test]
fn mail_package_marker_requires_the_exact_private_protocol() {
    assert!(check_protocol_marker(br#"{"version":2}"#).is_ok());
    for marker in [
        br#"{"version":1}"#.as_slice(),
        br#"{"version":2,"mailbox":true}"#.as_slice(),
        br#"{"version":"2"}"#.as_slice(),
    ] {
        assert!(check_protocol_marker(marker).is_err());
    }
    assert_eq!(mailbox_command("native_message"), Some("read"));
    assert_eq!(mailbox_command("attachment_file"), None);
    assert_eq!(mailbox_command("draft"), None);
}

#[test]
fn native_mail_references_and_draft_confirmation_cannot_cross_accounts() {
    assert!(prepare(&request(
        "read",
        Some(ACCOUNT),
        json!({"id":reference(OTHER)})
    ))
    .is_err());
    let valid = request(
        "draft_send",
        Some(ACCOUNT),
        json!({
            "id":reference(ACCOUNT), "sender_account":OTHER,
            "review":format!("sha256:{}", "a".repeat(64)), "confirm":true,
        }),
    );
    assert_eq!(prepare(&valid).unwrap(), None);
    let mut false_confirmation = valid.clone();
    false_confirmation
        .args
        .insert("confirm".into(), json!(false));
    assert!(prepare(&false_confirmation).is_err());
    let mut other_profile = valid;
    other_profile.args.insert(
        "sender_account".into(),
        json!("00000000-0000-4000-8000-000000000002:account2"),
    );
    assert!(prepare(&other_profile).is_err());
}

#[test]
fn mailbox_payloads_reject_provider_credentials_and_unbounded_attachment_requests() {
    assert!(prepare(&request("accounts", None, json!({"provider":"gmail"}))).is_err());
    assert!(prepare(&request(
        "send",
        Some(ACCOUNT),
        json!({
            "to":"public@example.invalid","subject":"hello\r\nBcc: other@example.invalid","body":"",
        })
    ))
    .is_err());
    let valid = request(
        "attachment_download",
        Some(ACCOUNT),
        json!({
            "id":reference(ACCOUNT), "attachment":"1.2", "output":"/tmp/mail-fixture.pdf",
            "max_bytes":MAX_ATTACHMENT,
        }),
    );
    assert_eq!(
        prepare(&valid).unwrap(),
        Some(Cap::new(
            Verb::FS_WRITE,
            Scope::path("/tmp/mail-fixture.pdf")
        ))
    );
    let mut too_large = valid;
    too_large
        .args
        .insert("max_bytes".into(), json!(MAX_ATTACHMENT + 1));
    assert!(prepare(&too_large).is_err());
}

#[tokio::test]
async fn only_missing_or_stale_mailbox_sockets_trigger_startup() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
    let path = root.path().join("private-mail.sock");
    let owner = unsafe { libc::geteuid() };
    assert!(connect(&path, owner).await.unwrap().is_none());
    let listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(
        connect(&path, owner).await.is_err(),
        "a same-UID impostor is not a native Mail host"
    );
    drop(listener);
    assert!(connect(&path, owner).await.unwrap().is_none());
    fs::remove_file(&path).unwrap();
    fs::write(&path, "not a socket").unwrap();
    assert!(connect(&path, owner).await.is_err());
}
