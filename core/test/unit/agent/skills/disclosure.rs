use super::*;
use std::fs;
use tempfile::tempdir;

fn write_skill(root: &Path, id: &str, description: &str, body: &str) {
    let dir = root.join(id);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("SKILL.md"),
        format!("---\nname: {id}\ndescription: {description}\n---\n{body}\n"),
    )
    .unwrap();
    crate::test_env::sign_test_package(&dir, crate::provenance::PackageKind::Skill, id);
}

/// Add a child resource to an already-written skill and re-sign it.
fn reseal(root: &Path, id: &str) {
    let dir = root.join(id);
    fs::remove_file(dir.join(crate::provenance::envelope::ENVELOPE_FILE)).ok();
    crate::test_env::sign_test_package(&dir, crate::provenance::PackageKind::Skill, id);
}

fn add_resource(root: &Path, id: &str, name: &str, body: &str) {
    let dir = root.join(id);
    let target = dir.join(name);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(target, body).unwrap();
    fs::remove_file(dir.join(crate::provenance::envelope::ENVELOPE_FILE)).ok();
    crate::test_env::sign_test_package(&dir, crate::provenance::PackageKind::Skill, id);
}

fn layered<'a>(system: &'a Path, user: &'a Path) -> super::super::loader::LoadResult {
    super::super::loader::load_layered(system, user, &super::super::loader::LoadOptions::default())
}

#[test]
fn prompt_catalog_contains_metadata_but_not_skill_body() {
    let system = tempdir().unwrap();
    let user = tempdir().unwrap();
    write_skill(
        system.path(),
        "yuos",
        "Operate the Claw system layer",
        "SECRET_INSTRUCTION_BODY",
    );
    let load = layered(system.path(), user.path());

    let catalog = render_prompt_catalog(&load).expect("catalog");

    assert!(catalog.contains("\"id\": \"yuos\""));
    assert!(catalog.contains("Operate the Claw system layer"));
    assert!(catalog.contains("\"source\": \"builtin\""));
    assert!(catalog.contains("command=read"));
    assert!(catalog.contains("command=resource"));
    assert!(!catalog.contains("SECRET_INSTRUCTION_BODY"));
}

#[test]
fn instruction_disclosure_returns_body_and_resource_names_only() {
    let system = tempdir().unwrap();
    let user = tempdir().unwrap();
    write_skill(
        system.path(),
        "yuos",
        "Operate the Claw system layer",
        "Use the matching playbook.",
    );
    add_resource(system.path(), "yuos", "diagnostics.md", "PRIVATE_RESOURCE_CONTENT");
    let load = layered(system.path(), user.path());

    let value = disclose_instructions(&load.skills["yuos"]).expect("instructions");

    assert_eq!(value["disclosure_level"], "instructions");
    assert!(value["instructions"]
        .as_str()
        .is_some_and(|body| body.contains("matching playbook")));
    assert_eq!(value["resources"][0]["path"], "diagnostics.md");
    assert!(!value.to_string().contains("PRIVATE_RESOURCE_CONTENT"));
}

#[test]
fn resource_disclosure_reads_one_requested_file() {
    let system = tempdir().unwrap();
    let user = tempdir().unwrap();
    write_skill(system.path(), "yuos", "Claw", "Read details as needed.");
    add_resource(
        system.path(),
        "yuos",
        "playbooks/network.md",
        "NETWORK_PLAYBOOK",
    );
    let load = layered(system.path(), user.path());

    let value =
        disclose_resource(&load.skills["yuos"], "playbooks/network.md").expect("resource");

    assert_eq!(value["disclosure_level"], "resource");
    assert_eq!(value["path"], "playbooks/network.md");
    assert_eq!(value["content"], "NETWORK_PLAYBOOK");
}

#[test]
fn resource_disclosure_rejects_escape_and_absolute_paths() {
    let system = tempdir().unwrap();
    let user = tempdir().unwrap();
    write_skill(system.path(), "yuos", "Claw", "Body");
    let skill = &layered(system.path(), user.path()).skills["yuos"];

    assert!(disclose_resource(skill, "../secret").is_err());
    assert!(disclose_resource(skill, "/etc/passwd").is_err());
    assert!(disclose_resource(skill, "SKILL.md").is_err());
    assert!(disclose_resource(skill, ".env").is_err());
    assert!(disclose_resource(skill, ".git/config").is_err());
}

#[cfg(unix)]
#[test]
fn resource_disclosure_rejects_symlinks() {
    use std::os::unix::fs::symlink;

    let system = tempdir().unwrap();
    let user = tempdir().unwrap();
    let outside = tempdir().unwrap();
    write_skill(system.path(), "yuos", "Claw", "Body");
    fs::write(outside.path().join("secret.md"), "SECRET").unwrap();
    symlink(
        outside.path().join("secret.md"),
        system.path().join("yuos").join("linked.md"),
    )
    .unwrap();
    let load = layered(system.path(), user.path());

    // A symlink dropped into a sealed package is not something the
    // disclosure layer has to reason about: the package no longer
    // verifies at all, so the skill is quarantined before any resource
    // can be requested.
    assert!(!load.skills.contains_key("yuos"));
    assert!(load.errors["yuos"].contains("symlink"));
}

#[test]
fn builtin_skill_skips_third_party_guard() {
    let system = tempdir().unwrap();
    let user = tempdir().unwrap();
    write_skill(system.path(), "yuos", "Claw", "Body");
    fs::File::create(system.path().join("yuos").join("large.bin"))
        .unwrap()
        .set_len(6 * 1024 * 1024)
        .unwrap();
    reseal(system.path(), "yuos");
    let load = layered(system.path(), user.path());

    assert!(disclose_instructions(&load.skills["yuos"]).is_ok());
}

#[test]
fn user_skill_still_uses_third_party_guard() {
    let system = tempdir().unwrap();
    let user = tempdir().unwrap();
    write_skill(user.path(), "community", "Community", "Body");
    fs::File::create(user.path().join("community").join("large.bin"))
        .unwrap()
        .set_len(6 * 1024 * 1024)
        .unwrap();
    reseal(user.path(), "community");
    let load = layered(system.path(), user.path());

    let error = disclose_instructions(&load.skills["community"]).expect_err("guard should block");
    assert!(error.contains("explicit operator review"));
}

#[test]
fn oversized_instruction_body_is_rejected() {
    let system = tempdir().unwrap();
    let user = tempdir().unwrap();
    write_skill(
        system.path(),
        "oversized",
        "Large",
        &"x".repeat(MAX_INSTRUCTION_BYTES + 1),
    );
    let load = layered(system.path(), user.path());

    let metadata = catalog_page(&load, 0, 1);
    assert_eq!(metadata[0]["disclosable"], false);
    let error =
        disclose_instructions(&load.skills["oversized"]).expect_err("body cap should apply");

    assert!(error.contains("exceeds disclosure cap"));
}
