#!/usr/bin/env python3
"""Stamp the automatically computed dated SDK version into packages and docs."""

from __future__ import annotations

import json
import pathlib
import re
import sys


ROOT = pathlib.Path(__file__).resolve().parents[2]
SDK_ROOT = ROOT / "yuos-sdk"

DOC_FILES = (
    SDK_ROOT / "quickstart.html",
    SDK_ROOT / "api.html",
    SDK_ROOT / "reference.html",
    SDK_ROOT / "linux-app-integration.html",
    SDK_ROOT / "README.md",
    SDK_ROOT / "llms.txt",
    SDK_ROOT / "python/README.md",
    SDK_ROOT / "node/README.md",
    SDK_ROOT / "go/README.md",
    SDK_ROOT / "rust/README.md",
)


def stamp_packages(display_version: str, semver_version: str) -> None:
    pyproject = SDK_ROOT / "python/pyproject.toml"
    pyproject.write_text(
        re.sub(
            r'^version = "[^"]+"',
            f'version = "{display_version}"',
            pyproject.read_text(encoding="utf-8"),
            count=1,
            flags=re.MULTILINE,
        ),
        encoding="utf-8",
    )

    py_init = SDK_ROOT / "python/src/yuos_sdk/__init__.py"
    py_init.write_text(
        re.sub(
            r'^__version__\s*=\s*"[^"]+"',
            f'__version__ = "{display_version}"',
            py_init.read_text(encoding="utf-8"),
            count=1,
            flags=re.MULTILINE,
        ),
        encoding="utf-8",
    )

    pkg_json_path = SDK_ROOT / "node/package.json"
    pkg_json = json.loads(pkg_json_path.read_text(encoding="utf-8"))
    pkg_json["version"] = semver_version
    pkg_json_path.write_text(json.dumps(pkg_json, indent=2) + "\n", encoding="utf-8")

    pkg_lock_path = SDK_ROOT / "node/package-lock.json"
    pkg_lock = json.loads(pkg_lock_path.read_text(encoding="utf-8"))
    pkg_lock["version"] = semver_version
    if "" in pkg_lock.get("packages", {}):
        pkg_lock["packages"][""]["version"] = semver_version
    pkg_lock_path.write_text(json.dumps(pkg_lock, indent=2) + "\n", encoding="utf-8")

    rust_cargo = SDK_ROOT / "rust/Cargo.toml"
    rust_cargo.write_text(
        re.sub(
            r'^version = "[^"]+"',
            f'version = "{semver_version}"',
            rust_cargo.read_text(encoding="utf-8"),
            count=1,
            flags=re.MULTILINE,
        ),
        encoding="utf-8",
    )

    cargo_lock = ROOT / "Cargo.lock"
    cargo_lock.write_text(
        re.sub(
            r'(name = "yuos-sdk"\nversion = )"[^"]+"',
            rf'\1"{semver_version}"',
            cargo_lock.read_text(encoding="utf-8"),
            count=1,
        ),
        encoding="utf-8",
    )


def stamp_docs(display_version: str, semver_version: str) -> None:
    tag = f"yuos-sdk-v{display_version}"
    for path in DOC_FILES:
        text = path.read_text(encoding="utf-8")
        text = re.sub(
            r'(<span class="sdk-version">)v?[^<]+(</span>)',
            rf"\1v{display_version}\2",
            text,
        )
        text = re.sub(
            r"yuos-sdk-v[0-9]+\.[0-9]+\.[0-9]+(?:[.-]dev\.?[0-9]+(?:\.[0-9]+)?)?",
            tag,
            text,
        )
        text = re.sub(
            r"yuos_sdk-[0-9]+\.[0-9]+\.[0-9]+(?:\.dev[0-9]+)?-py3-none-any\.whl",
            f"yuos_sdk-{display_version}-py3-none-any.whl",
            text,
        )
        text = re.sub(
            r"yuos-sdk-node-[0-9]+\.[0-9]+\.[0-9]+(?:[.-]dev\.?[0-9]+(?:\.[0-9]+)?)?\.tgz",
            f"yuos-sdk-node-{display_version}.tgz",
            text,
        )
        text = re.sub(
            r"github\.com/xiaoyu-work/Yu-OS/yuos-sdk/go@v[0-9]+\.[0-9]+\.[0-9]+(?:-dev\.[0-9]+)?",
            f"github.com/xiaoyu-work/Yu-OS/yuos-sdk/go@v{semver_version}",
            text,
        )
        text = re.sub(
            r"Download & Install \(`v[^`]+`\)",
            f"Download & Install (`v{display_version}`)",
            text,
        )
        path.write_text(text, encoding="utf-8")


def main() -> None:
    if len(sys.argv) not in (2, 3):
        raise SystemExit("usage: stamp-sdk-version.py <display_version> [<semver_version>] [--docs-only]")
    docs_only = "--docs-only" in sys.argv
    args = [arg for arg in sys.argv[1:] if arg != "--docs-only"]
    display_version = args[0]
    semver_version = args[1] if len(args) > 1 else display_version.replace(".dev", "-dev.", 1)
    if not docs_only:
        stamp_packages(display_version, semver_version)
    stamp_docs(display_version, semver_version)


if __name__ == "__main__":
    main()
