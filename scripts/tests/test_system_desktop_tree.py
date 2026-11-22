import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "system_desktop_tree.py"
SPEC = importlib.util.spec_from_file_location("system_desktop_tree", SCRIPT)
module = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(module)


class SystemDesktopTreeTests(unittest.TestCase):
    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory()
        self.addCleanup(self.scratch.cleanup)
        self.root = Path(self.scratch.name) / "desktop"
        self.root.mkdir(mode=0o755)
        for name in (
            "app.json",
            "bin/yuos-desktop-shell",
            "resources/usr/share/yuos-desktop-shell/shell.qml",
        ):
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(name.encode())
            path.chmod(0o755 if name.startswith("bin/") else 0o644)

    def test_inventory_tracks_extra_content_and_file_modes(self):
        before = module.inventory(self.root)
        (self.root / "extra").write_text("extra", encoding="utf-8")
        self.assertNotEqual(module.inventory(self.root), before)
        (self.root / "extra").unlink()
        (self.root / "app.json").chmod(0o600)
        self.assertNotEqual(module.inventory(self.root), before)
        (self.root / "app.json").chmod(0o644)
        self.root.chmod(0o700)
        self.assertNotEqual(module.inventory(self.root), before)

    def test_rejects_symlink_and_non_executable_entry(self):
        os.symlink("app.json", self.root / "link")
        with self.assertRaisesRegex(ValueError, "unprotected|unsupported"):
            module.inventory(self.root)
        (self.root / "link").unlink()
        (self.root / "bin/yuos-desktop-shell").chmod(0o644)
        with self.assertRaisesRegex(ValueError, "not executable"):
            module.inventory(self.root)

    def test_rejects_hardlinks_and_special_nodes(self):
        os.link(self.root / "app.json", self.root / "hardlink")
        with self.assertRaisesRegex(ValueError, "unsupported"):
            module.inventory(self.root)
        (self.root / "hardlink").unlink()
        os.mkfifo(self.root / "pipe", 0o600)
        with self.assertRaisesRegex(ValueError, "unsupported"):
            module.inventory(self.root)

    def test_rejects_tree_exceeding_byte_limit_before_reading(self):
        with (self.root / "large").open("wb") as output:
            output.truncate(module.MAX_BYTES + 1)
        with self.assertRaisesRegex(ValueError, "byte limit"):
            module.inventory(self.root)

    def test_rejects_writable_or_missing_resources(self):
        (self.root / "app.json").chmod(0o666)
        with self.assertRaisesRegex(ValueError, "unprotected"):
            module.inventory(self.root)
        (self.root / "app.json").chmod(0o644)
        (self.root / "resources/usr/share/yuos-desktop-shell/shell.qml").unlink()
        with self.assertRaisesRegex(ValueError, "missing"):
            module.inventory(self.root)

    def test_check_detects_changes_without_rewriting_inventory(self):
        stage = Path(self.scratch.name) / "stage"
        tree = stage / module.TREE
        tree.parent.mkdir(parents=True)
        self.root.rename(tree)
        command = [sys.executable, str(SCRIPT), "--stage", str(stage)]
        subprocess.run(command, check=True)
        inventory = stage / module.INVENTORY
        original = inventory.read_bytes()
        subprocess.run([*command, "--check"], check=True)
        (tree / "app.json").write_text("changed", encoding="utf-8")
        result = subprocess.run([*command, "--check"], capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("inventory differs", result.stderr)
        self.assertEqual(inventory.read_bytes(), original)

    def test_release_manifest_binds_generated_inventory(self):
        project = SCRIPT.parent.parent
        policy = json.loads((project / "packaging/release-security/policy.json").read_text())
        stage = Path(self.scratch.name) / "stage"
        tree = stage / module.TREE
        tree.parent.mkdir(parents=True)
        self.root.rename(tree)
        for component in policy["components"]:
            if component["package"] != "yuos-desktop":
                continue
            path = stage / component["path"].lstrip("/")
            if path == stage / module.INVENTORY:
                continue
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b"fixture")
        subprocess.run([sys.executable, str(SCRIPT), "--stage", str(stage)], check=True)
        manifest_path = stage / "manifest.json"
        subprocess.run([
            sys.executable, str(project / "packaging/release-security/make-manifest.py"),
            "--package", "yuos-desktop", "--version", "1:0.1.0", "--arch", "amd64",
            "--stage-dir", str(stage), "--output", str(manifest_path),
        ], check=True, capture_output=True)
        manifest = json.loads(manifest_path.read_text())
        component = next(entry for entry in manifest["components"]
                         if entry["name"] == "yuos-system-desktop-tree")
        inventory = (stage / module.INVENTORY).read_bytes()
        self.assertEqual(component["sha256"], hashlib.sha256(inventory).hexdigest())


if __name__ == "__main__":
    unittest.main()
