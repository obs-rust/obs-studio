#!/usr/bin/env -S uv run --script
"""Kept Tier 2 build tree and local tier2-tree handoff on the Dell (#4954 P5, P7). No network.

    uv run --no-project python -m unittest discover -s ci -p test_tier2_kept_tree.py

A trusted merge-queue build keeps its tree on the host's tmpfs and replays the
source mtimes of unchanged files (`zccache snapshot` / `zccache replay`), so
the next run rebuilds only what changed. The tree is trusted only behind a
`.clean` marker written after a successful build. The test jobs take the
packed tree from a host handoff directory, verified by sha256, instead of a
193 s artifact upload, but only when their labels pin this host.
"""

from __future__ import annotations

import hashlib
import json
import os
import shutil
import subprocess
import tempfile
import threading
import time
import unittest
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
from unittest.mock import patch

import tier2_kept_tree as kt


class FakeZccache:
    """Records snapshot/replay calls; snapshot writes a manifest file."""

    def __init__(self, replay_json: dict | None = None, fail: bool = False):
        self.calls: list[list[str]] = []
        self.replay_json = replay_json or {"applied": 10, "total": 12}
        self.fail = fail

    def __call__(self, argv, **kwargs):
        self.calls.append([str(a) for a in argv])
        if self.fail:
            return subprocess.CompletedProcess(argv, 1, "", "boom")
        if argv[1] == "snapshot":
            out = Path(argv[argv.index("--out") + 1])
            out.write_text('{"files": []}')
            return subprocess.CompletedProcess(argv, 0, "", "")
        return subprocess.CompletedProcess(argv, 0, json.dumps(self.replay_json), "")


