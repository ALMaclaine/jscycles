"""Real files and pass/defect/repair cases for the offline CI guard."""

import json
from pathlib import Path
import shutil
import tempfile
import unittest

from check_adoption import check


class AdoptionIntegrity(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        source = Path(__file__).resolve().parents[1]
        for name in ["rust-oe.toml", "rust-oe.lock.json", "AGENTS.md"]:
            shutil.copyfile(source / name, self.root / name)
        shutil.copytree(source / ".rust-oe", self.root / ".rust-oe")
        (self.root / "docs").mkdir()
        shutil.copyfile(source / "docs/rust-oe-adoption.md", self.root / "docs/rust-oe-adoption.md")
        self.expected_count = len(json.loads((self.root / "rust-oe.lock.json").read_bytes())["files"])
        self.assertEqual(check(self.root), self.expected_count)

    def rejected_edit(self, name, contents):
        path = self.root / name
        original = path.read_bytes()
        path.write_bytes(contents)
        with self.assertRaises((ValueError, KeyError, TypeError)):
            check(self.root)
        path.write_bytes(original)
        self.assertEqual(check(self.root), self.expected_count, "repair should restore a passing guard")

    def test_content_drift_and_repair(self):
        self.rejected_edit(".rust-oe/INDEX.md", b"edited index")

    def test_config_drift_and_repair(self):
        original = (self.root / "rust-oe.toml").read_bytes()
        self.rejected_edit("rust-oe.toml", original.replace(b'"api",', b'"unsafe",'))

    def test_config_formatting_is_not_drift(self):
        path = self.root / "rust-oe.toml"
        text = path.read_text()
        path.write_text("# formatting-only comment\n" + text.replace(
            'revision = "19e4133c0461129b0f5fdb286abd7b87511242e2"\ncatalog = "rust-oe-bundles.toml"',
            'catalog = "rust-oe-bundles.toml"\nrevision = "19e4133c0461129b0f5fdb286abd7b87511242e2"'))
        self.assertEqual(check(self.root), self.expected_count)

    def test_agent_block_drift_and_repair(self):
        text = (self.root / "AGENTS.md").read_bytes()
        self.rejected_edit("AGENTS.md", text.replace(b"load practices", b"ignore practices"))

    def test_project_owned_agent_text_can_change(self):
        with (self.root / "AGENTS.md").open("a") as stream:
            stream.write("\nProject-owned instruction.\n")
        self.assertEqual(check(self.root), self.expected_count)

    def test_extra_file_and_empty_directory_are_drift(self):
        for name, directory in [("extra.md", False), ("extra", True)]:
            path = self.root / ".rust-oe" / name
            path.mkdir() if directory else path.write_text("unmanaged")
            with self.assertRaisesRegex(ValueError, "unmanaged"):
                check(self.root)
            path.rmdir() if directory else path.unlink()
            self.assertEqual(check(self.root), self.expected_count)

    def test_missing_content_is_not_success(self):
        path = self.root / ".rust-oe/INDEX.md"
        original = path.read_bytes()
        path.unlink()
        with self.assertRaises(OSError):
            check(self.root)
        path.write_bytes(original)
        self.assertEqual(check(self.root), self.expected_count)

    def test_symlink_is_refused(self):
        path = self.root / ".rust-oe/INDEX.md"
        original = path.read_bytes()
        target = self.root / "index-copy.md"
        target.write_bytes(original)
        path.unlink()
        path.symlink_to(target)
        with self.assertRaisesRegex(ValueError, "symlink"):
            check(self.root)
        path.unlink()
        path.write_bytes(original)
        self.assertEqual(check(self.root), self.expected_count)

    def test_lock_inventory_and_dependencies_cannot_drift(self):
        path = self.root / "rust-oe.lock.json"
        lock = json.loads(path.read_bytes())
        lock["schema"] = 1.0
        self.rejected_edit("rust-oe.lock.json", json.dumps(lock).encode())
        lock["schema"] = 1
        lock["bundles"]["api"]["requires"] = ["missing"]
        self.rejected_edit("rust-oe.lock.json", json.dumps(lock).encode())

    def test_paths_cannot_escape_the_managed_directory(self):
        lock = json.loads((self.root / "rust-oe.lock.json").read_bytes())
        lock["bundles"]["api"]["files"]["../outside.md"] = {"sha256": "0" * 64}
        self.rejected_edit("rust-oe.lock.json", json.dumps(lock).encode())


if __name__ == "__main__":
    unittest.main()
