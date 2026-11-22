//! Root-owned App confirmations and separately recorded install choices.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::ai::consent::InstallModel;
use crate::apps::permission_review::{PermissionReview, RequestedPermission};
use crate::caps::{Cap, Manifest};
#[cfg(test)]
use crate::caps::Verb;
use crate::provenance::runtime::PackageRef;
use crate::provenance::{PackageKind, TrustTier, VerifiedPackage};

const SCHEMA_VERSION: u32 = 1;
const REVIEW_LIFETIME_SECS: u64 = 30 * 60;
const MAX_PENDING_PER_OWNER: usize = 32;
const MAX_RECORD_BYTES: u64 = 2 * 1024 * 1024;
const MAX_MANIFEST_BYTES: usize = 512 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReviewKind {
    AppInstall,
    AppActivation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReviewState {
    Pending,
    Approved,
    Denied,
    Consumed,
    Expired,
    Stale,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewRecord {
    pub schema_version: u32,
    pub id: String,
    pub owner_uid: u32,
    pub kind: ReviewKind,
    pub source_dir: PathBuf,
    pub package: PackageRef,
    pub manifest: Manifest,
    pub contract_digest: String,
    pub requester: String,
    pub requested_at: u64,
    pub expires_at: u64,
    pub owner_generation: u32,
    pub state: ReviewState,
    pub decided_at: Option<u64>,
    pub decided_by: Option<u32>,
    pub inherited_from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delivery: Option<DeliveryReview>,
    #[serde(default, skip_serializing_if = "super::app_policy::Choices::is_empty")]
    pub(crate) permission_choices: super::app_policy::Choices,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) ai_model: Option<InstallModel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    installed_at_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeSet::is_empty")]
    revoked_ai_choices: std::collections::BTreeSet<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeliveryReview {
    pub digest: String,
    pub packages: Vec<DeliveryPackage>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeliveryPackage {
    pub package: String,
    pub version: String,
    pub architecture: String,
}

impl DeliveryReview {
    fn validate(&self) -> Result<(), String> {
        if !crate::apps::store::valid_sha256(&self.digest)
            || self.packages.is_empty()
            || self.packages.len() > 512
        {
            return Err("invalid bounded App delivery plan".into());
        }
        let mut packages = std::collections::BTreeSet::new();
        for package in &self.packages {
            if [&package.package, &package.version, &package.architecture]
                .iter()
                .any(|value| value.is_empty() || value.len() > 255 || value.chars().any(char::is_control))
                || !packages.insert((&package.package, &package.architecture))
            {
                return Err("invalid or duplicate App delivery package".into());
            }
        }
        Ok(())
    }
}

impl ReviewRecord {
    fn is_official_apt_install(&self) -> bool {
        self.kind == ReviewKind::AppInstall
            && self.delivery.is_some()
            && self.package.tier == TrustTier::Vendor.as_str()
            && self.package.publisher_key_id.is_some()
    }

    pub(crate) fn can_choose_permissions(&self) -> bool {
        self.package.publisher_key_id.is_some() && !self.is_official_apt_install()
    }

    pub(crate) fn is_revoked_ai_choice(&self, id: &str) -> bool {
        self.revoked_ai_choices.contains(id)
    }

    pub(crate) fn ai_choice(
        &self,
        permission: &RequestedPermission,
        ceiling: &crate::provenance::Ceiling,
    ) -> Result<Option<(String, Cap)>, String> {
        let Some(model) = self.ai_model.as_ref().filter(|_| {
            self.kind == ReviewKind::AppInstall
                && self.package.publisher_key_id.is_some()
                && permission.requests_install_ai()
        }) else { return Ok(None); };
        let (cap, _) = super::canonical_capability(permission.verb, model.scope())?;
        if !ceiling.allows_cap(&cap) {
            return Ok(None);
        }
        let key = serde_json::to_vec(&(
            "install-ai", &self.package, &self.contract_digest, model, permission.decision_id()?,
        )).map_err(|error| error.to_string())?;
        Ok(Some((crate::crypto::sha256_hex(&key), cap)))
    }

    fn confirms_official_install(&self, package: &PackageRef) -> Result<bool, String> {
        Ok(self.is_official_apt_install()
            && self.package == *package
            && self.effective_state()? == ReviewState::Consumed)
    }

    pub fn permission_review(&self) -> Result<PermissionReview, String> {
        PermissionReview::from_manifest(&self.manifest)
    }

    pub(crate) fn resolved_permissions(&self) -> Result<PermissionReview, String> {
        let mut review = self.permission_review()?;
        if review.permissions.iter().any(|permission| matches!(
            permission.scope, crate::caps::ScopeBinding::OwnerPath { .. }
        )) {
            let home = crate::paths::verified_home_for_uid(self.owner_uid)?;
            review.resolve_owner_paths(&crate::caps::args::PathContext { home, cwd: None })?;
        }
        Ok(review)
    }

    pub fn effective_state(&self) -> Result<ReviewState, String> {
        if self.owner_generation != current_generation(self.owner_uid, &self.manifest.id)? {
            return Ok(ReviewState::Stale);
        }
        if matches!(self.state, ReviewState::Pending | ReviewState::Approved)
            && now()? >= self.expires_at
        {
            return Ok(ReviewState::Expired);
        }
        if matches!(self.state, ReviewState::Pending | ReviewState::Approved)
            && self.ai_model.is_some() && InstallModel::for_owner(self.owner_uid)? != self.ai_model
        {
            return Ok(ReviewState::Stale);
        }
        Ok(self.state)
    }
}

fn root() -> PathBuf {
    super::root().join("system-reviews")
}

fn record_path(id: &str) -> Result<PathBuf, String> {
    let valid = id.strip_prefix("rv-").is_some_and(|suffix| {
        suffix.len() == 32
            && suffix
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    });
    if !valid {
        return Err("invalid system review id".into());
    }
    Ok(root().join(format!("{id}.json")))
}

fn now() -> Result<u64, String> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|error| format!("system review clock: {error}"))
}

fn current_generation(owner: u32, app_id: &str) -> Result<u32, String> {
    super::generations::current(Some(owner), &format!("system-review:{app_id}"))
}

fn snapshot(package: &VerifiedPackage) -> Result<(Manifest, PermissionReview), String> {
    if package.kind() != PackageKind::App {
        return Err("system App review requires an authenticated App package".into());
    }
    let text = package.manifest_text().map_err(|error| error.to_string())?;
    if text.len() > MAX_MANIFEST_BYTES {
        return Err("App manifest exceeds the system review size limit".into());
    }
    let manifest = Manifest::from_json(&text).map_err(|error| error.to_string())?;
    if manifest.id != package.id() {
        return Err("App manifest identity does not match the authenticated package".into());
    }
    manifest
        .validate_tools_against_catalog(&crate::ai::tools::list_names())
        .map_err(|error| format!("App review catalog check: {error}"))?;
    let review = PermissionReview::from_manifest(&manifest)?;
    Ok((manifest, review))
}

fn save(record: &ReviewRecord) -> Result<(), String> {
    let body =
        serde_json::to_vec(record).map_err(|error| format!("serialize system review: {error}"))?;
    if body.len() as u64 > MAX_RECORD_BYTES {
        return Err("system review exceeds its record size limit".into());
    }
    super::write_atomic_with(
        &record_path(&record.id)?,
        &body,
        super::Durability::Committed,
    )
    .map_err(|error| format!("persist system review: {error}"))
}

#[cfg(unix)]
fn read_record(path: &Path) -> Result<ReviewRecord, String> {
    use std::io::Read;
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};

    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|error| format!("read system review: {error}"))?;
    let metadata = file
        .metadata()
        .map_err(|error| format!("inspect system review: {error}"))?;
    if !metadata.is_file()
        || metadata.nlink() != 1
        || metadata.mode() & 0o077 != 0
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.len() > MAX_RECORD_BYTES
    {
        return Err("system review must be a private, owned, bounded regular file".into());
    }
    let mut body = Vec::new();
    file.take(MAX_RECORD_BYTES + 1)
        .read_to_end(&mut body)
        .map_err(|error| format!("read system review data: {error}"))?;
    if body.len() as u64 > MAX_RECORD_BYTES {
        return Err("system review exceeds its record size limit".into());
    }
    let record: ReviewRecord =
        serde_json::from_slice(&body).map_err(|error| format!("parse system review: {error}"))?;
    if record.schema_version != SCHEMA_VERSION || record_path(&record.id)? != path {
        return Err("system review schema or record identity is invalid".into());
    }
    if record.package.kind != PackageKind::App
        || record.package.id != record.manifest.id
        || record.permission_review()?.contract_digest != record.contract_digest
    {
        return Err("system review does not match its App permission snapshot".into());
    }
    if let Some(delivery) = &record.delivery {
        if record.kind != ReviewKind::AppInstall {
            return Err("delivery plans require an App installation review".into());
        }
        delivery.validate()?;
    }
    if matches!(
        record.state,
        ReviewState::Approved | ReviewState::Denied | ReviewState::Consumed
    ) && (record.decided_by != Some(record.owner_uid) || record.decided_at.is_none())
    {
        return Err("resolved system review has no matching user decision".into());
    }
    Ok(record)
}

