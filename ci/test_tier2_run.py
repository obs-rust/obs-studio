#!/usr/bin/env -S uv run --script
"""Tests for rust/tools/linux-validate/run.py (obs-rust/obs-studio#135, #962).

    uv run python -m unittest ci/test_tier2_run.py

run.py is executed from two throwaway checkouts with stub `docker` and `git`
commands on PATH that record their arguments, so no container is started.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TOOLS = ROOT / "rust" / "tools" / "linux-validate"

# STUB_STATE: what `docker container inspect` reports for the task's
# container (empty: no such container).
STUB = """#!/bin/sh
printf '%s\\n' "$*" >> "$STUB_LOG"
case "$*" in
"container inspect"*) [ -n "$STUB_STATE" ] && printf '%s\\n' "$STUB_STATE" ;;
esac
exit 0
"""


class Tier2VolumeTest(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="tier2-run-"))
        self.bin = self.tmp / "bin"
        self.bin.mkdir()
        for name in ("docker", "git"):
            stub = self.bin / name
            stub.write_text(STUB)
            stub.chmod(0o755)

    def tearDown(self):
        shutil.rmtree(self.tmp)

    def checkout(self, name: str) -> Path:
        """A fake checkout holding a copy of the linux-validate tools."""
        repo = self.tmp / name
        dest = repo / "rust" / "tools" / "linux-validate"
        shutil.copytree(TOOLS, dest)
        (repo / "ci").mkdir()
        # The task runner shim and the Python module it execs (ci/docker_task.py).
        for name in ("docker-task.sh", "docker_task.py"):
            shutil.copy(ROOT / "ci" / name, repo / "ci" / name)
        (repo / "cmake" / "common").mkdir(parents=True)
        shutil.copy(ROOT / "cmake" / "common" / "soldr.cmake", repo / "cmake" / "common" / "soldr.cmake")
        shutil.copy(ROOT / "rust-toolchain.toml", repo / "rust-toolchain.toml")
        for sub in ("plugins/obs-browser", "plugins/obs-websocket"):
            (repo / sub).mkdir(parents=True)
            (repo / sub / "placeholder").write_text("")
        return repo

    def docker_run_mounts(self, repo: Path, extra_env=None) -> dict[str, str]:
        self.calls = getattr(self, "calls", 0) + 1
        log = self.tmp / f"{repo.name}-{self.calls}.log"
        self.last_log = log
        env = {k: v for k, v in os.environ.items() if not k.startswith(("OBS_TIER2", "OBS_CCACHE", "OBS_ZCCACHE"))}
        env.update(PATH=f"{self.bin}:{env['PATH']}", STUB_LOG=str(log), STUB_STATE="", XDG_STATE_HOME=str(self.tmp / "state"), **(extra_env or {}))
        subprocess.run([sys.executable, str(repo / "rust/tools/linux-validate/run.py"), *getattr(self, "run_args", [])], env=env, check=True)
        # The task's own run carries --name; helper runs (the soldr tools
        # fetch, du for volume budgets) don't.
        runs = [line.split() for line in log.read_text().splitlines() if line.startswith("run ") and " --name " in line]
        self.assertEqual(len(runs), 1, log.read_text())
        args = runs[0]
        mounts = {}
        for flag, value in zip(args, args[1:]):
            if flag == "-v":
                source, target = value.split(":")[:2]
                mounts[target] = source
        return mounts

    def test_build_volume_is_per_checkout(self):
        a = self.docker_run_mounts(self.checkout("a"))
        b = self.docker_run_mounts(self.checkout("b"))
        self.assertNotEqual(a["/build"], b["/build"])
        self.assertNotEqual(a["/build"], "obs-rust-linux-validate-build")

    def test_build_volume_is_stable(self):
        repo = self.checkout("a")
        self.assertEqual(self.docker_run_mounts(repo)["/build"], self.docker_run_mounts(repo)["/build"])

    def test_source_is_read_only_and_not_copied(self):
        """The checkout is built out of source from the read-only mount;
        nothing copies it (the rsync into /src is gone)."""
        repo = self.checkout("a")
        self.docker_run_mounts(repo)
        run = [l for l in self.last_log.read_text().splitlines() if l.startswith("run ") and " --name " in l][0]
        self.assertIn(f"-v {repo}:/ro:ro", run)
        text = (ROOT / "rust/tools/linux-validate/validate.sh").read_text()
        self.assertNotIn("rsync", text)
        self.assertIn('SRC="${OBS_TIER2_SRC:-/ro}"', text)
        self.assertIn("-DOBS_BUILD_NUMBER=1", text)

    def test_container_is_named_per_checkout_and_removed_after(self):
        """zackees/bosn#666, emulated by ci/docker-task.sh (its logic in
        ci/docker_task.py): one ephemeral container per task and checkout, its
        name the singleton lock."""
        repo = self.checkout("a")
        self.docker_run_mounts(repo)
        run = [l.split() for l in self.last_log.read_text().splitlines() if l.startswith("run ") and " --name " in l][0]
        self.assertIn("--rm", run)
        name = run[run.index("--name") + 1]
        self.assertRegex(name, r"^obs-rust-linux-validate-[0-9a-f]{12}$")
        self.assertTrue(self.docker_run_mounts(repo)["/build"].endswith(name.rsplit("-", 1)[1]))

    def run_sh(self, repo, *args, state=""):
        log = self.tmp / f"{repo.name}-direct.log"
        env = {k: v for k, v in os.environ.items() if not k.startswith(("OBS_TIER2", "OBS_CCACHE", "OBS_ZCCACHE"))}
        env.update(PATH=f"{self.bin}:{env['PATH']}", STUB_LOG=str(log), STUB_STATE=state, XDG_STATE_HOME=str(self.tmp / "state"))
        r = subprocess.run([sys.executable, str(repo / "rust/tools/linux-validate/run.py"), *args], env=env, capture_output=True, text=True)
        return r, log.read_text() if log.exists() else ""

    def test_second_run_of_a_checkout_refuses_while_one_runs(self):
        r, log = self.run_sh(self.checkout("a"), state="running")
        self.assertEqual(r.returncode, 75, r.stderr)
        self.assertIn("already running", r.stderr)
        self.assertNotIn(" --name ", log)

    def test_stopped_leftover_container_is_removed(self):
        r, log = self.run_sh(self.checkout("a"), state="exited")
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertRegex(log, r"(?m)^rm -f obs-rust-linux-validate-[0-9a-f]{12}$")

    def test_clean_starts_the_build_volume_over(self):
        r, log = self.run_sh(self.checkout("a"), "--clean")
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertRegex(log, r"(?m)^volume rm obs-rust-linux-validate-build-[0-9a-f]{12}$")
        self.assertNotRegex(log, r"(?m)^volume rm obs-rust-linux-validate-ccache$")
        run = [l for l in log.splitlines() if l.startswith("run ") and " --name " in l][0]
        self.assertNotIn("--clean", run)  # run.sh's flag, not validate.sh's

    def test_image_is_tagged_by_content_hash(self):
        self.docker_run_mounts(self.checkout("a"))
        build = [l.split() for l in self.last_log.read_text().splitlines() if l.startswith("build ")][0]
        self.assertRegex(build[build.index("-t") + 1], r"^obs-rust-linux-validate:[0-9a-f]{16}$")

    def test_volumes_have_budgets_and_cargo_targets_are_pruned(self):
        r, log = self.run_sh(self.checkout("a"))
        self.assertEqual(r.returncode, 0, r.stderr)
        run = [l for l in log.splitlines() if l.startswith("run ") and " --name " in l][0]
        self.assertIn("DOCKER_TASK_PRUNE=/build/tree/cargo/build /build/tree-debug/cargo/build", run)
        self.assertNotIn("CARGO_INCREMENTAL=0", run)  # incremental stays on; the prune bounds it
        self.assertIn("prune-target --keep-latest --force", log)
        self.assertRegex(run, r"-v docker-task-soldr-[0-9]+\.[0-9]+\.[0-9]+:/opt/docker-task-soldr:ro")
        # One du per budgeted volume after the run...
        self.assertEqual(sum(1 for l in log.splitlines() if "--entrypoint du" in l), 4, log)
        # ...and none again within the hour (DOCKER_TASK_BUDGET_SECS).
        (self.tmp / "a-direct.log").unlink()
        r, log = self.run_sh(self.tmp / "a")
        self.assertEqual(sum(1 for l in log.splitlines() if "--entrypoint du" in l), 0, log)

    def test_crate_downloads_are_shared(self):
        a = self.docker_run_mounts(self.checkout("a"))
        b = self.docker_run_mounts(self.checkout("b"))
        self.assertEqual(a["/root/.cargo/registry"], "obs-rust-linux-validate-cargo-registry")
        self.assertEqual(a["/root/.cargo/registry"], b["/root/.cargo/registry"])
        self.assertEqual(a["/root/.cargo/git"], "obs-rust-linux-validate-cargo-git")

    def test_zccache_is_per_checkout(self):
        # zccache's daemon socket lives in the cache dir and one daemon serves
        # a cache root: a shared volume sent one checkout's compiles to another
        # checkout's container and its /ro (#2028).
        a = self.docker_run_mounts(self.checkout("a"))
        b = self.docker_run_mounts(self.checkout("b"))
        self.assertIn("/zccache", a)
        self.assertRegex(a["/zccache"], r"^obs-rust-linux-validate-zccache-[0-9a-f]+$")
        self.assertNotEqual(a["/zccache"], b["/zccache"])

    def test_zccache_dir_override(self):
        mounts = self.docker_run_mounts(self.checkout("a"), {"OBS_ZCCACHE_DIR": str(self.tmp / "zc")})
        self.assertEqual(mounts["/zccache"], str(self.tmp / "zc"))

    def test_volume_override(self):
        mounts = self.docker_run_mounts(self.checkout("a"), {"OBS_TIER2_VOLUME": "custom-vol"})
        self.assertEqual(mounts["/build"], "custom-vol")

    def test_image_installs_pinned_toolchain(self):
        # rustup no longer installs a rust-toolchain.toml channel on demand, so
        # the image must bake in exactly the pinned one.
        repo = self.checkout("a")
        (repo / "rust-toolchain.toml").write_text('[toolchain]\nchannel = "1.2.3"\n')
        self.docker_run_mounts(repo)
        builds = [line.split() for line in self.last_log.read_text().splitlines() if line.startswith("build ")]
        self.assertEqual(len(builds), 1, self.last_log.read_text())
        self.assertIn("RUST_TOOLCHAIN=1.2.3", builds[0])


class PortedRunTest(unittest.TestCase):
    """run.sh is gone; the port is the module (#962 GEN-005)."""

    def test_shell_script_is_gone(self):
        self.assertFalse((TOOLS / "run.sh").exists(), "run.sh is the ported script")

    def test_no_exception_is_left_for_the_ported_script(self):
        ci_toml = (ROOT / "ci.toml").read_text(encoding="utf-8")
        self.assertNotIn(
            "rust/tools/linux-validate/run.sh",
            ci_toml,
            "the GEN-005 exception for run.sh is still in ci.toml",
        )

    def test_the_local_gate_lane_runs_the_python(self):
        gate = (ROOT / "ci" / "local_gate.py").read_text(encoding="utf-8")
        self.assertIn("rust/tools/linux-validate/run.py", gate)


class StaleBuildTest(unittest.TestCase):
    """validate.sh starts a /build dir over when the Rust toolchain changed.

    The /build volume outlives image rebuilds, and Corrosion caches the old
    toolchain's rustc and cargo paths, so after a toolchain pin change the
    old configure fails with "Could not find toolchain".
    """

    HELPER = TOOLS / "stale_build.py"

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="stale-build-"))
        self.build = self.tmp / "rust-ON"

    def tearDown(self):
        shutil.rmtree(self.tmp)

    def reset(self, rustc_version: str) -> None:
        subprocess.run(
            ["uv", "run", "--script", str(self.HELPER), str(self.build), rustc_version],
            check=True,
        )

    def configured(self, stamp: str | None) -> None:
        self.build.mkdir()
        (self.build / "CMakeCache.txt").write_text("Rust_TOOLCHAIN:STRING=stable\n")
        if stamp is not None:
            (self.tmp / "rust-ON.rustc").write_text(stamp + "\n")

    def test_same_toolchain_keeps_build(self):
        self.configured("rustc 1.98.1")
        self.reset("rustc 1.98.1")
        self.assertTrue((self.build / "CMakeCache.txt").exists())

    def test_changed_toolchain_starts_over(self):
        self.configured("rustc 1.99.0")
        self.reset("rustc 1.98.1")
        self.assertFalse((self.build / "CMakeCache.txt").exists())
        self.assertEqual((self.tmp / "rust-ON.rustc").read_text().strip(), "rustc 1.98.1")

    def test_unstamped_build_starts_over(self):
        # Volumes configured before the stamp existed.
        self.configured(None)
        self.reset("rustc 1.98.1")
        self.assertFalse((self.build / "CMakeCache.txt").exists())
        self.assertEqual((self.tmp / "rust-ON.rustc").read_text().strip(), "rustc 1.98.1")

    def test_fresh_build_is_stamped(self):
        self.reset("rustc 1.98.1")
        self.assertEqual((self.tmp / "rust-ON.rustc").read_text().strip(), "rustc 1.98.1")


if __name__ == "__main__":
    unittest.main()
