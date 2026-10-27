#!/usr/bin/env -S uv run --script
"""docker-task never prunes the image it is about to run (#1218), in Python.

The rule lives in ci/docker_task.py since the port off the shell script (#962);
`ci/docker-task.sh` is now a shim that execs that file through uv. A fake
`docker` on PATH lists images that share one CreatedAt and logs every removal,
so a tie falls to the tag string exactly as the script's
`LC_ALL=C sort -r | cut -f2` had it: the image just built sorts last and must
not be removed before its container starts.

    uv run python -m unittest ci/test_docker_task_prune.py
"""

from __future__ import annotations

import hashlib
import importlib.util
import os
import subprocess
import sys
import tempfile
import unittest
from contextlib import redirect_stdout
from io import StringIO
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parent.parent
CI = ROOT / "ci"
MODULE = CI / "docker_task.py"
SHIM = CI / "docker-task.sh"

# `image ls` prints four tags with one CreatedAt: the reverse sort puts 9999
# first and 0000 last, and 0000 is the image the run just built and is about to
# start. `image rm` logs what it was asked to remove.
FAKE_DOCKER = """#!/bin/sh
if [ "$1 $2" = "image ls" ]; then
  for t in 0000 1111 2222 9999; do
    printf '2026-01-01 00:00:00 +0000 UTC\\t%s\\n' "$t"
  done
elif [ "$1 $2" = "image rm" ]; then
  printf '%s\\n' "$3" >> "$REMOVED"
fi
exit 0
"""


def load_module():
    """ci/docker_task.py by path: `uv run python -m unittest
    ci/test_docker_task_prune.py` does not put ci/ on sys.path, and no file in
    this repo edits sys.path (ci/test_no_sys_path_edits.py)."""
    spec = importlib.util.spec_from_file_location("docker_task", MODULE)
    assert spec is not None and spec.loader is not None, f"cannot load {MODULE}"
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class ShimTest(unittest.TestCase):
    """ci/docker-task.sh keeps its path and arguments; the logic is Python."""

    def test_the_shim_execs_the_python_module(self):
        text = SHIM.read_text(encoding="utf-8")
        self.assertIn("ci/docker_task.py", text)
        self.assertNotIn("prune_images", text)
        self.assertNotIn("docker run", text)

    def test_the_entry_point_reports_usage_errors_as_docker_task(self):
        for args in (["run"], ["gc", "--bogus"]):
            with self.subTest(args=args):
                result = subprocess.run(
                    [sys.executable, str(MODULE), *args], capture_output=True, text=True
                )
                self.assertEqual(result.returncode, 2, result.stderr)
                self.assertTrue(result.stderr.startswith("docker-task: "), result.stderr)


class PureHelperTest(unittest.TestCase):
    """The pieces a port has to keep: the keys, the sizes and the hash."""

    def setUp(self):
        self.module = load_module()

    def test_checkout_id_is_twelve_hex_of_the_repo_path(self):
        self.assertEqual(
            self.module.checkout_id("/srv/checkout"), hashlib.sha256(b"/srv/checkout").hexdigest()[:12]
        )
        self.assertRegex(self.module.checkout_id("/srv/checkout"), r"^[0-9a-f]{12}$")

    def test_to_bytes_parses_the_documented_sizes(self):
        self.assertEqual(self.module.to_bytes("512"), 512)
        self.assertEqual(self.module.to_bytes("300K"), 300 * 1024)
        self.assertEqual(self.module.to_bytes("20M"), 20 * 1024 * 1024)
        self.assertEqual(self.module.to_bytes("8G"), 8 * 1024 * 1024 * 1024)

    def test_image_hash_covers_the_dockerfile_context_and_build_args(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            dockerfile = root / "Dockerfile"
            dockerfile.write_text("FROM scratch\n")
            context = root / "context"
            (context / "sub").mkdir(parents=True)
            (context / "a.txt").write_text("a\n")
            (context / "sub" / "b.txt").write_text("b\n")
            # The shell hashed `cat Dockerfile` + every context file in
            # LC_ALL=C order + `printf '%s\n' <build-args>`.
            expected = hashlib.sha256(b"FROM scratch\n" + b"a\n" + b"b\n" + b"WITH_X=1\n")
            args = ["WITH_X=1"]
            self.assertEqual(self.module.image_hash(dockerfile, context, args), expected.hexdigest()[:16])
            self.assertEqual(len(self.module.image_hash(dockerfile, context, [])), 16)
            for changed in (
                self.module.image_hash(dockerfile, context, ["WITH_X=2"]),
                self.module.image_hash(dockerfile, context, ["WITH_X=1", "Y=2"]),
                self.module.image_hash(dockerfile, context, []),
            ):
                self.assertNotEqual(changed, expected.hexdigest()[:16])
            (context / "a.txt").write_text("changed\n")
            self.assertNotEqual(self.module.image_hash(dockerfile, context, args), expected.hexdigest()[:16])

    def test_ordered_tags_is_newest_first_with_ties_reversed(self):
        listing = (
            "2026-01-01 00:00:00 +0000 UTC\t0000\n"
            "2026-01-01 00:00:00 +0000 UTC\t3333\n"
            "2025-12-31 00:00:00 +0000 UTC\t9999\n"
            "2026-01-01 00:00:00 +0000 UTC\t<none>\n"
            "2026-01-01 00:00:00 +0000 UTC\t1111\n"
        )
        self.assertEqual(self.module.ordered_tags(listing), ["3333", "1111", "0000", "9999"])


class PruneKeepsCurrentImage(unittest.TestCase):
    """#1218: prune_images never removes the tag it was told is in use."""

    def setUp(self):
        self.module = load_module()
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.bin = Path(self.tmp.name) / "bin"
        self.bin.mkdir()
        docker = self.bin / "docker"
        docker.write_text(FAKE_DOCKER, encoding="utf-8")
        docker.chmod(0o755)
        self.removed = Path(self.tmp.name) / "removed"
        self.removed.touch()

    def prune(self, *args) -> str:
        """prune_images with the fake docker on PATH; returns what it printed."""
        env = {"PATH": f"{self.bin}{os.pathsep}{os.environ['PATH']}", "REMOVED": str(self.removed)}
        out = StringIO()
        with patch.dict(os.environ, env), redirect_stdout(out):
            self.module.prune_images(*args)
        return out.getvalue()

    def test_image_just_built_survives(self):
        self.prune("obs-task", 1, True, "obs-task:0000")
        gone = self.removed.read_text(encoding="utf-8").split()
        self.assertNotIn("obs-task:0000", gone)
        # The newest tag, plus the image in use which is not counted.
        self.assertEqual(sorted(gone), ["obs-task:1111", "obs-task:2222"])

    def test_a_dry_run_only_lists_what_would_go(self):
        printed = self.prune("obs-task", 2, False, "obs-task:0000")
        self.assertEqual(self.removed.read_text(encoding="utf-8"), "")
        self.assertIn("image obs-task:1111 (beyond the newest 2)", printed)
        self.assertNotIn("obs-task:3333", printed)


if __name__ == "__main__":
    unittest.main()