#[cfg(not(unix))]
fn read_record(_path: &Path) -> Result<ReviewRecord, String> {
    Err("protected system reviews require a Unix host".into())
}

pub fn get(owner: u32, id: &str) -> Result<ReviewRecord, String> {
    let record = read_record(&record_path(id)?)?;
    if record.owner_uid != owner {
        return Err("system review is not owned by this user".into());
    }
    Ok(record)
}

fn records(owner: u32) -> Result<Vec<ReviewRecord>, String> {
    let entries = match std::fs::read_dir(root()) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("list system reviews: {error}")),
    };
    let mut records = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("read system review entry: {error}"))?;
        let path = entry.path();
        if path.extension() != Some(std::ffi::OsStr::new("json"))
            || path
                .file_name()
                .is_some_and(|name| name.as_encoded_bytes().starts_with(b"."))
        {
            continue;
        }
        let record = read_record(&path)?;
        if record.owner_uid == owner {
            records.push(record);
        }
    }
    records.sort_by_key(|record| (record.requested_at, record.id.clone()));
    Ok(records)
}

pub(crate) fn permission_choices(owner: u32, package: &PackageRef) -> Result<super::app_policy::Choices, String> {
    let mut choices = super::app_policy::Choices::new();
    if package.tier == "system-desktop" && package.id == "desktop-shell" {
        choices.extend(super::generations::legacy_desktop_denials(owner)?);
        let mut previous = Vec::new();
        for record in records(owner)? {
            if record.state == ReviewState::Consumed
                && record.package.id == package.id
                && record.package.tier == TrustTier::Vendor.as_str()
                && record.package.publisher_key_id.is_some()
                && !previous.iter().any(|entry: &PackageRef| {
                    entry.publisher_key_id == record.package.publisher_key_id
                        && entry.tier == record.package.tier
                })
            {
                previous.push(record.package);
            }
        }
        for previous in previous {
            for (permission, allowed) in permission_choices(owner, &previous)? {
                if !allowed {
                    choices.insert(permission, false);
                }
            }
        }
        return Ok(choices);
    }
    if package.publisher_key_id.is_none() {
        return Ok(choices);
    }
    for record in records(owner)?.into_iter().filter(|record| {
        record.state == ReviewState::Consumed
            && record.package.id == package.id
            && record.package.publisher_key_id == package.publisher_key_id
            && record.package.tier == package.tier
    }) {
        for (permission, allowed) in record.permission_choices {
            choices.entry(permission).or_insert(allowed);
        }
    }
    choices.extend(super::app_policy::choices(owner, package)?);
    Ok(choices)
}

