#!/usr/bin/env -S uv run --script
"""The Tier 2 Linux build uses the libobs lib-module cache (#3264).

    uv run python -m unittest ci/test_tier2_modcache.py

ci/libobs_modules.py restores unchanged module archives and stores new ones
(#3262). The Tier 2 Linux build wires it in:

- validate.sh, only when OBS_LIBOBS_MODULE_CACHE_DIR is set: `restore` after
  the configure, a reconfigure with -DOBS_LIBOBS_MODULE_CACHE, then `store`
  after a good build. Its main configure always clears the variable, so a
  persistent local tree never keeps the hits of an earlier run.
- build-project.yaml's Tier 2 Linux build job restores .tier2-modcache (the
  PR's own key, then master's prefix) and saves it only on master pushes,
  as it does zccache.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
VALIDATE = ROOT / "rust" / "tools" / "linux-validate" / "validate.sh"
WORKFLOW = ROOT / ".github" / "workflows" / "build-project.yaml"


def tier2_build_job() -> str:
    text = WORKFLOW.read_text(encoding="utf-8")
    start = text.index("name: Rust Tier 2 Linux 🐧 (build)")
    end = text.index("name: Rust Tier 2 Linux 🐧 (tests", start)
    return text[start:end]


class ValidateTest(unittest.TestCase):
    def setUp(self):
        self.text = VALIDATE.read_text(encoding="utf-8")

    def test_main_configure_clears_the_cache_variable(self):
        configure = self.text[self.text.index('cmake -S "$SRC" -B "$build"') :]
        configure = configure[: configure.index(".configure.log")]
        self.assertIn("-DOBS_LIBOBS_MODULE_CACHE=", configure)

    def test_restore_reconfigure_build_store_in_order(self):
        restore = self.text.find("libobs_modules.py\" restore")
        reconfigure = self.text.find('-DOBS_LIBOBS_MODULE_CACHE="$build/libobs-modcache"')
        build = self.text.find('cmake --build "$build" --target "${tier2_targets[@]}"')
        store = self.text.find("libobs_modules.py\" store --prune")
        for name, pos in (("restore", restore), ("reconfigure", reconfigure), ("store", store)):
            self.assertNotEqual(pos, -1, f"validate.sh does not {name} the module cache")
        self.assertLess(restore, reconfigure)
        self.assertLess(reconfigure, build)
        self.assertLess(build, store)

    def test_store_prunes_to_one_set(self):
        # Each master save would otherwise carry every older archive forward.
        import importlib.util
        import shutil
        import tempfile
        spec = importlib.util.spec_from_file_location("libobs_modules", ROOT / "ci" / "libobs_modules.py")
        mods = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mods)
        tmp = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, tmp)
        cache = tmp / "cache"
        cache.mkdir()
        (cache / "old.a").write_bytes(b"old")
        (cache / "kept.a").write_bytes(b"kept")
        build = tmp / "build"
        build.mkdir()
        original = mods.keys
        mods.keys = lambda _build: {"core": "kept"}
        try:
            mods.store(build, cache, prune=True)
        finally:
            mods.keys = original
        self.assertEqual(sorted(p.name for p in cache.iterdir()), ["kept.a"])

    def test_only_when_the_cache_dir_is_set(self):
        guarded = re.findall(r'if \[ -n "\$\{OBS_LIBOBS_MODULE_CACHE_DIR:-\}" \]; then', self.text)
        self.assertGreaterEqual(len(guarded), 2, "restore and store must both be opt-in")


class WorkflowTest(unittest.TestCase):
    def setUp(self):
        self.job = tier2_build_job()

    def test_restores_with_a_master_fallback(self):
        self.assertRegex(self.job, r"path: \$\{\{ github\.workspace \}\}/\.tier2-modcache")
        self.assertIn("restore-keys: libobs-modules-tier2-linux-", self.job)

    def test_saves_only_on_master(self):
        save = self.job[self.job.index("Save Tier 2 Module Cache") :]
        save = save[: save.index("key:")]
        self.assertIn("github.ref == 'refs/heads/master'", save)
        self.assertIn("github.event_name != 'pull_request'", save)

    def test_the_build_step_passes_the_cache_dir(self):
        build = self.job[self.job.index("id: tier2-build") :]
        build = build[: build.index("run: bash rust/tools/linux-validate/validate.sh")]
        self.assertIn("OBS_LIBOBS_MODULE_CACHE_DIR: ${{ github.workspace }}/.tier2-modcache", build)


if __name__ == "__main__":
    unittest.main()
