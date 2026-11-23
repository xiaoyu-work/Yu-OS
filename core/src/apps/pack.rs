//! Package an App directory into a clean Debian `.deb` archive for Store submission.
//!
//! The generated `.deb` installs the App tree under `usr/lib/yuos/apps/<id>` with
//! `uid=0, gid=0`, safe file modes, and a `control` file—never maintainer scripts
//! (`postinst`/`prerm`) or setuid bits. Store review inspects and signs the
//! package before publication.

use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use flate2::write::GzEncoder;
use flate2::Compression;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::{lint::app_lint_violations, AppManifest};

const FIXED_MTIME: u64 = 1_700_000_000;

pub(crate) fn pack_app_deb(
    source_dir: &Path,
    out_path: Option<&Path>,
    architecture: &str,
) -> Result<Value, String> {
    if !matches!(architecture, "all" | "amd64" | "arm64") {
        return Err(format!(
            "unsupported architecture `{architecture}`: expected all, amd64, or arm64"
        ));
    }
    if !source_dir.is_dir() {
        return Err(format!(
            "pack source `{}` is not a directory",
            source_dir.display()
        ));
    }
    let manifest_path = source_dir.join("app.json");
    if !manifest_path.is_file() {
        return Err(format!(
            "pack source `{}` has no app.json",
            source_dir.display()
        ));
    }
    let raw_manifest = fs::read_to_string(&manifest_path)
        .map_err(|e| format!("read {}: {e}", manifest_path.display()))?;
    let manifest = AppManifest::from_json(&raw_manifest)
        .map_err(|e| format!("parse {}: {e}", manifest_path.display()))?;
    let catalog = crate::ai::tools::list_names();
    manifest
        .validate_tools_against_catalog(&catalog)
        .map_err(|e| format!("manifest catalog check: {e}"))?;

    let app_probe = super::App {
        manifest: manifest.clone(),
        dir: source_dir.to_path_buf(),
        provenance: Err("unverified local development source".into()),
    };
    let violations = app_lint_violations(&app_probe);
    if !violations.is_empty() {
        return Err(format!(
            "App failed lint checks: {}",
            serde_json::to_string(&violations).unwrap_or_default()
        ));
    }

    let exec_entries = declared_entrypoints(&manifest);
    for entry in &exec_entries {
        let path = source_dir.join(entry);
        let meta = fs::symlink_metadata(&path)
            .map_err(|_| format!("declared entrypoint `{entry}` does not exist in {}", source_dir.display()))?;
        if !meta.is_file() {
            return Err(format!("declared entrypoint `{entry}` must be a regular file"));
        }
    }

    let mut files: Vec<(String, Vec<u8>, u32)> = Vec::new();
    let mut subdirs: BTreeSet<String> = BTreeSet::new();
    collect_files(source_dir, source_dir, &exec_entries, &mut subdirs, &mut files)?;

    let app_id = &manifest.id;
    let package_name = format!("yuos-app-{app_id}");
    let summary_raw = manifest.summary.en_str();
    let summary = if summary_raw.is_empty() {
        manifest.name.en_str()
    } else {
        summary_raw
    }
    .replace(['\r', '\n'], " ");

    let control_text = format!(
        "Package: {package_name}\n\
         Version: {}\n\
         Section: utils\n\
         Priority: optional\n\
         Architecture: {architecture}\n\
         Depends: yuos-app-runtime-v1 (>= 0.1.0)\n\
         Maintainer: Yu OS Developer <dev@yuos.local>\n\
         X-YuOS-App-Ids: {app_id}\n\
         Description: {summary}\n",
        manifest.version
    );

    let control_tar_gz = build_control_tar_gz(control_text.as_bytes())?;
    let data_tar_gz = build_data_tar_gz(app_id, &subdirs, &files)?;
    let deb_bytes = build_ar_deb(&control_tar_gz, &data_tar_gz)?;

    let default_name = format!("{package_name}_{}_{architecture}.deb", manifest.version);
    let destination: PathBuf = out_path
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(&default_name));
    if let Some(parent) = destination.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("create {}: {e}", parent.display()))?;
        }
    }
    fs::write(&destination, &deb_bytes)
        .map_err(|e| format!("write {}: {e}", destination.display()))?;

    let sha256 = hex::encode(Sha256::digest(&deb_bytes));
    Ok(json!({
        "app": app_id,
        "package": package_name,
        "version": manifest.version,
        "architecture": architecture,
        "deb": destination.display().to_string(),
        "size": deb_bytes.len(),
        "sha256": sha256,
    }))
}

fn declared_entrypoints(manifest: &AppManifest) -> BTreeSet<String> {
    let mut entries = BTreeSet::new();
    if !manifest.operations.is_empty() || manifest.desktop.is_some() {
        if let Some(entry) = &manifest.entry {
            entries.insert(entry.clone());
        } else if !manifest.operations.is_empty()
            || manifest
                .desktop
                .as_ref()
                .is_some_and(|d| d.entry.is_none())
        {
            entries.insert(manifest.runtime.default_entry().to_string());
        }
    }
    if let Some(desktop) = &manifest.desktop {
        if let Some(entry) = &desktop.entry {
            entries.insert(entry.clone());
        }
        for extra in desktop.entries.values() {
            entries.insert(extra.entry.clone());
        }
    }
    if let Some(mcp) = &manifest.mcp {
        entries.insert(
            mcp.entry
                .clone()
                .unwrap_or_else(|| manifest.runtime.default_mcp_entry().to_string()),
        );
    }
    entries
}