pub fn pending(owner: u32, limit: usize) -> Result<Vec<ReviewRecord>, String> {
    if limit == 0 {
        return Ok(Vec::new());
    }
    let mut pending = Vec::new();
    for record in records(owner)? {
        if record.effective_state()? == ReviewState::Pending {
            pending.push(record);
            if pending.len() >= limit {
                break;
            }
        }
    }
    Ok(pending)
}

pub fn submit(
    owner: u32,
    kind: ReviewKind,
    package: &VerifiedPackage,
    requester: String,
) -> Result<ReviewRecord, String> {
    submit_record(owner, kind, package, requester, None, false)
}

pub fn submit_delivery(
    owner: u32,
    package: &VerifiedPackage,
    requester: String,
    delivery: DeliveryReview,
) -> Result<ReviewRecord, String> {
    delivery.validate()?;
    submit_record(owner, ReviewKind::AppInstall, package, requester, Some(delivery), false)
}

pub(crate) fn submit_confirmed_official_install(
    owner: u32,
    package: &VerifiedPackage,
    requester: String,
    delivery: DeliveryReview,
) -> Result<ReviewRecord, String> {
    delivery.validate()?;
    if package.tier() != TrustTier::Vendor || PackageRef::of(package).publisher_key_id.is_none() {
        return Err("The selected release is not signed by a vendor App publisher".into());
    }
    submit_record(owner, ReviewKind::AppInstall, package, requester, Some(delivery), true)
}

