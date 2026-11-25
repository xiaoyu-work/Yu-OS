//! Default permissions for exact App builds selected by the OS packages.

use std::path::Path;

use crate::caps::Cap;
use crate::provenance::VerifiedPackage;

pub(super) enum Defaults {
    Preinstalled,
    #[cfg(target_os = "linux")]
    OfficialDesktop(String),
}

impl Defaults {
    pub(super) fn allows(&self, cap: &Cap, home: &Path) -> bool {
        allows(cap, home)
    }

    #[cfg(target_os = "linux")]
    pub(super) fn is_current(&self, owner: u32, package: &VerifiedPackage) -> Result<bool, String> {
        match self {
            Self::Preinstalled => Ok(listed(package)),
            Self::OfficialDesktop(id) => crate::approvals::system_review::official_installation_is_current(
                owner, id, package,
            ),
        }
    }
}

pub(super) fn allows(cap: &Cap, _home: &Path) -> bool {
    crate::caps::lookup_meta(cap.verb).is_some()
}

#[cfg(target_os = "linux")]
pub(super) fn listed(package: &VerifiedPackage) -> bool {
    use crate::provenance::{PackageKind, TrustSource, TrustTier};

    if package.kind() != PackageKind::App
        || package.tier() != TrustTier::Vendor
        || !matches!(package.source(), TrustSource::Publisher { .. })
        || package.envelope_sha256().is_none()
    {
        return false;
    }
    let Ok(relative) = package.dir().strip_prefix("/usr/lib/yuos/apps") else {
        return false;
    };
    relative
        .to_str()
        .is_some_and(|path| !path.is_empty() && !path.contains('/'))
}

#[cfg(target_os = "linux")]
pub(super) fn approved(package: &VerifiedPackage) -> bool {
    listed(package) && protected_app(package)
}

#[cfg(target_os = "linux")]
pub(super) fn official_desktop(owner: u32, package: &VerifiedPackage) -> Result<Option<Defaults>, String> {
    let Some(id) = crate::approvals::system_review::official_installation(owner, package)? else {
        return Ok(None);
    };
    Ok((package.dir().starts_with("/usr/lib/yuos/apps") && protected_app(package))
        .then_some(Defaults::OfficialDesktop(id)))
}

#[cfg(target_os = "linux")]
pub(super) fn protected_app(package: &VerifiedPackage) -> bool {
    use crate::provenance::{envelope::ENVELOPE_FILE, fsec};

    fsec::require_secure_location(package.dir(), &[0]).is_ok()
        && package
            .files()
            .map(|file| file.path.as_str())
            .chain([ENVELOPE_FILE])
            .all(|path| fsec::lstat(&package.dir().join(path)).is_ok_and(|node| {
                node.uid == 0 && !node.is_symlink && !node.is_group_or_world_writable()
            }))
        && package.assert_current(&crate::provenance::trust_store()).is_ok()
}

#[cfg(not(target_os = "linux"))]
pub(super) fn approved(_package: &VerifiedPackage) -> bool {
    false
}