fn should_skip(name: &str) -> bool {
    matches!(
        name,
        ".git" | "__pycache__" | ".pytest_cache" | ".DS_Store" | ".provenance.json"
    ) || name.ends_with(".pyc")
        || name.ends_with(".deb")
}

fn collect_files(
    root: &Path,
    current: &Path,
    exec_entries: &BTreeSet<String>,
    subdirs: &mut BTreeSet<String>,
    files: &mut Vec<(String, Vec<u8>, u32)>,
) -> Result<(), String> {
    let mut entries: Vec<_> = fs::read_dir(current)
        .map_err(|e| format!("read {}: {e}", current.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("read {}: {e}", current.display()))?;
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| "App file names must be valid UTF-8".to_string())?;
        if should_skip(&name) {
            continue;
        }
        let path = entry.path();
        let meta = fs::symlink_metadata(&path)
            .map_err(|e| format!("inspect {}: {e}", path.display()))?;
        let rel = path
            .strip_prefix(root)
            .map_err(|e| e.to_string())?
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/");
        if meta.file_type().is_symlink() {
            return Err(format!("App package cannot contain symlinks: {rel}"));
        }
        if meta.is_dir() {
            subdirs.insert(rel);
            collect_files(root, &path, exec_entries, subdirs, files)?;
        } else if meta.is_file() {
            let data = fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
            let is_exec = exec_entries.contains(&rel) || data.starts_with(b"#!") || data.starts_with(b"\x7fELF");
            let mode = if is_exec { 0o755 } else { 0o644 };
            files.push((rel, data, mode));
        } else {
            return Err(format!("App package contains a non-regular file: {rel}"));
        }
    }
    Ok(())
}

fn append_dir<W: Write>(builder: &mut tar::Builder<W>, path: &str) -> Result<(), String> {
    let mut header = tar::Header::new_gnu();
    header.set_entry_type(tar::EntryType::Directory);
    header.set_mode(0o755);
    header.set_uid(0);
    header.set_gid(0);
    header.set_username("root").map_err(|e| e.to_string())?;
    header.set_groupname("root").map_err(|e| e.to_string())?;
    header.set_mtime(FIXED_MTIME);
    header.set_size(0);
    header.set_cksum();
    builder
        .append_data(&mut header, path, std::io::empty())
        .map_err(|e| format!("tar dir {path}: {e}"))
}

fn append_file<W: Write>(
    builder: &mut tar::Builder<W>,
    path: &str,
    bytes: &[u8],
    mode: u32,
) -> Result<(), String> {
    let mut header = tar::Header::new_gnu();
    header.set_entry_type(tar::EntryType::Regular);
    header.set_mode(mode);
    header.set_uid(0);
    header.set_gid(0);
    header.set_username("root").map_err(|e| e.to_string())?;
    header.set_groupname("root").map_err(|e| e.to_string())?;
    header.set_mtime(FIXED_MTIME);
    header.set_size(bytes.len() as u64);
    header.set_cksum();
    builder
        .append_data(&mut header, path, bytes)
        .map_err(|e| format!("tar file {path}: {e}"))
}

fn build_control_tar_gz(control_bytes: &[u8]) -> Result<Vec<u8>, String> {
    let encoder = GzEncoder::new(Vec::new(), Compression::default());
    let mut builder = tar::Builder::new(encoder);
    append_dir(&mut builder, "./")?;
    append_file(&mut builder, "./control", control_bytes, 0o644)?;
    let encoder = builder.into_inner().map_err(|e| e.to_string())?;
    encoder.finish().map_err(|e| e.to_string())
}

fn build_data_tar_gz(
    app_id: &str,
    subdirs: &BTreeSet<String>,
    files: &[(String, Vec<u8>, u32)],
) -> Result<Vec<u8>, String> {
    let encoder = GzEncoder::new(Vec::new(), Compression::default());
    let mut builder = tar::Builder::new(encoder);
    let app_prefix = format!("./usr/lib/yuos/apps/{app_id}");
    for dir in [
        "./",
        "./usr/",
        "./usr/lib/",
        "./usr/lib/yuos/",
        "./usr/lib/yuos/apps/",
    ] {
        append_dir(&mut builder, dir)?;
    }
    append_dir(&mut builder, &format!("{app_prefix}/"))?;
    for subdir in subdirs {
        append_dir(&mut builder, &format!("{app_prefix}/{subdir}/"))?;
    }
    for (rel, bytes, mode) in files {
        append_file(&mut builder, &format!("{app_prefix}/{rel}"), bytes, *mode)?;
    }
    let encoder = builder.into_inner().map_err(|e| e.to_string())?;
    encoder.finish().map_err(|e| e.to_string())
}

fn build_ar_deb(control_tar_gz: &[u8], data_tar_gz: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    out.extend_from_slice(b"!<arch>\n");
    write_ar_member(&mut out, "debian-binary", b"2.0\n")?;
    write_ar_member(&mut out, "control.tar.gz", control_tar_gz)?;
    write_ar_member(&mut out, "data.tar.gz", data_tar_gz)?;
    Ok(out)
}

fn write_ar_member(out: &mut Vec<u8>, name: &str, data: &[u8]) -> Result<(), String> {
    let header = format!(
        "{:<16}{:<12}{:<6}{:<6}{:<8}{:<10}`\n",
        name,
        FIXED_MTIME,
        0,
        0,
        "100644",
        data.len()
    );
    if header.len() != 60 {
        return Err(format!("invalid ar header length for {name}"));
    }
    out.extend_from_slice(header.as_bytes());
    out.extend_from_slice(data);
    if data.len() % 2 == 1 {
        out.push(b'\n');
    }
    Ok(())
}