fn submit_record(
    owner: u32,
    kind: ReviewKind,
    package: &VerifiedPackage,
    requester: String,
    delivery: Option<DeliveryReview>,
    store_confirmed: bool,
) -> Result<ReviewRecord, String> {
    let (manifest, review) = snapshot(package)?;
    let package_ref = PackageRef::of(package);
    let ai_model = if kind == ReviewKind::AppInstall && package_ref.publisher_key_id.is_some()
        && review.requests_install_ai()
    {
        InstallModel::for_owner(owner)?
    } else {
        None
    };
    // A general Store click can still confirm delivery, never optional AI.
    let store_confirmed = store_confirmed && ai_model.is_none();
    let submitted_at = now()?;
    let expires_at = submitted_at
        .checked_add(REVIEW_LIFETIME_SECS)
        .ok_or("system review deadline overflow")?;
    super::ensure_dirs().map_err(|error| format!("approvals directory: {error}"))?;
    crate::filelock::with_exclusive_path_lock(&super::grant_lock_path(), || {
        let existing = records(owner)?;
        let mut open = 0;
        for record in &existing {
            let state = record.effective_state()?;
            if state == ReviewState::Pending {
                open += 1;
            }
            if matches!(state, ReviewState::Pending | ReviewState::Approved)
                && record.kind == kind
                && record.package == package_ref
                && record.contract_digest == review.contract_digest
                && record.source_dir == package.dir()
                && record.delivery == delivery
                && record.ai_model == ai_model
            {
                if store_confirmed && state == ReviewState::Pending {
                    return Err("An existing protected review cannot be changed by a Store click".into());
                }
                return Ok(record.clone());
            }
        }
        let owner_generation = current_generation(owner, &manifest.id)?;
        let accepted = existing.iter().rev().find(|record| {
            delivery.is_none()
                && ai_model.is_none()
                && record.owner_generation == owner_generation
                && record.state == ReviewState::Consumed
                && record.package.id == package_ref.id
                && record.package.publisher_key_id == package_ref.publisher_key_id
                && record.package.tier == package_ref.tier
                && record.contract_digest == review.contract_digest
        });
        if !store_confirmed && accepted.is_none() && open >= MAX_PENDING_PER_OWNER {
            return Err("too many pending system reviews for this user".into());
        }
        let record = ReviewRecord {
            schema_version: SCHEMA_VERSION,
            id: format!("rv-{}", uuid::Uuid::new_v4().simple()),
            owner_uid: owner,
            kind,
            source_dir: package.dir().to_path_buf(),
            package: package_ref,
            manifest,
            contract_digest: review.contract_digest,
            requester,
            requested_at: submitted_at,
            expires_at,
            owner_generation,
            state: if store_confirmed || accepted.is_some() {
                ReviewState::Approved
            } else {
                ReviewState::Pending
            },
            decided_at: (store_confirmed || accepted.is_some()).then_some(submitted_at),
            decided_by: if store_confirmed { Some(owner) } else { accepted.and_then(|record| record.decided_by) },
            inherited_from: accepted.map(|record| record.id.clone()),
            delivery,
            permission_choices: Default::default(),
            ai_model,
            installed_at_ms: None,
            revoked_ai_choices: Default::default(),
        };
        save(&record)?;
        Ok(record)
    })
}

fn require_package(record: &ReviewRecord, package: &VerifiedPackage) -> Result<(), String> {
    let (_, review) = snapshot(package)?;
    if record.package != PackageRef::of(package) || record.contract_digest != review.contract_digest
    {
        return Err("App package changed after system review; review the current package".into());
    }
    Ok(())
}

/// The caller is the broker after authenticating the privileged OS decision helper.
pub fn decide(
    owner: u32,
    id: &str,
    approve: bool,
    package: Option<&VerifiedPackage>,
) -> Result<ReviewRecord, String> {
    decide_with_permissions(owner, id, approve, package, Default::default())
}

