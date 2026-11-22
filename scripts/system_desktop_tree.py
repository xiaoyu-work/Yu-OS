#!/usr/bin/env python3
"""Create the release-bound inventory for the fixed system desktop tree."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import stat


TREE = Path("usr/lib/yuos/system-desktop")
INVENTORY = Path("usr/lib/yuos/release-security/yuos-desktop/system-desktop-tree.json")
FORMAT = "yuos.system-desktop-tree/v1"
MAX_FILES = 20_000
MAX_BYTES = 256 * 1024 * 1024
MAX_INVENTORY_BYTES = 8 * 1024 * 1024


def valid_path(path):
    parts = path.split("/")
    return (
        path.isascii()
        and 0 < len(path) <= 512
        and len(parts) <= 16
        and all(
            part not in ("", ".", "..")
            and len(part) <= 128
            and not part.endswith((" ", "."))
            and "\\" not in part
            and all(32 <= ord(char) != 127 for char in part)
            for part in parts
        )
    )


def inventory(root):
    if root.is_symlink() or not root.is_dir():
        raise ValueError("system desktop tree must be a regular directory")
    root_mode = stat.S_IMODE(root.lstat().st_mode)
    if root_mode & 0o7022:
        raise ValueError("system desktop tree root is writable by other users")

    def fail_walk(error):
        raise error

    files = []
    total = 0
    for parent, directories, names in os.walk(root, followlinks=False, onerror=fail_walk):
        for name in directories + names:
            path = Path(parent) / name
            relative = path.relative_to(root).as_posix()
            info = path.lstat()
            mode = stat.S_IMODE(info.st_mode)
            if not valid_path(relative) or mode & 0o7022:
                raise ValueError(f"unprotected or invalid system desktop path: {relative}")
            if len(files) >= MAX_FILES:
                raise ValueError("system desktop tree exceeds its entry limit")
            if stat.S_ISDIR(info.st_mode):
                entry = {"path": relative, "type": "dir", "mode": mode, "size": 0, "digest": ""}
            elif stat.S_ISREG(info.st_mode) and info.st_nlink == 1:
                total += info.st_size
                if total > MAX_BYTES:
                    raise ValueError("system desktop tree exceeds its byte limit")
                digest = hashlib.sha256()
                descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC)
                with os.fdopen(descriptor, "rb") as stream:
                    opened = os.fstat(stream.fileno())
                    if (opened.st_dev, opened.st_ino, opened.st_mode, opened.st_size, opened.st_nlink) != (
                        info.st_dev, info.st_ino, info.st_mode, info.st_size, info.st_nlink
                    ):
                        raise ValueError(f"system desktop file changed during inventory: {relative}")
                    for chunk in iter(lambda: stream.read(1024 * 1024), b""):
                        digest.update(chunk)
                    after = os.fstat(stream.fileno())
                    if (after.st_size, after.st_mtime_ns, after.st_ctime_ns) != (
                        opened.st_size, opened.st_mtime_ns, opened.st_ctime_ns
                    ):
                        raise ValueError(f"system desktop file changed during inventory: {relative}")
                entry = {
                    "path": relative, "type": "file", "mode": mode,
                    "size": info.st_size, "digest": f"sha256:{digest.hexdigest()}",
                }
            else:
                raise ValueError(f"unsupported system desktop node: {relative}")
            files.append(entry)
    files.sort(key=lambda entry: entry["path"])
    entries = {entry["path"]: entry for entry in files}
    for required in ("app.json", "bin/yuos-desktop-shell", "resources/usr/share/yuos-desktop-shell/shell.qml"):
        if required not in entries or entries[required]["type"] != "file":
            raise ValueError(f"system desktop is missing {required}")
    if not entries["bin/yuos-desktop-shell"]["mode"] & 0o111:
        raise ValueError("system desktop entry is not executable")
    return {"format": FORMAT, "root_mode": root_mode, "files": files}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stage", type=Path, required=True)
    parser.add_argument("--check", action="store_true", help="verify the existing inventory without changing it")
    args = parser.parse_args()
    document = inventory(args.stage / TREE)
    encoded = (json.dumps(document, sort_keys=True, separators=(",", ":"), ensure_ascii=False) + "\n").encode()
    if len(encoded) > MAX_INVENTORY_BYTES:
        raise ValueError("system desktop inventory exceeds its byte limit")
    output = args.stage / INVENTORY
    if args.check:
        if output.read_bytes() != encoded:
            raise ValueError("system desktop inventory differs from the staged tree")
        return
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(encoded)
    output.chmod(0o644)


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError) as error:
        raise SystemExit(f"error: system desktop inventory: {error}")
