//! Release-manifest signature verification.
//!
//! # Which key
//!
//! Yu OS already distributes exactly one publisher identity to every
//! installed system: the OpenPGP key APT pins with
//! `signed-by=/usr/share/keyrings/yuos-archive-keyring.gpg`. The
//! release-security manifest is signed by that same identity, so a
//! machine that can authenticate its package index can authenticate
//! its release manifest with no second trust root to ship, rotate or
//! lose. Operators who rebuild privately drop their own binary keyring
//! into [`OPERATOR_KEYRING_DIR`](super::OPERATOR_KEYRING_DIR).
//!
//! **No key material is generated here and none is shipped in the
//! repository.** When a machine has no release keyring at all — a
//! developer tree, an image composed without the `apt-source` feature
//! — signature verification cannot succeed, and
//! [`verify_detached`] reports [`Signature::Unverifiable`] rather than
//! inventing a placeholder. [`super::decide`] then fails closed for
//! any system that has previously recorded a trusted key, and records
//! the decision as developer-trust for one that never had one. The
//! recovery path is to import the real keyring, not to relax the
//! check.
//!
//! # How
//!
//! Verification shells out to `gpgv`, the minimal verify-only OpenPGP
//! tool that is already a dependency of every Debian/Ubuntu system
//! (`apt` uses it for `InRelease`). Using it rather than a bundled
//! implementation keeps one OpenPGP parser on the machine and keeps
//! this crate free of private-key handling entirely.

use std::path::{Path, PathBuf};
use std::process::Command;

use super::OPERATOR_KEYRING_DIR;

/// What is known about a manifest's signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Signature {
    /// A good signature from `key_id`, made with `keyring`.
    Verified { key_id: String, keyring: PathBuf },
    /// No detached signature accompanied the manifest.
    Absent,
    /// A signature exists but this machine cannot check it: no
    /// keyring, or no `gpgv`. Never treated as valid.
    Unverifiable { reason: String },
}

impl Signature {
    pub fn key_id(&self) -> Option<&str> {
        match self {
            Self::Verified { key_id, .. } => Some(key_id.as_str()),
            _ => None,
        }
    }

    pub fn is_verified(&self) -> bool {
        matches!(self, Self::Verified { .. })
    }
}

/// Normalize an OpenPGP long key id or fingerprint to uppercase hex.
pub fn normalize_key_id(raw: &str) -> Result<String, String> {
    let cleaned = raw
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>();
    if cleaned.len() < 16 || cleaned.len() > 64 || !cleaned.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!("`{raw}` is not an OpenPGP key id or fingerprint"));
    }
    Ok(cleaned.to_ascii_uppercase())
}

/// Keyrings to try, in order: the operator roots first (so a rotation
/// can be introduced without waiting for a package), then the keyring
/// APT itself trusts.
pub fn keyrings(root: &Path, apt_keyring: &str) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let operator_dir = joined(root, OPERATOR_KEYRING_DIR);
    if let Ok(entries) = std::fs::read_dir(&operator_dir) {
        let mut paths = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.is_file()
                    && path
                        .extension()
                        .is_some_and(|ext| ext == "gpg" || ext == "kbx")
            })
            .collect::<Vec<_>>();
        paths.sort();
        found.extend(paths);
    }
    let apt = joined(root, apt_keyring);
    if apt.is_file() {
        found.push(apt);
    }
    found
}

/// Join an absolute installed path onto an alternate root.
pub fn joined(root: &Path, absolute: &str) -> PathBuf {
    let relative = absolute.trim_start_matches('/');
    root.join(relative)
}

/// Verify `signature_path` over `document_path`.
///
/// Returns [`Signature::Verified`] only for a `gpgv` exit status of 0
/// together with a `VALIDSIG`/`GOODSIG` status line, so a warning-only
/// run or an expired-key run cannot be mistaken for success.
pub fn verify_detached(
    document_path: &Path,
    signature_path: &Path,
    keyrings: &[PathBuf],
) -> Signature {
    if !signature_path.exists() {
        return Signature::Absent;
    }
    if keyrings.is_empty() {
        return Signature::Unverifiable {
            reason: "no Yu OS release keyring is installed".to_string(),
        };
    }
    let Ok(gpgv) = which("gpgv") else {
        return Signature::Unverifiable {
            reason: "gpgv is not installed".to_string(),
        };
    };
    let mut last_reason = "signature did not verify against any installed keyring".to_string();
    for keyring in keyrings {
        let output = Command::new(&gpgv)
            .arg("--status-fd")
            .arg("1")
            .arg("--keyring")
            .arg(keyring)
            .arg(signature_path)
            .arg(document_path)
            .output();
        let Ok(output) = output else {
            last_reason = "gpgv could not be executed".to_string();
            continue;
        };
        if !output.status.success() {
            continue;
        }
        let status = String::from_utf8_lossy(&output.stdout);
        if let Some(key_id) = good_signature_key(&status) {
            return Signature::Verified {
                key_id,
                keyring: keyring.clone(),
            };
        }
        last_reason = "gpgv reported success without a good signature".to_string();
    }
    Signature::Unverifiable {
        reason: last_reason,
    }
}

