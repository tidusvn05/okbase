"""Exercise tag validation against real, disposable git repositories and Cargo metadata."""
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest


class CheckReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / "scripts").mkdir()
        for name in ("release.py", "check-release.py"):
            shutil.copy(Path(__file__).with_name(name), self.root / "scripts" / name)
        (self.root / "src").mkdir()
        (self.root / "src/lib.rs").write_text("")
        (self.root / "examples").mkdir()
        (self.root / "examples/demo.rs").write_text("fn main() {}")
        (self.root / "Cargo.toml").write_text('[workspace]\n[workspace.package]\nversion = "1.2.3"\n[package]\nname = "release-test"\nversion.workspace = true\n')
        (self.root / "CHANGELOG.md").write_text("## [1.2.3] - 2026-10-18\n")
        self.git("init", "-q")
        self.git("config", "user.email", "release@example.invalid")
        self.git("config", "user.name", "Release Test")
        self.git("add", ".")
        self.git("commit", "-qm", "test: release fixture")
        self.git("update-ref", "refs/remotes/origin/main", "HEAD")
        self.git("tag", "v1.2.3")

    def git(self, *args):
        return subprocess.run(["git", *args], cwd=self.root, check=True, capture_output=True)

    def check(self, tag):
        env = {key: value for key, value in os.environ.items() if key != "GITHUB_OUTPUT"}
        return subprocess.run([sys.executable, "scripts/check-release.py", tag], cwd=self.root,
                              env=env, text=True, capture_output=True)

    def test_examples_are_not_release_binaries(self):
        result = self.check("v1.2.3")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("binaries=false", result.stdout)
        self.assertIn("prerelease=false", result.stdout)

    def test_invalid_tag_is_rejected(self):
        result = self.check("v1.2.3-dev")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("invalid release tag", result.stderr)

    def test_tag_outside_main_is_rejected(self):
        (self.root / "other").write_text("change")
        self.git("add", "other")
        self.git("commit", "-qm", "test: unmerged commit")
        self.git("tag", "v1.2.4")
        self.assertNotEqual(self.check("v1.2.4").returncode, 0)

    def test_version_mismatch_is_rejected(self):
        self.git("tag", "v1.2.4")
        result = self.check("v1.2.4")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("does not match workspace", result.stderr)

    def test_missing_changelog_is_rejected(self):
        (self.root / "CHANGELOG.md").write_text("## [Unreleased]\n")
        result = self.check("v1.2.3")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("missing changelog", result.stderr)

    def test_rc_is_prerelease(self):
        path = self.root / "Cargo.toml"
        path.write_text(path.read_text().replace("1.2.3", "1.2.4-rc.1"))
        (self.root / "CHANGELOG.md").write_text("## [1.2.4-rc.1]\n")
        self.git("add", ".")
        self.git("commit", "-qm", "test: rc fixture")
        self.git("update-ref", "refs/remotes/origin/main", "HEAD")
        self.git("tag", "v1.2.4-rc.1")
        result = self.check("v1.2.4-rc.1")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("prerelease=true", result.stdout)
