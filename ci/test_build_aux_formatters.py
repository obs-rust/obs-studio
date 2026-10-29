#!/usr/bin/env -S uv run --script
"""The dead build-aux formatter symlinks stay deleted (#2708, GEN-005 burn-down).

`build-aux/run-clang-format` and `build-aux/run-gersemi` were upstream OBS's zsh
formatter entry points, symlinks to `build-aux/.run-format.zsh`. Nothing called
them: the lint lane pins and runs `clang-format` and `gersemi` through `uvx` in
`ci/local_gate.py`, so they were dead developer scripts and their two `GEN-005`
exceptions in `ci.toml` were pure baseline. They are deleted, not ported.

The third formatter, `build-aux/run-swift-format`, was the live one: the
`run-swift-format` composite action runs it from `check-format.yaml`. It is now
ported into `ci/run_swift_format.py` as well (#962, GEN-005), so its symlink,
`.run-format.zsh` and the `.functions` log helpers are gone too.

Usage:
    uv run python -m unittest ci/test_build_aux_formatters.py
"""

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BUILD_AUX = ROOT / "build-aux"
DEAD = ("run-clang-format", "run-gersemi", "run-swift-format")


class DeadFormatterScriptsTest(unittest.TestCase):
    def test_dead_formatter_scripts_are_gone(self):
        for name in DEAD:
            with self.subTest(name=name):
                self.assertFalse(
                    (BUILD_AUX / name).exists() or (BUILD_AUX / name).is_symlink(),
                    f"build-aux/{name} is dead (the lint lane runs that formatter "
                    "through uvx); it must stay deleted, not ported",
                )

    def test_no_exception_block_names_a_dead_formatter_script(self):
        ci_toml = (ROOT / "ci.toml").read_text()
        for name in DEAD:
            with self.subTest(name=name):
                self.assertNotIn(
                    f'path = "build-aux/{name}"',
                    ci_toml,
                    f"the GEN-005 exception for build-aux/{name} must go with the file",
                )

    def test_readme_does_not_advertise_a_dead_formatter_script(self):
        readme = (BUILD_AUX / "README.md").read_text()
        for name in DEAD:
            with self.subTest(name=name):
                self.assertNotIn(
                    f"build-aux/{name}",
                    readme,
                    f"build-aux/README.md still documents the deleted {name}",
                )

    def test_the_action_still_runs_the_swift_check(self):
        # The call site moved into the Python port of the action (#2792), and the
        # formatter itself into the module (#962): the action calls the module.
        module = (ROOT / "ci" / "run_swift_format.py").read_text()
        self.assertNotIn("'build-aux' /", module)
        action = (ROOT / ".github/actions/run-swift-format/action.yaml").read_text()
        self.assertIn("ci/run_swift_format.py format", action)
        workflow = (ROOT / ".github/workflows/check-format.yaml").read_text()
        self.assertIn("uses: ./.github/actions/run-swift-format", workflow)


if __name__ == "__main__":
    unittest.main()