pub(crate) fn decide_with_permissions(
    owner: u32,
    id: &str,
    approve: bool,
    package: Option<&VerifiedPackage>,
    choices: super::app_policy::Choices,
) -> Result<ReviewRecord, String> {
    super::ensure_dirs().map_err(|error| format!("approvals directory: {error}"))?;
    crate::filelock::with_exclusive_path_lock(&super::grant_lock_path(), || {
        let mut record = get(owner, id)?;
        if record.effective_state()? != ReviewState::Pending {
            return Err("system review is no longer pending; refresh its state".into());
        }
        if approve {
            let package = package.ok_or("App review requires fresh package verification")?;
            require_package(&record, package)?;
            let previous = permission_choices(owner, &record.package)?;
            let mut required = std::collections::BTreeSet::new();
            let ceiling = package.ceiling();
            for permission in record.resolved_permissions()?.permissions {
                if let Some((id, _)) = record.ai_choice(&permission, &ceiling)? {
                    required.insert(id);
                } else if record.can_choose_permissions() && permission.persistent_cap(&ceiling).is_some() {
                    let id = permission.decision_id()?;
                    if !previous.contains_key(&id) { required.insert(id); }
                }
            }
            if !required.iter().eq(choices.keys()) {
                return Err("Choose Allow or Don't allow for each new permission shown, then confirm the app.".into());
            }
            record.permission_choices = choices;
        }
        record.state = if approve {
            ReviewState::Approved
        } else {
            ReviewState::Denied
        };
        let decided_at = now()?;
        record.decided_at = Some(decided_at);
        if approve {
            record.expires_at = decided_at
                .checked_add(REVIEW_LIFETIME_SECS)
                .ok_or("system review deadline overflow")?;
        }
        record.decided_by = Some(owner);
        save(&record)?;
        Ok(record)
    })
}

/// Called only after the installed bytes and transaction outcome were verified.
/// The pre-publication consume step alone is deliberately not enough.
pub(crate) fn complete_install(
    owner: u32,
    id: &str,
    package: &PackageRef,
    delivery: Option<&str>,
) -> Result<(), String> {
    let review = get(owner, id)?;
    if review.ai_model.is_none() && !review.permission_review()?.requests_install_ai() {
        return Ok(());
    }
    crate::filelock::with_exclusive_path_lock(&super::grant_lock_path(), || {
        let mut review = get(owner, id)?;
        if review.kind != ReviewKind::AppInstall
            || &review.package != package
            || review.delivery.as_ref().map(|plan| plan.digest.as_str()) != delivery
        {
            return Err("Installed App differs from its protected AI choice".into());
        }
        if review.installed_at_ms.is_none()
            && review.effective_state()? == ReviewState::Consumed
            && now()? < review.expires_at
            && (review.ai_model.is_none() || InstallModel::for_owner(owner)? == review.ai_model)
        {
            review.installed_at_ms = Some(crate::agentd::grant::now_ms());
            save(&review)?;
        }
        Ok(())
    })
}

pub(crate) fn installed_gui_ai(
    owner: u32,
    package: &VerifiedPackage,
    model: &InstallModel,
) -> Result<Vec<(String, Cap)>, String> {
    let (_, disclosure) = snapshot(package)?;
    if !disclosure.requests_install_ai() {
        return Ok(Vec::new());
    }
    let Some(review) = latest_installed_ai(owner, package, model, &disclosure.contract_digest)? else {
        return Ok(Vec::new());
    };
    let ceiling = package.ceiling();
    let mut allowed = Vec::new();
    for permission in &disclosure.permissions {
        if let Some((id, cap)) = review.ai_choice(permission, &ceiling)? {
            if review.permission_choices.get(&id) == Some(&true) && !review.is_revoked_ai_choice(&id) {
                allowed.push((id, cap));
            }
        }
    }
    Ok(allowed)
}

fn latest_installed_ai(
    owner: u32,
    package: &VerifiedPackage,
    model: &InstallModel,
    contract: &str,
) -> Result<Option<ReviewRecord>, String> {
    let package_ref = PackageRef::of(package);
    let generation = current_generation(owner, package.id())?;
    let latest = records(owner)?.into_iter().filter(|review| {
        review.kind == ReviewKind::AppInstall
            && review.package == package_ref
            && review.contract_digest == contract
            && review.state == ReviewState::Consumed
            && review.owner_generation == generation
            && review.installed_at_ms.is_some()
    }).max_by_key(|review| (review.installed_at_ms, review.id.clone()));
    Ok(latest.filter(|review| review.ai_model.as_ref() == Some(model)))
}

