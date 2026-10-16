import json
import pathlib
import stat
import subprocess
import sys
import tempfile
import unittest

SCRIPT = pathlib.Path(__file__).with_name('update-qwen-paths.py')


class QwenPathTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = pathlib.Path(self.temp.name)
        self.path = self.root / 'config' / 'qwen-asr.json'
        self.path.parent.mkdir()
        self.worker = self.root / 'worker'
        self.ort = self.root / 'ort.so'
        self.model = self.root / 'model'
        self.worker.touch()
        self.ort.touch()
        self.model.mkdir()

    def run_script(self):
        return subprocess.run([sys.executable, str(SCRIPT), str(self.path), str(self.worker),
                               str(self.model), str(self.ort)], capture_output=True)

    def test_preserves_other_keys_and_original_backup(self):
        before = {'worker_path': '/old', 'model_dir': '/old-model', 'extra': 'fixture'}
        original = json.dumps(before).encode()
        self.path.write_bytes(original)
        self.assertEqual(self.run_script().returncode, 0)
        after = json.loads(self.path.read_text())
        self.assertEqual(after['extra'], 'fixture')
        self.assertEqual(after['worker_path'], str(self.worker))
        self.assertEqual(after['model_dir'], str(self.model))
        self.assertEqual(after['ort_library'], str(self.ort))
        self.assertEqual(stat.S_IMODE(self.path.stat().st_mode), 0o600)
        backup = self.path.with_name('qwen-asr.pre-home-manager.json')
        self.assertEqual(backup.read_bytes(), original)
        self.assertEqual(self.run_script().returncode, 0)
        self.assertEqual(backup.read_bytes(), original)

    def test_missing_model_does_not_modify_settings(self):
        self.path.write_text('{}')
        self.model.rmdir()
        self.assertNotEqual(self.run_script().returncode, 0)
        self.assertEqual(self.path.read_text(), '{}')

    def test_symlink_is_not_replaced(self):
        other = self.root / 'other.json'
        other.write_text('{}')
        self.path.symlink_to(other)
        self.assertNotEqual(self.run_script().returncode, 0)
        self.assertTrue(self.path.is_symlink())
        self.assertEqual(other.read_text(), '{}')

    def test_invalid_settings_are_not_replaced(self):
        self.path.write_text('invalid')
        self.assertNotEqual(self.run_script().returncode, 0)
        self.assertEqual(self.path.read_text(), 'invalid')

    def test_new_configuration(self):
        self.assertEqual(self.run_script().returncode, 0)
        self.assertEqual(json.loads(self.path.read_text())['model_dir'], str(self.model))
        self.assertFalse(self.path.with_name('qwen-asr.pre-home-manager.json').exists())


if __name__ == '__main__':
    unittest.main()
