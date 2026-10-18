"""Check that advisory exceptions fail closed when their evidence changes."""
import importlib.util
from datetime import date
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('audit', Path(__file__).with_name('check-zip-advisories.py'))
audit = importlib.util.module_from_spec(spec)
spec.loader.exec_module(audit)


class ZipAssessmentTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        root = Path(self.temp.name)
        (root / 'src').mkdir()
        self.source = root / 'src/lib.rs'
        self.source.write_text('pub fn read() {}')
        self.policy = {'review_before': '2026-11-18', 'zip_version': '8.6.0',
                       'readers': {'reader': {'version': '1.0.0', 'source_sha256': audit.source_digest(root)}}}
        self.data = {'packages': [
            {'id': 'zip', 'name': 'zip', 'version': '8.6.0'},
            {'id': 'reader', 'name': 'reader', 'version': '1.0.0',
             'source': 'registry+https://github.com/rust-lang/crates.io-index',
             'manifest_path': str(root / 'Cargo.toml')}],
            'resolve': {'nodes': [{'id': 'reader', 'deps': [{'pkg': 'zip'}]}]}}

    def check(self):
        audit.check(self.data, self.policy, date(2026, 10, 18))

    def test_reviewed_readers_pass(self):
        self.check()

    def test_expired_assessment_fails(self):
        with self.assertRaisesRegex(ValueError, 'expired'):
            audit.check(self.data, self.policy, date(2026, 11, 18))

    def test_new_caller_requires_review(self):
        self.data['packages'].append({'id': 'new', 'name': 'new', 'version': '1.0.0'})
        self.data['resolve']['nodes'].append({'id': 'new', 'deps': [{'pkg': 'zip'}]})
        with self.assertRaisesRegex(ValueError, 'parents changed'):
            self.check()

    def test_changed_source_requires_review(self):
        self.source.write_text('pub fn extract() {}')
        with self.assertRaisesRegex(ValueError, 'source differs'):
            self.check()

    def test_dependency_update_requires_review(self):
        self.data['packages'][1]['version'] = '1.0.1'
        with self.assertRaisesRegex(ValueError, 'parents changed'):
            self.check()

    def test_patched_zip_requires_exception_removal(self):
        self.data['packages'][0]['version'] = '9.0.5'
        with self.assertRaisesRegex(ValueError, 'ZIP versions changed'):
            self.check()

    def test_git_source_requires_review(self):
        self.data['packages'][1]['source'] = 'git+https://example.invalid'
        with self.assertRaisesRegex(ValueError, 'no longer a crates.io'):
            self.check()