#[cfg(test)]
pub(crate) fn revoke_installed_gui_ai(
    owner: u32, package: &VerifiedPackage, model: &InstallModel, id: &str,
) -> Result<(), String> {
    let (_, disclosure) = snapshot(package)?;
    crate::filelock::with_exclusive_path_lock(&super::grant_lock_path(), || {
        let mut review = latest_installed_ai(owner, package, model, &disclosure.contract_digest)?
            .ok_or("The installed AI choice is no longer current")?;
        if review.permission_choices.get(id) != Some(&true)
            || !disclosure.permissions.iter().filter(|permission| permission.requests_install_ai())
                .any(|permission| review.ai_choice(permission, &package.ceiling())
                    .is_ok_and(|choice| choice.is_some_and(|(choice, _)| choice == id)))
        {
            return Err("The installed AI choice is no longer current".into());
        }
        review.revoked_ai_choices.insert(id.to_string());
        save(&review)
    })
}

#[cfg(test)]
pub(crate) fn has_install_gui_ai(
    owner: u32, package: &VerifiedPackage, model: &InstallModel, verb: Verb,
) -> Result<bool, String> {
    Ok(installed_gui_ai(owner, package, model)?.iter().any(|(_, cap)| cap.verb == verb))
}

pub fn consume(owner: u32, id: &str, package: &VerifiedPackage) -> Result<ReviewRecord, String> {
    consume_with_delivery(owner, id, package, None)
}

pub fn consume_delivery(
    owner: u32,
    id: &str,
    package: &VerifiedPackage,
    digest: &str,
) -> Result<ReviewRecord, String> {
    consume_with_delivery(owner, id, package, Some(digest))
}

fn consume_with_delivery(
    owner: u32,
    id: &str,
    package: &VerifiedPackage,
    digest: Option<&str>,
) -> Result<ReviewRecord, String> {
    super::ensure_dirs().map_err(|error| format!("approvals directory: {error}"))?;
    crate::filelock::with_exclusive_path_lock(&super::grant_lock_path(), || {
        let mut record = get(owner, id)?;
        if record.delivery.as_ref().map(|plan| plan.digest.as_str()) != digest {
            return Err("App delivery plan differs from the owner's protected confirmation".into());
        }
        if record.effective_state()? != ReviewState::Approved {
            return Err("App installation requires an approved, current system review".into());
        }
        require_package(&record, package)?;
        record.state = ReviewState::Consumed;
        save(&record)?;
        Ok(record)
    })
}

pub fn invalidate(owner: u32, id: &str) -> Result<(), String> {
    super::ensure_dirs().map_err(|error| format!("approvals directory: {error}"))?;
    crate::filelock::with_exclusive_path_lock(&super::grant_lock_path(), || {
        let mut record = get(owner, id)?;
        if matches!(record.state, ReviewState::Pending | ReviewState::Approved) {
            record.state = ReviewState::Stale;
            save(&record)?;
        }

        Ok(())
    })
}

pub fn has_accepted(owner: u32, package: &VerifiedPackage) -> Result<bool, String> {
    let (manifest, review) = snapshot(package)?;
    let package_ref = PackageRef::of(package);
    let generation = current_generation(owner, &manifest.id)?;
    Ok(records(owner)?.iter().any(|record| {
        record.owner_generation == generation
            && record.state == ReviewState::Consumed
            && record.package.id == package_ref.id
            && record.package.publisher_key_id == package_ref.publisher_key_id
            && record.package.tier == package_ref.tier
            && record.contract_digest == review.contract_digest
    }))
}

pub(crate) fn official_installation(
    owner: u32,
    package: &VerifiedPackage,
) -> Result<Option<String>, String> {
    let package = PackageRef::of(package);
    if package.tier != TrustTier::Vendor.as_str() || package.publisher_key_id.is_none() {
        return Ok(None);
    }
    for record in records(owner)?.into_iter().rev() {
        if record.confirms_official_install(&package)? {
            return Ok(Some(record.id));
        }
    }
    Ok(None)
}

pub(crate) fn official_installation_is_current(
    owner: u32,
    id: &str,
    package: &VerifiedPackage,
) -> Result<bool, String> {
    get(owner, id)?.confirms_official_install(&PackageRef::of(package))
}

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/test/unit/approvals/system_review.rs"
    ));
}
