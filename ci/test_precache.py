#!/usr/bin/env -S uv run --script
"""The cache transport must be reproducible, verifiable and lossless.

`fetch_libs_asset.fetch` already reads a pinned release asset and verifies its
sha256; these tests pin the half that did not exist: packing a cache tree so the
digest is stable, and carrying the pin in a manifest the consumer can trust.
"""
from __future__ import annotations

import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest

import precache

# ci/ tests run under the gate's interpreter (3.13 today) while the tool itself
# declares requires-python >= 3.14 for `compression.zstd`, the same split
# fetch_libs_asset.py already lives with. Packing is exercised when 3.14 is the
# interpreter; the naming and manifest halves always run.
HAS_ZSTD = importlib.util.find_spec('compression') is not None

ROOT = Path(__file__).resolve().parent.parent


def sample_tree(base: Path) -> Path:
    tree = base / 'cache'
    (tree / 'nested').mkdir(parents=True)
    (tree / 'a.o').write_bytes(b'object' * 100)
    (tree / 'nested' / 'b.rlib').write_bytes(b'library' * 50)
    (tree / 'empty').mkdir()
    return tree


class NamingTest(unittest.TestCase):
    def setUp(self):
        self.precache = precache

    def test_asset_and_manifest_follow_the_libs_convention(self):
        self.assertEqual(self.precache.asset_name('tier2-ccache', 'windows-x64', 'abc123'),
                         'cache-tier2-ccache-windows-x64-abc123.tar.zst')
        self.assertEqual(self.precache.manifest_name('tier2-ccache', 'windows-x64', 'abc123'),
                         'cache-tier2-ccache-windows-x64-abc123.manifest.json')

    def test_each_platform_gets_its_own_asset(self):
        names = {self.precache.asset_name('app', platform, 'id')
                 for platform in ('linux-x64', 'windows-x64', 'macos-arm64')}
        self.assertEqual(len(names), 3)


@unittest.skipUnless(HAS_ZSTD, 'pack needs Python 3.14 (compression.zstd)')
class PackTest(unittest.TestCase):
    def setUp(self):
        self.precache = precache

    def test_the_same_tree_packs_to_the_same_digest(self):
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            tree = sample_tree(base)
            first = base / 'first.tar.zst'
            second = base / 'second.tar.zst'
            digest_a, entries_a = self.precache.pack(tree, first)
            digest_b, entries_b = self.precache.pack(tree, second)
        self.assertEqual(digest_a, digest_b, 'a re-push of an unchanged cache must be a no-op')
        self.assertEqual(entries_a, entries_b)

    def test_modification_times_survive_a_round_trip(self):
        # cargo and zccache compare mtimes, so normalising them would publish a
        # cache that looks valid and silently rebuilds everything.
        from fetch_libs_asset import extract_tar_zst

        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            tree = sample_tree(base)
            stamp = 1_600_000_000
            assert os.utime(tree / 'a.o', (stamp, stamp)) is None
            archive = base / 'cache.tar.zst'
            self.precache.pack(tree, archive)
            restored = base / 'restored'
            restored.mkdir()
            extract_tar_zst(archive, restored)
            self.assertEqual((restored / 'a.o').stat().st_size, 600)
            self.assertEqual(int((restored / 'a.o').stat().st_mtime), stamp)
            self.assertTrue((restored / 'nested' / 'b.rlib').is_file())
            self.assertTrue((restored / 'empty').is_dir())


class ManifestTest(unittest.TestCase):
    def setUp(self):
        self.precache = precache

    def test_manifest_carries_the_pin_the_consumer_needs(self):
        text = self.precache.manifest('app', 'macos-arm64', 'deadbeef',
                                      'a' * 64, 1234, 7)
        data = self.precache.parse_manifest(text)
        self.assertEqual(data['asset'], 'cache-app-macos-arm64-deadbeef.tar.zst')
        self.assertEqual(data['sha256'], 'a' * 64)
        self.assertEqual(data['bytes'], 1234)
        self.assertEqual(data['entries'], 7)

    def test_manifest_is_sorted_so_diffs_are_readable(self):
        text = self.precache.manifest('app', 'linux-x64', 'x', 'b' * 64, 1, 1)
        keys = [line.split('"')[1] for line in text.splitlines() if '":' in line]
        self.assertEqual(keys, sorted(keys))

    def test_a_manifest_without_a_digest_is_refused(self):
        with self.assertRaises(ValueError):
            self.precache.parse_manifest(json.dumps({'asset': 'a.tar.zst', 'identity': 'x'}))

    def test_a_manifest_without_an_identity_is_refused(self):
        with self.assertRaises(ValueError):
            self.precache.parse_manifest(json.dumps({'asset': 'a.tar.zst', 'sha256': 'c' * 64}))


if __name__ == '__main__':
    unittest.main()
