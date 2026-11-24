"""Keep the other publisher's files when replacing the GitHub Pages site."""

import argparse
import hashlib
from pathlib import Path
import re
import shutil
import subprocess
from urllib.error import HTTPError
from urllib.parse import quote
from urllib.request import HTTPRedirectHandler, Request, build_opener
from uuid import uuid4


PAGES_URL = "https://xiaoyu-work.github.io/Yu-OS/"
INVENTORY = "pages-files.sha256"
SIGNATURE = INVENTORY + ".asc"
KEYRING = "yuos-archive-keyring.gpg"
# Same OS archive signer required by release.yml; this is not the App APT key.
APT_SIGNER = "CB53349313705C3BCB1C2F84BEB2F1F489A298CA"
APT_MARKERS = ("dists/trixie/InRelease", "dists/trixie/Release", KEYRING)
REQUIRED = {
    "website": {"index.html", "style.css", "app.js"},
    "apt": {*APT_MARKERS, "dists/trixie/Release.gpg", "yuos-archive-keyring.asc"},
}
PREFIXES = {"website": ("assets/brand/", "fonts/", "install/", "sdk/"), "apt": ("dists/", "pool/")}


class NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


OPENER = build_opener(NoRedirect())


def valid_path(section, name):
    return all(
        re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._+~:-]*", part) for part in name.split("/")
    ) and (name in REQUIRED[section] or name.startswith(PREFIXES[section]))


def check_inventory(section, files):
    missing = REQUIRED[section] - files.keys()
    if missing:
        raise ValueError(f"Incomplete {section} inventory: missing {', '.join(sorted(missing))}")
    if section == "apt" and not any(name.startswith("pool/") for name in files):
        raise ValueError("The signed APT inventory has no packages")


def sha256(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def record(section, root):
    files = {}
    for path in sorted(root.rglob("*")):
        name = path.relative_to(root).as_posix()
        if not path.is_symlink() and path.is_dir():
            continue
        if path.is_symlink() or not path.is_file() or not valid_path(section, name):
            raise ValueError(f"Unexpected file in {section} staging: {name}")
        files[name] = sha256(path)
    check_inventory(section, files)
    (root / INVENTORY).write_text(
        "".join(f"{digest}  {name}\n" for name, digest in files.items()), encoding="ascii"
    )


def parse_inventory(section, body):
    files = {}
    for line in body.decode("ascii").splitlines():
        digest, sep, name = line.partition("  ")
        if (
            not sep or not re.fullmatch("[0-9a-f]{64}", digest)
            or not valid_path(section, name) or name in files
        ):
            raise ValueError(f"Invalid entry in the published {section} inventory")
        files[name] = digest
    check_inventory(section, files)
    return files


def published(name, nonce):
    url = f"{PAGES_URL}{quote(name)}?snapshot={nonce}"
    request = Request(url, headers={"Cache-Control": "no-cache, no-store"})
    response = OPENER.open(request, timeout=45)
    if response.status != 200:
        response.close()
        raise ValueError(f"Unexpected Pages response for {name}")
    return response


def read_metadata(name, nonce):
    limit = 1024 * 1024
    with published(name, nonce) as source:
        body = source.read(limit + 1)
    if len(body) > limit:
        raise ValueError(f"Pages metadata exceeds size limit: {name}")
    return body


def confirm_no_archive(nonce):
    # The first website-only release must not discard a pre-inventory archive.
    for prefix in ("apt/", ""):
        for name in APT_MARKERS:
            try:
                with published(prefix + name, nonce):
                    raise ValueError(f"Cannot omit the published APT archive: {prefix}{name} exists")
            except HTTPError as error:
                if error.code != 404:
                    raise


def verify_apt_inventory(root):
    result = subprocess.run(
        ["gpgv", "--status-fd", "1", "--keyring", str(root / KEYRING),
         str(root / SIGNATURE), str(root / INVENTORY)],
        check=True, capture_output=True, text=True,
    )
    for line in result.stdout.splitlines():
        fields = line.split()
        if len(fields) >= 11 and fields[:2] == ["[GNUPG:]", "VALIDSIG"]:
            if APT_SIGNER in (fields[2], fields[-1]):
                return
    raise ValueError("The APT inventory was not signed by the Yu OS archive key")


def restore(section, root, allow_no_apt):
    prefix = "apt/" if section == "apt" else ""
    nonce = uuid4().hex
    try:
        body = read_metadata(prefix + INVENTORY, nonce)
    except HTTPError as error:
        if error.code != 404:
            raise
        if section == "website":
            raise ValueError("The live website inventory is missing; run Publish Website first") from error
        if not allow_no_apt:
            raise ValueError(
                "The signed APT inventory is missing; allow_no_apt is only for an absent archive"
            ) from error
        confirm_no_archive(nonce)
        print("No signed OS APT archive is published; deploying the website alone.")
        return
    files = parse_inventory(section, body)
    root.mkdir(parents=True, exist_ok=True)
    (root / INVENTORY).write_bytes(body)
    if section == "apt":
        (root / SIGNATURE).write_bytes(read_metadata(prefix + SIGNATURE, nonce))
        key = read_metadata(prefix + KEYRING, nonce)
        if hashlib.sha256(key).hexdigest() != files.pop(KEYRING):
            raise ValueError("The published APT keyring differs from its inventory")
        (root / KEYRING).write_bytes(key)
        verify_apt_inventory(root)
    for name, digest in files.items():
        target = root / name
        target.parent.mkdir(parents=True, exist_ok=True)
        with published(prefix + name, nonce) as source, target.open("xb") as destination:
            shutil.copyfileobj(source, destination)
        if sha256(target) != digest:
            raise ValueError(f"The published file differs from its inventory: {prefix}{name}")
    print(f"Preserved the published {section} files.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("record", "restore"))
    parser.add_argument("section", choices=tuple(REQUIRED))
    parser.add_argument("root", type=Path)
    parser.add_argument("--allow-no-apt", action="store_true")
    args = parser.parse_args()
    if args.allow_no_apt and (args.action, args.section) != ("restore", "apt"):
        parser.error("--allow-no-apt is only valid when restoring APT")
    try:
        if args.action == "record":
            record(args.section, args.root)
        else:
            restore(args.section, args.root.resolve(), args.allow_no_apt)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"error: Pages {args.action} {args.section}: {error}\n")


if __name__ == "__main__":
    main()