class KeptTreeTest(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.kept = self.tmp / "trees" / "dell-1" / "tier2"
        self.build = self.tmp / "temp" / "tier2"
        self.src = self.tmp / "ws"
        self.src.mkdir(parents=True)
        (self.src / "a.c").write_text("int a;")

    def tearDown(self):
        import shutil

        shutil.rmtree(self.tmp, ignore_errors=True)

    def built_tree(self):
        tree = self.build / "tree"
        (tree / "libobs").mkdir(parents=True)
        obj = tree / "libobs" / "a.o"
        obj.write_text("obj")
        os.utime(obj, ns=(1_700_000_000_000_000_000, 1_700_000_000_000_000_000))
        (self.build / "tree.rustc").write_text("rustc 1.95.0\n")
        return obj

    def test_save_then_restore_round_trips_tree_mtimes_and_replays_sources(self):
        self.built_tree()
        zcc = FakeZccache()
        saved = kt.save(self.kept, self.build, self.src, run=zcc)
        self.assertTrue(saved["saved"])
        self.assertTrue((self.kept / ".clean").is_file())
        self.assertFalse((self.build / "tree").exists(), "the tree moves, it is not copied twice")
        snap = zcc.calls[0]
        self.assertEqual(snap[1], "snapshot")
        self.assertEqual(snap[snap.index("--workspace") + 1], str(self.src))

        restored = kt.restore(self.kept, self.build, self.src, run=zcc)
        self.assertTrue(restored["restored"])
        self.assertEqual(restored["replay"], {"applied": 10, "total": 12})
        obj = self.build / "tree" / "libobs" / "a.o"
        self.assertEqual(obj.stat().st_mtime_ns, 1_700_000_000_000_000_000, "ninja needs the outputs' mtimes")
        self.assertEqual((self.build / "tree.rustc").read_text(), "rustc 1.95.0\n")
        replay = zcc.calls[1]
        self.assertEqual(replay[1], "replay")
        self.assertEqual(replay[replay.index("--manifest") + 1], str(self.kept / kt.MANIFEST))
        self.assertFalse((self.kept / ".clean").exists(), "an open tree is unclean until the next save")

    def test_restore_without_clean_marker_discards_the_tree(self):
        self.built_tree()
        kt.save(self.kept, self.build, self.src, run=FakeZccache())
        (self.kept / ".clean").unlink()  # a job died between restore and save
        zcc = FakeZccache()
        restored = kt.restore(self.kept, self.build, self.src, run=zcc)
        self.assertFalse(restored["restored"])
        self.assertFalse((self.kept / "tree").exists())
        self.assertFalse((self.build / "tree").exists())
        self.assertEqual(zcc.calls, [], "no replay onto a tree we do not trust")

    def test_restore_of_nothing_is_a_cold_start(self):
        self.assertFalse(kt.restore(self.kept, self.build, self.src, run=FakeZccache())["restored"])

    def test_failed_snapshot_leaves_no_clean_marker(self):
        self.built_tree()
        saved = kt.save(self.kept, self.build, self.src, run=FakeZccache(fail=True))
        self.assertFalse(saved["saved"])
        self.assertFalse((self.kept / ".clean").exists())

    def test_failed_replay_still_restores_with_fresh_mtimes(self):
        # Fresh mtimes only cost a rebuild; they can never skip one.
        self.built_tree()
        kt.save(self.kept, self.build, self.src, run=FakeZccache())
        restored = kt.restore(self.kept, self.build, self.src, run=FakeZccache(fail=True))
        self.assertTrue(restored["restored"])
        self.assertIsNone(restored["replay"])

    def test_snapshot_excludes_workspace_caches(self):
        self.built_tree()
        (self.src / ".tier2-modcache").mkdir()
        zcc = FakeZccache()
        kt.save(self.kept, self.build, self.src, run=zcc)
        excluded = [a for i, a in enumerate(zcc.calls[0]) if i and zcc.calls[0][i - 1] == "--exclude"]
        self.assertIn(str(self.src / ".tier2-modcache"), excluded)


class HandoffTest(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.root = self.tmp / "handoff"
        self.root.mkdir()
        self.pack = self.tmp / "pack"
        self.pack.mkdir()
        (self.pack / "tier2-tree.tar.zst").write_bytes(b"tree" * 1000)
        (self.pack / "runtime-packages.txt").write_text("libc6\n")
        self.marker = self.tmp / ".persistent-cache"
        self.marker.write_text("dell-fsx3mn2\n")

    def tearDown(self):
        import shutil

        shutil.rmtree(self.tmp, ignore_errors=True)

    def test_put_get_round_trip_with_verified_digests(self):
        labels = json.dumps(["self-hosted", "obs-ubuntu-2604", "dell-fsx3mn2"])
        put = kt.handoff_put(self.root, "42", self.pack, labels, self.marker)
        self.assertTrue(put["local"], "the test jobs' labels pin this host: skip the upload")
        sums = (self.root / "42" / "SHA256SUMS").read_text()
        self.assertIn(hashlib.sha256(b"tree" * 1000).hexdigest(), sums)
        dest = self.tmp / "dest"
        got = kt.handoff_get(self.root, "42", dest)
        self.assertTrue(got["hit"])
        self.assertEqual((dest / "tier2-tree.tar.zst").read_bytes(), b"tree" * 1000)
        self.assertEqual((dest / "runtime-packages.txt").read_text(), "libc6\n")

    def test_labels_that_do_not_pin_this_host_keep_the_upload(self):
        for labels in ("", "[]", json.dumps(["self-hosted", "obs-ubuntu-2604"]), "ubuntu-26.04"):
            with self.subTest(labels=labels):
                self.assertFalse(kt.handoff_put(self.root, "1", self.pack, labels, self.marker)["local"])
        self.marker.unlink()
        self.assertFalse(kt.handoff_put(self.root, "2", self.pack, '["dell-fsx3mn2"]', self.marker)["local"])

    def test_corrupt_or_missing_handoff_falls_back_to_the_artifact(self):
        dest = self.tmp / "dest"
        self.assertFalse(kt.handoff_get(self.root, "missing", dest)["hit"])
        kt.handoff_put(self.root, "7", self.pack, '["dell-fsx3mn2"]', self.marker)
        (self.root / "7" / "tier2-tree.tar.zst").write_bytes(b"truncated")
        self.assertFalse(kt.handoff_get(self.root, "7", dest)["hit"])
        self.assertFalse((dest / "tier2-tree.tar.zst").exists(), "no unverified file reaches the test job")

    def test_prune_drops_old_runs_and_bounds_the_total(self):
        now = time.time()
        for key, age in (("old", 2 * 86400), ("mid", 3600), ("new", 60)):
            kt.handoff_put(self.root, key, self.pack, "[]", self.marker)
            os.utime(self.root / key, (now - age, now - age))
        kt.prune(self.root, max_age_s=86400, max_bytes=10_000_000, now=now)
        self.assertEqual(sorted(p.name for p in self.root.iterdir() if not p.name.startswith(".")), ["mid", "new"])
        kt.prune(self.root, max_age_s=86400, max_bytes=5000, now=now)
        self.assertEqual(sorted(p.name for p in self.root.iterdir() if not p.name.startswith(".")), ["new"])

    def while_copy_paused(self, operation, concurrent):
        """Run a competitor inside a real copy, with bounded waits and no sleeps."""
        copying = threading.Event()
        resume = threading.Event()
        copy = shutil.copy2

        def paused_copy(src, dest, *args, **kwargs):
            copying.set()
            if not resume.wait(5):
                raise TimeoutError("copy was not resumed")
            return copy(src, dest, *args, **kwargs)

        with patch.object(kt.shutil, "copy2", side_effect=paused_copy), ThreadPoolExecutor(max_workers=1) as pool:
            future = pool.submit(operation)
            try:
                self.assertTrue(copying.wait(5), "operation did not reach its copy")
                concurrent()
            finally:
                resume.set()
            return future.result(timeout=5)

    def test_prune_skips_an_in_progress_write(self):
        def prune_stale_temporary_entry():
            # An old temporary entry is eligible for cleanup even before rename.
            tmp = next(self.root.glob(".42.*.tmp"))
            os.utime(tmp, (0, 0))
            kt.prune(self.root, max_bytes=0)
            self.assertTrue(tmp.is_dir(), "prune removed the active writer's temporary entry")

        put = self.while_copy_paused(
            lambda: kt.handoff_put(self.root, "42", self.pack, '["dell-fsx3mn2"]', self.marker),
            prune_stale_temporary_entry,
        )
        self.assertTrue(put["local"])
        self.assertTrue(kt.handoff_get(self.root, "42", self.tmp / "dest")["hit"])

    def test_prune_skips_a_reader_until_its_copy_finishes(self):
        kt.handoff_put(self.root, "42", self.pack, "[]", self.marker)

        def prune_during_copy():
            kt.prune(self.root, max_bytes=0)
            self.assertTrue((self.root / "42").is_dir(), "prune deleted an entry during its copy")

        got = self.while_copy_paused(
            lambda: kt.handoff_get(self.root, "42", self.tmp / "dest"), prune_during_copy,
        )
        self.assertTrue(got["hit"])
        self.assertEqual((self.tmp / "dest" / "tier2-tree.tar.zst").read_bytes(), b"tree" * 1000)
        kt.prune(self.root, max_bytes=0)
        self.assertFalse((self.root / "42").exists(), "reader must release its lock after copying")

    def test_concurrent_prune_skips_the_busy_round(self):
        kt.handoff_put(self.root, "42", self.pack, "[]", self.marker)
        scanning = threading.Event()
        resume = threading.Event()
        tree_bytes = kt._tree_bytes

        def paused_size(entry):
            size = tree_bytes(entry)
            if threading.current_thread() is not threading.main_thread():
                scanning.set()
                if not resume.wait(5):
                    raise TimeoutError("prune was not resumed")
            return size

        with patch.object(kt, "_tree_bytes", side_effect=paused_size), ThreadPoolExecutor(max_workers=1) as pool:
            future = pool.submit(kt.prune, self.root)
            try:
                self.assertTrue(scanning.wait(5), "prune did not reach its scan")
                kt.prune(self.root, max_bytes=0)
                self.assertTrue((self.root / "42").exists(), "a second prune ran while the first held the lock")
            finally:
                resume.set()
            future.result(timeout=5)

    def test_prune_tolerates_an_entry_vanishing_during_stat(self):
        gone = self.root / "gone"
        gone.mkdir()
        stat = Path.stat

        def vanished(path, *args, **kwargs):
            if path == gone:
                raise FileNotFoundError(path)
            return stat(path, *args, **kwargs)

        with patch.object(Path, "stat", new=vanished):
            kt.prune(self.root)

    def test_prune_io_error_does_not_undo_a_successful_publication(self):
        with patch.object(kt, "_tree_bytes", side_effect=OSError("cache scan failed")):
            put = kt.handoff_put(self.root, "42", self.pack, '["dell-fsx3mn2"]', self.marker)
        self.assertTrue(put["local"])
        self.assertTrue(kt.handoff_get(self.root, "42", self.tmp / "dest")["hit"])

    def test_prune_never_removes_the_lock_file(self):
        lock = self.root / ".lock"
        lock.touch()
        os.utime(lock, (0, 0))
        inode = lock.stat().st_ino
        kt.prune(self.root, max_bytes=0)
        self.assertEqual(lock.stat().st_ino, inode, "replacing the lock inode splits mutual exclusion")


if __name__ == "__main__":
    unittest.main()
