"""Seal and embed the locked Desktop Shell source build into yuos-desktop."""

import argparse
import gzip
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import subprocess
import tarfile
import tempfile

from app_packages import DESKTOP_SHELL_ROOT, ROOT, run, sha256, work_root


APP = "desktop-shell"
ENTRY = "bin/yuos-desktop-shell"
SHELL_QML = "resources/usr/share/yuos-desktop-shell/shell.qml"
BUILD_FORMAT = "yuos.desktop-shell-build/v1"
SELECTION_FORMAT = "yuos.desktop-shell-selection/v1"
MAX_BYTES = 264 * 1024 * 1024
MAX_FILES = 20_001
MAX_METADATA = 8 * 1024 * 1024
SHA256 = re.compile(r"[0-9a-f]{64}")
REVISION = re.compile(r"[0-9a-f]{40}")
VERSION = re.compile(r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)")
MACHINE = {"amd64": 62, "arm64": 183}
PUBLISHER = ROOT / "packaging/deb/yuos-agent/trust/publishers.d/yuos-app.json"
SIGNING_KEY = "YUOS_APPS_PROVENANCE_SIGNING_PRIVATE_KEY"


def inputs(architecture):
    return work_root() / f"desktop-shell-{architecture}"


def unique_object(pairs):
    value = {}
    for key, entry in pairs:
        if key in value:
            raise ValueError("Desktop Shell metadata contains a duplicate field")
        value[key] = entry
    return value


def metadata(path):
    if path.is_symlink() or not path.is_file() or path.stat().st_size > MAX_METADATA:
        raise ValueError(f"Expected bounded regular Desktop Shell metadata: {path.name}")
    value = json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=unique_object)
    if not isinstance(value, dict):
        raise ValueError(f"Expected a Desktop Shell JSON object: {path.name}")
    return value


def payload_contract(root, architecture, revision):
    manifest = metadata(root / "app.json")
    version = manifest["version"]
    desktop = manifest["desktop"]
    if (
        manifest["id"] != APP or manifest["runtime"] != "binary" or manifest["entry"] != ENTRY
        or not isinstance(version, str) or not VERSION.fullmatch(version)
        or not isinstance(desktop, dict) or desktop.get("exec") != "--gui"
        or desktop.get("entry", ENTRY) != ENTRY or desktop.get("entries", {}) != {}
        or {"mcp", "operations", "web_ui"}.intersection(manifest)
    ):
        raise ValueError("Desktop Shell requires its declared GUI-only App entrypoint")
    entry = root / ENTRY
    if not entry.is_file() or not entry.stat().st_mode & 0o100 or not (root / SHELL_QML).is_file():
        raise ValueError("Desktop Shell requires a regular executable entry and private Shell QML")
    with entry.open("rb") as stream:
        if not target_elf(stream.read(20), architecture):
            raise ValueError("Desktop Shell does not match the target architecture")
    build = metadata(root / "build.json")
    if (build["format"], build["architecture"], build["release_version"], build["source_revision"]) != (
        BUILD_FORMAT, architecture, version, revision,
    ):
        raise ValueError("Desktop Shell build does not match the target, App manifest or locked App revision")
    depends = build["depends"]
    if not isinstance(depends, list) or not depends or any(
        not isinstance(item, str) or not re.fullmatch(r"[a-z0-9][a-z0-9.+-]*(?: \(>= [A-Za-z0-9.+:~-]+\))?", item)
        for item in depends
    ):
        raise ValueError("Invalid Desktop Shell Debian runtime dependencies")
    return version, ", ".join(depends)


def target_elf(header, architecture):
    return (
        len(header) == 20 and header[:7] == b"\x7fELF\x02\x01\x01"
        and int.from_bytes(header[18:20], "little") == MACHINE[architecture]
    )


def provenance(root, *, key=None, version=None):
    host = run(["dpkg", "--print-architecture"]).stdout.decode().strip()
    tool = work_root() / f"yu-provenance-linux-{host}"
    if tool.is_symlink() or not tool.is_file() or not os.access(tool, os.X_OK):
        raise ValueError(f"Build the native musl Yu OS CLI from this checkout and install it at {tool}")
    # The CLI uses passwd, not $HOME, for its isolated trust and audit paths.
    with tempfile.TemporaryDirectory(prefix="yuos-desktop-shell-trust-") as scratch:
        home = Path(scratch) / "home"
        home.mkdir(mode=0o700)
        passwd = Path(scratch) / "passwd"
        passwd.write_text("builder:x:0:0:Build verifier:/home/builder:/bin/false\n")
        sandbox = [
            "bwrap", "--unshare-all", "--die-with-parent", "--uid", "0", "--gid", "0",
            "--clearenv", "--setenv", "LC_ALL", "C", "--chdir", "/",
            "--ro-bind", passwd, "/etc/passwd", "--bind", home, "/home/builder",
            "--ro-bind", tool, "/yu",
        ]
        cli = ["--", "/yu", "--json", "provenance"]
        app = ["--kind", "app", "--id", APP, "--path", "/payload"]
        if key is not None:
            run([
                *sandbox, "--bind", root, "/payload", "--ro-bind", key, "/key.json",
                *cli, "sign", *app, "--version", version, "--key", "/key.json",
                "--entrypoint", ENTRY, "--resource", SHELL_QML,
            ])
            return None
        isolate = [*sandbox, "--ro-bind", root, "/payload", "--ro-bind", PUBLISHER, "/publisher.json", *cli]
        run([*isolate, "trust", "add", "--file", "/publisher.json"])
        return json.loads(run([*isolate, "verify", *app]).stdout)


