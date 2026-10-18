"""Offline regression tests for release safety boundaries."""
import importlib.util
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("release", Path(__file__).with_name("release.py"))
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)


def package(name, deps=(), publish=None, version="1.2.3"):
    return {"id": name, "name": name, "version": version, "publish": publish,
            "dependencies": [{"name": n, "path": f"/workspace/{n}", "kind": kind} for n, kind in deps]}


class ReleaseTests(unittest.TestCase):
    def metadata(self, *packages):
        return {"packages": list(packages), "workspace_members": [p["id"] for p in packages]}

    def test_dev_dependencies_precede_dependents(self):
        data = self.metadata(package("cli", [("core", "dev")]), package("core"), package("eval", publish=[]))
        with patch.object(release, "metadata", return_value=data):
            self.assertEqual(release.publish_order(), (["core", "cli"], "1.2.3"))

    def test_cycle_is_rejected(self):
        data = self.metadata(package("a", [("b", "dev")]), package("b", [("a", None)]))
        with patch.object(release, "metadata", return_value=data), self.assertRaisesRegex(SystemExit, "cycle.*a.*b"):
            release.publish_order()

    def test_yanked_version_cannot_be_reused(self):
        with patch.object(release, "index_versions", return_value={"1.2.3": {"yanked": True}}):
            with self.assertRaisesRegex(SystemExit, "yanked"):
                release.published("core", "1.2.3")

    def test_workspace_dry_run_excludes_private_crates(self):
        data = self.metadata(package("core"), package("eval", publish=[]))
        with patch.object(release, "metadata", return_value=data), patch.object(release.subprocess, "run") as run:
            release.publish(True, True)
            args = run.call_args.args[0]
            self.assertIn("--workspace", args)
            self.assertIn("--locked", args)
            self.assertIn("--allow-dirty", args)
            self.assertEqual(args[args.index("--exclude") + 1], "eval")

    def test_no_upload_if_any_crate_is_new(self):
        with patch.object(release, "publish_order", return_value=(["a", "new"], "1.2.3")), \
             patch.object(release.subprocess, "check_output", return_value=b""), \
             patch.object(release, "index_versions", side_effect=[{"1.0.0": {}}, None]), \
             patch.object(release.subprocess, "run") as run:
            with self.assertRaisesRegex(SystemExit, "new crates"):
                release.publish(False)
            run.assert_not_called()

    def test_dirty_tree_prevents_upload(self):
        with patch.object(release, "publish_order", return_value=(["a"], "1.2.3")), \
             patch.object(release.subprocess, "check_output", return_value=b" M Cargo.toml"):
            with self.assertRaisesRegex(SystemExit, "clean"):
                release.publish(False)

    def test_version_updates_explicit_members_and_internal_dependencies(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            member = root / "member.toml"
            (root / "Cargo.toml").write_text('[workspace.package]\nversion = "1.0.0"\n[workspace.dependencies]\na = { version = "1.0.0", path = "a" }\nexternal = "2"\n')
            member.write_text('[package]\nname = "a"\nversion = "1.0.0"\n[dev-dependencies.a]\npath = "../a"\nversion = "1.0.0"\n')
            data = {"packages": [{"manifest_path": str(member)}]}
            with patch.object(release, "ROOT", root), patch.object(release, "metadata", return_value=data), \
                 patch.object(release, "publish_order"), patch.object(release.subprocess, "run"):
                release.set_version("1.2.0-rc.1")
            self.assertEqual(member.read_text().count('version = "1.2.0-rc.1"'), 2)
            self.assertIn('external = "2"', (root / "Cargo.toml").read_text())

    def test_resume_skips_published_versions(self):
        with patch.object(release, "publish_order", return_value=(["a"], "1.2.3")), \
             patch.object(release.subprocess, "check_output", return_value=b""), \
             patch.object(release, "index_versions", return_value={"1.2.3": {"yanked": False}}), \
             patch.object(release.subprocess, "run") as run:
            release.publish(False)
            run.assert_not_called()

    def test_yanked_crate_stops_before_any_upload(self):
        with patch.object(release, "publish_order", return_value=(["a", "b"], "1.2.3")), \
             patch.object(release.subprocess, "check_output", return_value=b""), \
             patch.object(release, "index_versions", side_effect=[{"1.0.0": {}}, {"1.0.0": {}}, {}, {"1.2.3": {"yanked": True}}]), \
             patch.object(release.subprocess, "run") as run:
            with self.assertRaisesRegex(SystemExit, "yanked"):
                release.publish(False)
            run.assert_not_called()

    def test_retry_after_header(self):
        import urllib.error
        error = urllib.error.HTTPError("https://crates.io", 429, "limit", {"Retry-After": "7"}, None)
        with patch.object(release.urllib.request, "urlopen", side_effect=[error, "response"]), \
             patch.object(release.time, "sleep") as sleep:
            self.assertEqual(release.request_with_retry("request"), "response")
            sleep.assert_called_once_with(7)


if __name__ == "__main__":
    unittest.main()
