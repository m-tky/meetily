import hashlib
import importlib.util
import io
import pathlib
import tempfile
import unittest
from unittest import mock

spec = importlib.util.spec_from_file_location('prepare_model', pathlib.Path(__file__).with_name('prepare-qwen-model.py'))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class ModelPreparationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = pathlib.Path(self.temp.name)
        self.manifest = {'id': 'fixture/model', 'revision': 'fixture', 'files': [
            {'file': 'config.json', 'bytes': 1, 'sha256': hashlib.sha256(b'x').hexdigest()}]}

    def test_check_does_not_download_initialize_or_write(self):
        (self.root / 'config.json').write_bytes(b'x')
        (self.root / 'qwen-asr-int8.sidecar').touch()
        before = sorted(p.name for p in self.root.iterdir())
        with mock.patch.object(module.urllib.request, 'urlopen') as download, mock.patch.object(module.subprocess, 'run') as initialize:
            module.prepare(self.root, self.manifest, 'fixture', check=True)
            download.assert_not_called()
            initialize.assert_not_called()
        self.assertEqual(sorted(p.name for p in self.root.iterdir()), before)

    def test_existing_mismatch_is_preserved(self):
        path = self.root / 'config.json'
        path.write_bytes(b'changed')
        with self.assertRaises(SystemExit):
            module.prepare(self.root, self.manifest, 'fixture')
        self.assertEqual(path.read_bytes(), b'changed')

    def test_offline_missing_file_never_downloads(self):
        with mock.patch.object(module.urllib.request, 'urlopen') as download:
            with self.assertRaises(SystemExit):
                module.prepare(self.root, self.manifest, 'fixture', offline=True)
            download.assert_not_called()

    def test_verified_download_and_initialization(self):
        def initialize(*args, **kwargs):
            (self.root / 'qwen-asr-int8.sidecar').touch()
        with mock.patch.object(module.urllib.request, 'urlopen', return_value=io.BytesIO(b'x')), mock.patch.object(module.subprocess, 'run', side_effect=initialize):
            module.prepare(self.root, self.manifest, 'fixture')
        self.assertEqual((self.root / 'config.json').read_bytes(), b'x')
        self.assertFalse(list(self.root.glob('.qwen-download-*')))

    def test_failed_download_is_not_published(self):
        with mock.patch.object(module.urllib.request, 'urlopen', return_value=io.BytesIO(b'y')):
            with self.assertRaises(SystemExit):
                module.prepare(self.root, self.manifest, 'fixture')
        self.assertFalse((self.root / 'config.json').exists())
        self.assertFalse(list(self.root.glob('.qwen-download-*')))


if __name__ == '__main__':
    unittest.main()