def verify(root, architecture, pin):
    if root.is_symlink() or not root.is_dir():
        raise ValueError("Missing regular Desktop Shell payload directory")
    report = provenance(root)
    package = report["package"]
    publisher = metadata(PUBLISHER)["keys"][0]["key_id"]
    if report["verified"] is not True or report["manifest_path"] != "app.json" or (
        report["entrypoints"] != [ENTRY] or package["trust"] != "publisher"
        or package["publisher_key_id"] != publisher
    ):
        raise ValueError("Desktop Shell requires its existing App publisher and GUI entrypoint")
    envelope = root / ".provenance.json"
    if metadata(envelope)["package"]["resources"] != [SHELL_QML]:
        raise ValueError("Desktop Shell requires signed private resources")
    if pin["envelope_sha256"] != sha256(envelope):
        raise ValueError("Desktop Shell differs from the frozen release input")
    version, depends = payload_contract(root, architecture, pin["source_revision"])
    if version != package["version"] or version != pin["release_version"]:
        raise ValueError("Signed Desktop Shell does not match the selected App manifest version")
    return depends


def validate_archive(archive):
    # tarfile parses extended headers before exposing members. Bound raw
    # headers and the gzip stream before allowing tarfile to process them.
    with gzip.open(archive, "rb") as stream:
        count = total = 0
        extended = False
        while True:
            block = stream.read(tarfile.BLOCKSIZE)
            if len(block) != tarfile.BLOCKSIZE:
                raise ValueError("Truncated Desktop Shell archive header")
            if not any(block):
                tail = stream.read(tarfile.RECORDSIZE + 1)
                if extended or not tarfile.BLOCKSIZE <= len(tail) <= tarfile.RECORDSIZE or any(tail):
                    raise ValueError("Invalid Desktop Shell archive end")
                return
            node = tarfile.TarInfo.frombuf(block, "utf-8", "strict")
            count += 1
            total += node.size
            if node.size < 0 or total > MAX_BYTES or count > 2 * MAX_FILES:
                raise ValueError("Desktop Shell archive exceeds its inventory limits")
            if node.type == tarfile.XHDTYPE and not extended and node.size <= 64 * 1024:
                extended = True
            elif node.type in (tarfile.REGTYPE, tarfile.AREGTYPE, tarfile.DIRTYPE):
                extended = False
            else:
                raise ValueError("Unsupported Desktop Shell archive header")
            blocks = (node.size + tarfile.BLOCKSIZE - 1) // tarfile.BLOCKSIZE
            stream.seek(blocks * tarfile.BLOCKSIZE, os.SEEK_CUR)


def extract(archive, root, architecture, checksum=None):
    if archive.is_symlink() or not archive.is_file() or archive.stat().st_size > MAX_BYTES or (
        checksum is not None and sha256(archive) != checksum
    ):
        raise ValueError("Missing or changed Desktop Shell archive")
    validate_archive(archive)
    root.mkdir()
    root.chmod(0o755)
    directories = {PurePosixPath(".")}
    nodes = set()
    size = 0
    with tarfile.open(archive, mode="r|gz") as payload:
        for member in payload:
            path = PurePosixPath(member.name)
            folded = member.name.casefold()
            size += member.size
            if (
                path.is_absolute() or path.as_posix() != member.name or not path.parts or ".." in path.parts
                or "\\" in member.name or any(ord(char) < 32 or ord(char) == 127 for char in member.name)
                or path.parent not in directories or folded in nodes or len(nodes) >= MAX_FILES
                or size > MAX_BYTES or member.size < 0
                or member.uid or member.gid or member.mode & 0o7022
                or (not member.isdir() and not member.isfile()) or member.issparse()
                or set(member.pax_headers) - {"path"} or (member.isdir() and member.size)
            ):
                raise ValueError(f"Invalid Desktop Shell archive entry: {member.name!r}")
            payload.extract(member, root, filter="data")
            extracted = root / path
            extracted.chmod(member.mode)
            nodes.add(folded)
            if member.isdir():
                directories.add(path)
            else:
                with extracted.open("rb") as stream:
                    header = stream.read(20)
                if header.startswith(b"\x7fELF") and not target_elf(header, architecture):
                    raise ValueError(f"Wrong architecture in Desktop Shell archive: {member.name!r}")


