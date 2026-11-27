#!/usr/bin/env -S uv run --script
"""docker-task never prunes the image it is about to run (#1218), in Python.

The rule lives in ci/docker_task.py since the port off the shell script (#962).
A fake `docker` on PATH lists images that share one CreatedAt and logs every
removal, so a tie falls to the tag string exactly as the script's
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


class EntryPointTest(unittest.TestCase):
    """ci/docker_task.py is the entry point; the shell shim is gone (#962 GEN-005)."""

    def test_the_shell_shim_is_gone(self):
        self.assertFalse(
            (CI / "docker-task.sh").exists(),
            "ci/docker-task.sh is a GEN-005 violation (#962): run `uv run --script ci/docker_task.py`",
        )

    def test_every_caller_runs_the_module_through_uv(self):
        for caller in (
            ROOT / "cmake" / "cross" / "docker.py",
            ROOT / "rust" / "tools" / "linux-validate" / "run.py",
        ):
            with self.subTest(caller=str(caller.relative_to(ROOT))):
                text = caller.read_text(encoding="utf-8")
                self.assertNotIn("docker-task.sh", text)
                self.assertIn('"uv",', text)

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


# Per-run volumes for `gc --older-than` (#4638). The fake prints
# `docker volume ls -q`, answers `docker volume inspect -f ... NAME...` with
# "<name>\t<CreatedAt>" lines from $VOLUMES, lists the volumes containers
# mount from $MOUNTS for `docker ps -a ... --format {{.Mounts}}`, and logs
# `docker volume rm NAME`.
FAKE_DOCKER_VOLUMES = """#!/bin/sh
if [ "$1 $2" = "volume ls" ]; then
  cut -f1 "$VOLUMES"
elif [ "$1 $2" = "volume inspect" ]; then
  format="$4"
  shift 4
  for name in "$@"; do
    case "$format" in
      *docker-task.task*) grep "^$name	" "$VOLUMES" ;;
      *) grep "^$name	" "$VOLUMES" | cut -f1,2 ;;
    esac
  done
elif [ "$1" = "ps" ]; then
  cat "$MOUNTS"
elif [ "$1 $2" = "volume rm" ]; then
  printf '%s\\n' "$3" >> "$REMOVED"
fi
exit 0
"""

NOW = "2026-11-15T12:00:00+00:00"
OLD = "2026-11-03T07:30:59-05:00"
FRESH = "2026-11-15T11:29:00+00:00"


class GcOlderThan(unittest.TestCase):
    """#4638: `gc --older-than H` reclaims the per-run build and zccache
    volumes of checkouts that still exist, but never a key still in use."""

    def setUp(self):
        self.module = load_module()
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        root = Path(self.tmp.name)
        self.bin = root / "bin"
        self.bin.mkdir()
        docker = self.bin / "docker"
        docker.write_text(FAKE_DOCKER_VOLUMES, encoding="utf-8")
        docker.chmod(0o755)
        self.volumes = root / "volumes"
        self.mounts = root / "mounts"
        self.mounts.touch()
        self.removed = root / "removed"
        self.removed.touch()

    def gc(self, volumes: dict[str, str], mounts: str = "", args=("--older-than", "24", "--apply"),
           tasks: dict[str, str] | None = None) -> str:
        """`gc ARGS` against the fake docker at NOW; returns what it printed."""
        self.volumes.write_text("".join(f"{n}\t{c}\t{(tasks or {}).get(n, '')}\n" for n, c in volumes.items()),
                                encoding="utf-8")
        self.mounts.write_text(mounts, encoding="utf-8")
        env = {
            "PATH": f"{self.bin}{os.pathsep}{os.environ['PATH']}",
            "VOLUMES": str(self.volumes),
            "MOUNTS": str(self.mounts),
            "REMOVED": str(self.removed),
        }
        out = StringIO()
        with patch.dict(os.environ, env), redirect_stdout(out), patch.object(
            self.module, "now", lambda: self.module.parse_created(NOW)
        ):
            self.module.gc(list(args))
        return out.getvalue()

    def gone(self) -> list[str]:
        return sorted(self.removed.read_text(encoding="utf-8").split())

    def test_needs_no_task_name(self):
        self.gc({})
        self.assertEqual(self.gone(), [])

    def test_an_old_unused_pair_is_removed(self):
        self.gc({
            "obs-rust-linux-validate-build-1e26c0a3ee51": OLD,
            "obs-rust-linux-validate-zccache-1e26c0a3ee51": OLD,
            "obs-studio10-output-error-validate-build-6a4d99a369ca": OLD,
            "obs-rust-linux-validate-obs-studio10-build-6a4d99a369ca": OLD,
        })
        self.assertEqual(self.gone(), [
            "obs-rust-linux-validate-build-1e26c0a3ee51",
            "obs-rust-linux-validate-obs-studio10-build-6a4d99a369ca",
            "obs-rust-linux-validate-zccache-1e26c0a3ee51",
            "obs-studio10-output-error-validate-build-6a4d99a369ca",
        ])

    def test_an_old_build_volume_with_a_fresh_partner_is_kept(self):
        # The case from the report: build-cf38… 40 h old, zccache-cf38… 31 min.
        self.gc({
            "obs-rust-linux-validate-build-cf38ef20b89e": OLD,
            "obs-rust-linux-validate-zccache-cf38ef20b89e": FRESH,
        })
        self.assertEqual(self.gone(), [])

    def test_an_old_pair_with_a_partner_in_use_is_kept(self):
        self.gc(
            {
                "obs-rust-linux-validate-build-cf38ef20b89e": OLD,
                "obs-rust-linux-validate-zccache-cf38ef20b89e": OLD,
                "obs-rust-linux-validate-obs3-soldr-cf38ef20b89e": OLD,
            },
            mounts="/src,obs-rust-linux-validate-obs3-soldr-cf38ef20b89e\n",
        )
        self.assertEqual(self.gone(), [])

    def test_a_volume_a_stopped_container_mounts_is_kept(self):
        self.gc(
            {"obs-rust-linux-validate-build-1e26c0a3ee51": OLD},
            mounts="obs-rust-linux-validate-build-1e26c0a3ee51,obs-rust-linux-validate-cargo-git\n",
        )
        self.assertEqual(self.gone(), [])

    def test_only_build_and_zccache_volumes_are_removed(self):
        self.gc({
            "obs-rust-linux-validate-cargo-registry": OLD,
            "obs-rust-linux-validate-obs3-soldr-5dd0892dd40b": OLD,
            "obs-rust-linux-build-5dd0892dd40b": OLD,
            "docker-task-soldr-0.9.1": OLD,
        })
        self.assertEqual(self.gone(), [])

    def test_a_dry_run_only_lists(self):
        printed = self.gc(
            {"obs-rust-linux-validate-build-1e26c0a3ee51": OLD},
            args=("--older-than", "24"),
        )
        self.assertEqual(self.gone(), [])
        self.assertIn("obs-rust-linux-validate-build-1e26c0a3ee51", printed)

    def test_older_than_takes_hours(self):
        self.gc({"obs-rust-linux-validate-build-1e26c0a3ee51": FRESH}, args=("--older-than", "0", "--apply"))
        self.assertEqual(self.gone(), ["obs-rust-linux-validate-build-1e26c0a3ee51"])
        for bad in ("x", "-1"):
            with self.subTest(bad=bad), self.assertRaises(SystemExit):
                self.module.parse_gc(["--older-than", bad])

    def test_gc_without_name_or_age_is_a_usage_error(self):
        with self.assertRaises(SystemExit):
            self.module.parse_gc([])

    def test_all_tasks_requires_opt_in_and_an_ownership_label(self):
        build = "obs-studio2-extract-gstex-build-ee6074b20403"
        cache = "obs-studio2-swap-4678-zccache-ee6074b20403"
        unowned = "unrelated-build-ee6074b20403"
        registry = "obs-studio2-cargo-registry-ee6074b20403"
        legacy = "obs-rust-linux-validate-build-5dd0892dd40b"
        volumes = dict.fromkeys((build, cache, unowned, registry, legacy), OLD)
        tasks = {build: "obs-studio2-extract-gstex", cache: "obs-studio2-swap-4678", registry: "obs-studio2"}
        self.gc({build: OLD, cache: OLD}, tasks=tasks)
        self.assertEqual(self.gone(), [])
        self.gc(volumes, tasks=tasks, args=("--older-than", "24", "--all-tasks", "--apply"))
        self.assertEqual(self.gone(), sorted((build, cache)))

    def test_all_tasks_keeps_a_key_with_a_fresh_unknown_or_mounted_partner(self):
        build = "obs-studio2-extract-gstex-build-ee6074b20403"
        partner = "another-task-cargo-ee6074b20403"
        for stamp, mounts in ((FRESH, ""), ("unknown", ""), (OLD, partner + "\n"), (OLD, build + "\n")):
            with self.subTest(stamp=stamp, mounts=mounts):
                self.gc({build: OLD, partner: stamp}, mounts=mounts, tasks={build: "obs-studio2-extract-gstex"},
                        args=("--older-than", "24", "--all-tasks", "--apply"))
                self.assertEqual(self.gone(), [])

    def test_all_tasks_dry_run_only_lists_owned_candidates(self):
        build = "obs-studio2-extract-gstex-build-ee6074b20403"
        unowned = "unrelated-zccache-ee6074b20403"
        printed = self.gc({build: OLD, unowned: OLD}, tasks={build: "obs-studio2-extract-gstex"},
                          args=("--older-than", "24", "--all-tasks"))
        self.assertEqual(self.gone(), [])
        self.assertIn(build, printed)
        self.assertNotIn(unowned, printed)

    def test_all_tasks_needs_an_age_even_with_a_task_name(self):
        with self.assertRaises(SystemExit) as error:
            self.module.parse_gc(["--name", "obs-task", "--all-tasks"])
        self.assertEqual(error.exception.code, 2)


if __name__ == "__main__":
    unittest.main()