/// Verify pinned release files without reopening their installed names.
#[cfg(target_os = "linux")]
pub(crate) struct PinnedKeyring {
    pub(crate) path: PathBuf,
    pub(crate) file: crate::provenance::fsec::VerifiedFd,
}

#[cfg(target_os = "linux")]
pub(crate) fn verify_detached_fds(
    document: &crate::provenance::fsec::VerifiedFd,
    signature: &crate::provenance::fsec::VerifiedFd,
    keyrings: &[PinnedKeyring],
) -> Signature {
    use std::os::unix::process::CommandExt;

    if keyrings.is_empty() {
        return Signature::Unverifiable {
            reason: "no Yu OS release keyring is installed".to_string(),
        };
    }
    let Ok(gpgv) = which("gpgv") else {
        return Signature::Unverifiable {
            reason: "gpgv is not installed".to_string(),
        };
    };
    let Ok(meta) = crate::provenance::fsec::require_secure_location(&gpgv, &[0]) else {
        return Signature::Unverifiable {
            reason: "gpgv is not installed in a protected location".to_string(),
        };
    };
    if !meta.is_file || meta.nlink != 1 || meta.mode & 0o111 == 0 {
        return Signature::Unverifiable {
            reason: "gpgv is not a protected regular executable".to_string(),
        };
    }
    let document_fd = document.as_raw_fd();
    let signature_fd = signature.as_raw_fd();
    let mut last_reason = "signature did not verify against any installed keyring".to_string();
    for keyring in keyrings {
        let keyring_fd = keyring.file.as_raw_fd();
        let mut command = Command::new(&gpgv);
        command
            .env_clear()
            .env("LC_ALL", "C")
            .arg("--status-fd")
            .arg("1")
            .arg("--keyring")
            .arg(format!("/proc/self/fd/{keyring_fd}"))
            .arg(format!("/proc/self/fd/{signature_fd}"))
            .arg(format!("/proc/self/fd/{document_fd}"));
        unsafe {
            command.pre_exec(move || {
                for fd in [document_fd, signature_fd, keyring_fd] {
                    if libc::fcntl(fd, libc::F_SETFD, 0) < 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                }
                Ok(())
            });
        }
        let output = match command.output() {
            Ok(output) => output,
            Err(error) => {
                last_reason = format!("cannot execute gpgv: {error}");
                continue;
            }
        };
        if output.status.success() {
            let status = String::from_utf8_lossy(&output.stdout);
            if let Some(key_id) = good_signature_key(&status) {
                return Signature::Verified {
                    key_id,
                    keyring: keyring.path.clone(),
                };
            }
            last_reason = "gpgv reported success without a good signature".to_string();
        }
    }
    Signature::Unverifiable {
        reason: last_reason,
    }
}

/// Pull the signing key out of `gpgv --status-fd` output. `VALIDSIG`
/// carries the full fingerprint and is preferred; `GOODSIG` carries
/// the long key id.
fn good_signature_key(status: &str) -> Option<String> {
    let mut good = None;
    for line in status.lines() {
        let Some(rest) = line.strip_prefix("[GNUPG:] ") else {
            continue;
        };
        let mut fields = rest.split_whitespace();
        match fields.next() {
            Some("VALIDSIG") => {
                if let Some(fingerprint) = fields.next() {
                    if let Ok(normalized) = normalize_key_id(fingerprint) {
                        return Some(normalized);
                    }
                }
            }
            Some("GOODSIG") => {
                if let Some(key_id) = fields.next() {
                    if let Ok(normalized) = normalize_key_id(key_id) {
                        good = Some(normalized);
                    }
                }
            }
            _ => {}
        }
    }
    good
}

fn which(program: &str) -> Result<PathBuf, ()> {
    for dir in ["/usr/bin", "/bin", "/usr/local/bin"] {
        let candidate = Path::new(dir).join(program);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(())
}

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/test/unit/update/signature.rs"
    ));
}