def archive_payload(root, destination):
    def normalize(entry):
        entry.uid = entry.gid = entry.mtime = 0
        entry.uname = entry.gname = ""
        return entry

    with destination.open("xb") as stream, gzip.GzipFile(
        filename="", mode="wb", fileobj=stream, mtime=0, compresslevel=6,
    ) as compressed, tarfile.open(fileobj=compressed, mode="w|", format=tarfile.PAX_FORMAT) as archive:
        for path in sorted(root.rglob("*")):
            archive.add(path, arcname=path.relative_to(root).as_posix(), recursive=False, filter=normalize)


def seal(architecture, revision, unsigned):
    signing_material = os.environ.pop(SIGNING_KEY, "")
    if not signing_material:
        raise ValueError(f"Set {SIGNING_KEY} on the OS signer to the approved Yu OS App publisher key JSON")
    if not REVISION.fullmatch(revision):
        raise ValueError("Desktop Shell requires the exact locked App Git revision")
    publisher = metadata(PUBLISHER)["keys"][0]
    try:
        identity = json.loads(signing_material)
    except ValueError:
        raise ValueError(f"{SIGNING_KEY} must contain Yu OS App publisher key JSON") from None
    if not isinstance(identity, dict) or any(
        identity.get(field) != publisher[field] for field in ("key_id", "public_key", "algorithm")
    ):
        raise ValueError(f"{SIGNING_KEY} must match the official Yu OS App publisher bundled by this checkout")
    directory = inputs(architecture)
    directory.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="seal-", dir=directory) as scratch:
        scratch = Path(scratch)
        root = scratch / APP
        extract(unsigned, root, architecture)
        envelope = root / ".provenance.json"
        if envelope.exists():
            raise ValueError("Desktop Shell source build must be unsigned")
        version, _ = payload_contract(root, architecture, revision)
        with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", prefix="yuos-app-key-") as key:
            key.write(signing_material)
            key.flush()
            try:
                provenance(root, key=Path(key.name), version=version)
            except subprocess.CalledProcessError:
                raise ValueError(f"Signing failed; check the native CLI and approved {SIGNING_KEY}") from None
        envelope.chmod(0o644)
        pin = {
            "format": SELECTION_FORMAT, "architecture": architecture, "release_version": version,
            "source_revision": revision, "envelope_sha256": sha256(envelope),
        }
        verify(root, architecture, pin)
        archive = scratch / "payload.tar.gz"
        archive_payload(root, archive)
        pin["archive_sha256"] = sha256(archive)
        (scratch / "selection.json").write_text(json.dumps(pin, sort_keys=True) + "\n", encoding="utf-8")
        archive.replace(directory / archive.name)
        (scratch / "selection.json").replace(directory / "selection.json")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    sign = commands.add_parser("seal")
    sign.add_argument("--source-revision", required=True)
    sign.add_argument("--unsigned-archive", type=Path, required=True)
    stage = commands.add_parser("stage")
    check = commands.add_parser("check")
    for command in (sign, stage, check):
        command.add_argument("--architecture", choices=MACHINE, required=True)
    for command in (stage, check):
        command.add_argument("--stage", type=Path, required=True)
    args = parser.parse_args()
    if args.command == "seal":
        seal(args.architecture, args.source_revision, args.unsigned_archive)
        return
    directory = inputs(args.architecture)
    selection = directory / "selection.json"
    if not selection.is_file():
        raise ValueError("Sealed Desktop Shell inputs are required; see docs/building-desktop.md")
    pin = metadata(selection)
    if pin.get("format") != SELECTION_FORMAT or pin.get("architecture") != args.architecture or not all(
        isinstance(pin.get(field), str) and pattern.fullmatch(pin[field])
        for field, pattern in (
            ("archive_sha256", SHA256), ("envelope_sha256", SHA256),
            ("source_revision", REVISION), ("release_version", VERSION),
        )
    ):
        raise ValueError("Invalid sealed Desktop Shell selection; seal the current App build")
    root = args.stage / DESKTOP_SHELL_ROOT
    if args.command == "stage":
        root.parent.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(prefix=".shell-", dir=root.parent) as scratch:
            candidate = Path(scratch) / APP
            extract(directory / "payload.tar.gz", candidate, args.architecture, pin["archive_sha256"])
            verify(candidate, args.architecture, pin)
            if root.exists() or root.is_symlink():
                raise ValueError("Desktop Shell staging path already exists")
            candidate.rename(root)
    else:
        print(verify(root, args.architecture, pin))


if __name__ == "__main__":
    try:
        main()
    except subprocess.CalledProcessError as error:
        details = error.stderr or error.stdout or str(error).encode()
        raise SystemExit(f"error: Desktop Shell input failed: {details.decode(errors='replace').strip()}")
    except (ValueError, KeyError, OSError, EOFError, tarfile.TarError) as error:
        raise SystemExit(f"error: Desktop Shell input failed: {error}")
