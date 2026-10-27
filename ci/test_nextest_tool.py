"""Pinned test-tool archives must be verified before installing executable bytes."""
import hashlib
import io
from pathlib import Path
import tarfile
import tempfile
import unittest

import importlib.util

spec = importlib.util.spec_from_file_location('nextest_tool_test_target', Path(__file__).with_name('nextest_tool.py'))
nextest_tool = importlib.util.module_from_spec(spec)
spec.loader.exec_module(nextest_tool)


class NextestToolTest(unittest.TestCase):
    def archive(self, root, kind=tarfile.REGTYPE, name='cargo-nextest'):
        path = root / 'tool.tar.gz'
        with tarfile.open(path, 'w:gz') as archive:
            member = tarfile.TarInfo(name)
            member.type = kind
            member.linkname = '/outside'
            data = b'verified executable bytes'
            member.size = len(data) if kind == tarfile.REGTYPE else 0
            archive.addfile(member, io.BytesIO(data) if member.size else None)
        return path, hashlib.sha256(path.read_bytes()).hexdigest()

    def test_validated_regular_binary_installs_without_extracting_other_paths(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            archive, digest = self.archive(root)
            output = root / 'bin' / 'cargo-nextest'
            nextest_tool.unpack_checked(archive, output, digest)
            self.assertEqual(output.read_bytes(), b'verified executable bytes')
            self.assertTrue(output.stat().st_mode & 0o100)

    def test_archive_checksum_mismatch_cannot_replace_an_existing_binary(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            archive, _ = self.archive(root)
            output = root / 'cargo-nextest'
            output.write_bytes(b'old')
            with self.assertRaisesRegex(ValueError, 'checksum'):
                nextest_tool.unpack_checked(archive, output, '0' * 64)
            self.assertEqual(output.read_bytes(), b'old')

    def test_link_entries_are_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            archive, digest = self.archive(root, tarfile.SYMTYPE)
            with self.assertRaises(ValueError):
                nextest_tool.unpack_checked(archive, root / 'cargo-nextest', digest)

    def test_unsupported_hosts_do_not_download_or_change_the_cargo_runner(self):
        from unittest import mock
        with mock.patch.object(nextest_tool.host, 'is_linux', return_value=False):
            self.assertIsNone(nextest_tool.ensure_nextest())


if __name__ == '__main__':
    unittest.main()
