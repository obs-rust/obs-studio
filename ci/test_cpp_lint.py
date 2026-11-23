#!/usr/bin/env -S uv run --script
"""The C++ lint report (#5453): which files and groups it lints, how it
reads clang-tidy and lizard, and that it never fails its caller.

    PYTHONPATH=ci uv run --no-project python -m unittest test_cpp_lint

Offline: git, clang-tidy, lizard and uvx are injected fakes.
"""

from __future__ import annotations

import io
import json
import os
import subprocess
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest import mock

import cpp_lint as c

ROOT = Path(__file__).resolve().parent.parent
BASE = "a" * 40

TREE = {
    "frontend/build/frontend.docks+.hpp.cpp": (
        '/* Unity group */\n#include "frontend/build/frontend-docks.pch.hpp"\n\n'
        '#include "frontend/docks/OBSDock.cpp.hpp"\n#include "frontend/docks/LogDock.cpp.hpp"\n'
    ),
    "frontend/build/frontend.widgets+.hpp.cpp": (
        '#include "frontend/build/frontend-widgets.pch.hpp"\n#include "frontend/widgets/OBSBasic.cpp.hpp"\n'
    ),
    "frontend/build/frontend.widgets-2+.hpp.cpp": (
        '#include "frontend/build/frontend-widgets.pch.hpp"\n#include "frontend/widgets/OBSBasic_Docks.cpp.hpp"\n'
    ),
    "frontend/build/frontend.app+.hpp.cpp": (
        '#include "frontend/build/frontend-app.pch.hpp"\n#include "frontend/OBSApp.cpp.hpp"\n'
    ),
    "frontend/build/frontend-docks.pch.hpp": '#pragma once\n#include "frontend/docks/OBSDock.hpp"\n#include <QDockWidget>\n',
    "frontend/build/frontend-widgets.pch.hpp": (
        '#pragma once\n#include "frontend/widgets/OBSBasic.hpp"\n#include "frontend/docks/OBSDock.hpp"\n'
    ),
    "frontend/build/frontend-app.pch.hpp": '#pragma once\n#include "frontend/OBSApp.hpp"\n',
}
GROUPS = {"docks", "widgets", "widgets-2", "app"}


def groups(*changed: str) -> tuple[set[str], list[str]]:
    return c.groups_for(list(changed), list(TREE), TREE.get)


class GroupsTest(unittest.TestCase):
    def test_an_implementation_file_lints_its_group(self):
        self.assertEqual(groups("frontend/docks/LogDock.cpp.hpp"), ({"docks"}, []))
        self.assertEqual(groups("frontend/widgets/OBSBasic_Docks.cpp.hpp"), ({"widgets-2"}, []))

    def test_a_header_lints_the_groups_whose_precompiled_header_includes_it(self):
        self.assertEqual(groups("frontend/docks/OBSDock.hpp"), ({"docks", "widgets", "widgets-2"}, []))
        self.assertEqual(groups("frontend/OBSApp.hpp"), ({"app"}, []))

    def test_a_changed_group_file_or_precompiled_header_lints_its_groups(self):
        self.assertEqual(groups("frontend/build/frontend.app+.hpp.cpp"), ({"app"}, []))
        self.assertEqual(groups("frontend/build/frontend-widgets.pch.hpp"), ({"widgets", "widgets-2"}, []))

    def test_a_header_no_precompiled_header_includes_lints_every_group(self):
        self.assertEqual(groups("frontend/utility/deep.hpp"), (GROUPS, []))
        self.assertEqual(groups("frontend/utility/obs-thing.h"), (GROUPS, []))

    def test_the_config_and_this_script_lint_every_group(self):
        for path in (c.CONFIG, "ci/cpp_lint.py"):
            with self.subTest(path=path):
                self.assertEqual(groups(path), (GROUPS, []))

    def test_an_implementation_file_in_no_group_is_named_not_linted(self):
        self.assertEqual(groups("frontend/utility/screenshot-render.cpp"),
                         (set(), ["frontend/utility/screenshot-render.cpp"]))

    def test_other_changes_lint_nothing(self):
        self.assertEqual(groups("libobs/obs.c", "frontend/data/locale/en-US.ini", "docs/features.md"), (set(), []))


class FakeGit:
    def __init__(self, answers: dict[tuple[str, ...], str | None]) -> None:
        self.answers = answers
        self.calls: list[tuple[str, ...]] = []

    def __call__(self, *args: str) -> str | None:
        self.calls.append(args)
        return self.answers.get(args)


def diff(base: str) -> tuple[str, ...]:
    return ("diff", "--no-ext-diff", "--no-renames", "--name-only", "-z", base, "HEAD", "--")


class ChangedFilesTest(unittest.TestCase):
    def event(self, payload: object) -> str:
        tmp = tempfile.NamedTemporaryFile("w", suffix=".json", delete=False, encoding="utf-8")
        with tmp:
            json.dump(payload, tmp)
        self.addCleanup(os.unlink, tmp.name)
        return tmp.name

    def test_a_pull_request_diffs_from_its_base(self):
        fake = FakeGit({diff(BASE): "frontend/a.cpp.hpp\0ci/x.py\0"})
        path = self.event({"pull_request": {"base": {"sha": BASE}}})
        self.assertEqual(c.changed_files(fake, None, path), (BASE, ["frontend/a.cpp.hpp", "ci/x.py"]))

    def test_a_merge_group_and_a_push_diff_from_their_base(self):
        for payload in ({"merge_group": {"base_sha": BASE}}, {"before": BASE}):
            with self.subTest(payload=payload):
                fake = FakeGit({diff(BASE): "frontend/a.hpp\0"})
                self.assertEqual(c.changed_files(fake, None, self.event(payload)), (BASE, ["frontend/a.hpp"]))

    def test_an_event_without_a_base_has_no_change(self):
        for payload in ({"before": "0" * 40}, {"schedule": "0 3 * * *"}, {"pull_request": {"base": {"sha": "main"}}}, []):
            with self.subTest(payload=payload):
                self.assertIsNone(c.changed_files(FakeGit({}), None, self.event(payload)))
        self.assertIsNone(c.changed_files(FakeGit({}), None, "/nonexistent/event.json"))

    def test_an_explicit_base_wins_and_a_failed_diff_has_no_change(self):
        fake = FakeGit({diff("HEAD~3"): "frontend/b.cpp.hpp\0"})
        self.assertEqual(c.changed_files(fake, "HEAD~3", self.event({"before": BASE})),
                         ("HEAD~3", ["frontend/b.cpp.hpp"]))
        self.assertIsNone(c.changed_files(FakeGit({}), BASE, None))

    def test_locally_it_takes_the_merge_base_the_working_tree_and_untracked_files(self):
        fake = FakeGit({
            ("merge-base", "HEAD", "origin/master"): BASE + "\n",
            ("diff", "--no-ext-diff", "--no-renames", "--name-only", "-z", BASE, "--"): "frontend/b.hpp\0frontend/a.hpp\0",
            ("ls-files", "--others", "--exclude-standard", "-z"): "frontend/new.cpp.hpp\0frontend/a.hpp\0",
        })
        self.assertEqual(c.changed_files(fake, None, None),
                         (BASE, ["frontend/a.hpp", "frontend/b.hpp", "frontend/new.cpp.hpp"]))
        self.assertIsNone(c.changed_files(FakeGit({}), None, None))


class CompileDatabaseTest(unittest.TestCase):
    def test_only_the_obs_studio_entry_of_a_group_is_kept(self):
        tu = "/w/src/frontend/build/frontend.dock-layout+.hpp.cpp"
        test_entry = {"file": tu, "output": "test/cmocka/CMakeFiles/test_dock_proportions.dir/x.o", "command": "c++ -DTEST"}
        app_entry = {"file": tu, "command": "c++ -o frontend/CMakeFiles/obs-studio.dir/build/x.o -c " + tu}
        other = {"file": "/w/src/libobs/obs.c", "command": "cc -o libobs/CMakeFiles/libobs.dir/obs.c.o"}
        args_entry = {"file": "/w/src/frontend/build/frontend.app+.hpp.cpp",
                      "arguments": ["c++", "-o", "frontend/CMakeFiles/obs-studio.dir/build/a.o"]}
        self.assertEqual(c.obs_studio_entries([test_entry, other, app_entry, args_entry]), [args_entry, app_entry])


TIDY_OUT = """\
/w/src/frontend/utility/DockLayout.cpp.hpp:12:5: warning: use nullptr [modernize-use-nullptr]
/w/src/frontend/utility/DockLayout.cpp.hpp:12:5: warning: use nullptr [modernize-use-nullptr]
/w/src/frontend/widgets/OBSBasic.cpp.hpp:40:1: warning: function 'X' has cognitive complexity of 31 (threshold 15) [readability-function-cognitive-complexity]
/w/src/frontend/widgets/OBSBasic.cpp.hpp:41:3: note: +1, including nesting penalty of 0
/w/build/frontend/obs-studio_autogen/include/ui_OBSBasic.h:9:1: warning: generated [bugprone-macro-parentheses]
/w/src/libobs/obs.h:3:1: warning: not ours [bugprone-reserved-identifier,cert-dcl37-c]
2 warnings generated.
"""


class ParseTest(unittest.TestCase):
    def test_tidy_keeps_the_frontend_warnings_once_relative_to_the_tree(self):
        self.assertEqual(c.parse_tidy(TIDY_OUT, Path("/w/src")), [
            c.Finding("frontend/utility/DockLayout.cpp.hpp", 12, 5, "modernize-use-nullptr", "use nullptr"),
            c.Finding("frontend/widgets/OBSBasic.cpp.hpp", 40, 1, "readability-function-cognitive-complexity",
                      "function 'X' has cognitive complexity of 31 (threshold 15)"),
        ])

    def test_a_finding_key_has_no_position(self):
        finding = c.Finding("frontend/a.hpp", 3, 7, "modernize-use-override", "annotate this")
        moved = c.Finding("frontend/a.hpp", 90, 1, "modernize-use-override", "annotate this")
        self.assertEqual(finding.key(), "frontend/a.hpp: modernize-use-override: annotate this")
        self.assertEqual(finding.key(), moved.key())

    def test_lizard_csv(self):
        text = (
            '11,3,49,2,12,"DockFeatures@3-14@frontend/utility/ExtraDocks.cpp.hpp",'
            '"frontend/utility/ExtraDocks.cpp.hpp","DockFeatures","DockFeatures( bool locked , bool closable)",3,14\n'
            "NLOC,CCN,token\n"
        )
        self.assertEqual(c.parse_lizard(text), [
            c.Function("frontend/utility/ExtraDocks.cpp.hpp", "DockFeatures( bool locked , bool closable)", 3, 11, 3)
        ])

    def test_complexity_rows_are_the_new_and_changed_functions_most_complex_first(self):
        same = c.Function("f.hpp", "A()", 4, 10, 1)
        grown = c.Function("f.hpp", "B()", 12, 70, 20)
        new = c.Function("f.hpp", "C()", 2, 5, 90)
        old_grown = c.Function("f.hpp", "B()", 9, 50, 18)
        self.assertEqual(c.complexity_rows([same, new, grown], [same, old_grown]), [(grown, old_grown), (new, None)])


def completed(argv: list[str], stdout: str = "", code: int = 0) -> subprocess.CompletedProcess[str]:
    return subprocess.CompletedProcess(argv, code, stdout, "")


class LintTest(unittest.TestCase):
    """The whole report with fake tools: it prints and never raises."""

    def setUp(self) -> None:
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.build = Path(tmp.name)
        src = str(c.ROOT)
        self.tu = f"{src}/frontend/build/frontend.dock-layout+.hpp.cpp"
        db = [{"file": self.tu, "directory": str(self.build),
               "command": f"c++ -o frontend/CMakeFiles/obs-studio.dir/build/x.o -c {self.tu}"}]
        (self.build / "compile_commands.json").write_text(json.dumps(db), encoding="utf-8")
        self.calls: list[list[str]] = []
        self.tidy_out = f"{src}/frontend/utility/DockLayout.cpp.hpp:5:1: warning: m [modernize-use-override]\n"

    def runner(self, argv: list[str], timeout: float) -> subprocess.CompletedProcess[str]:
        self.calls.append(argv)
        if argv[0] == "uvx":
            return completed(argv, f"/bin/{argv[2].split('==')[0]}\n")
        if argv[0] == "/bin/clang-tidy":
            return completed(argv, self.tidy_out)
        return completed(argv, "")

    def lint(self, changed: str, **kw) -> list[str]:
        fake = FakeGit({diff(BASE): changed, ("ls-files", "-z", "--", "frontend/build"): "\0".join(
            ["frontend/build/frontend.dock-layout+.hpp.cpp"])})
        with redirect_stdout(io.StringIO()):
            report = c.lint(self.build, BASE, git=fake, runner=kw.pop("runner", self.runner), env={})
        return report.lines

    def test_a_change_in_a_group_runs_clang_tidy_on_it_and_prints_the_findings(self):
        lines = self.lint("frontend/build/frontend.dock-layout+.hpp.cpp\0frontend/utility/DockLayout.cpp.hpp\0")
        tidy = [argv for argv in self.calls if argv[0] == "/bin/clang-tidy"]
        self.assertEqual(len(tidy), 1)
        self.assertEqual(tidy[0][-1], self.tu)
        self.assertIn("--quiet", tidy[0])
        # A GCC PCH and the build's -Werror must not drop the group.
        self.assertIn("--extra-arg=-Wno-error", tidy[0])
        self.assertIn("--extra-arg=-Wno-ignored-gch", tidy[0])
        self.assertIn("  frontend/utility/DockLayout.cpp.hpp", lines)
        self.assertIn("frontend/utility/DockLayout.cpp.hpp:5:1: modernize-use-override: m", lines)
        self.assertIn("clang-tidy: 1 finding(s) in these groups, 1 in the changed files", lines)
        self.assertTrue(any(argv[:3] == ["uvx", "--from", c.CLANG_TIDY] for argv in self.calls))
        self.assertTrue(any(argv[:3] == ["uvx", "--from", c.LIZARD] for argv in self.calls))

    def test_no_frontend_change_runs_no_tool(self):
        lines = self.lint("libobs/obs.c\0")
        self.assertEqual(self.calls, [])
        self.assertIn("no frontend C/C++ change; nothing to lint", lines)

    def test_no_compile_db_is_a_skip_before_any_tool(self):
        (self.build / "compile_commands.json").unlink()
        lines = self.lint("frontend/utility/DockLayout.cpp.hpp\0")
        self.assertEqual(self.calls, [])
        self.assertTrue(any("no compile DB" in line for line in lines))

    def test_failing_tools_are_reported_not_raised(self):
        def broken(argv: list[str], timeout: float) -> subprocess.CompletedProcess[str]:
            if argv[0] == "uvx":
                return completed(argv, "", 2)
            raise AssertionError("no tool may run without uvx")

        lines = self.lint("frontend/utility/DockLayout.cpp.hpp\0", runner=broken)
        self.assertIn(f"clang-tidy: {c.CLANG_TIDY} is not available through uvx; skipped", lines)
        self.assertIn(f"lizard: {c.LIZARD} is not available through uvx; skipped", lines)

        def slow(argv: list[str], timeout: float) -> subprocess.CompletedProcess[str]:
            if argv[0] == "/bin/clang-tidy":
                raise subprocess.TimeoutExpired(argv, timeout)
            return self.runner(argv, timeout)

        lines = self.lint("frontend/utility/DockLayout.cpp.hpp\0", runner=slow)
        self.assertIn("  dock-layout: timed out", lines)

        def erring(argv: list[str], timeout: float) -> subprocess.CompletedProcess[str]:
            if argv[0] == "/bin/clang-tidy":
                return subprocess.CompletedProcess(argv, 1, "", "Error: no checks enabled.\nUSAGE: clang-tidy\n")
            return self.runner(argv, timeout)

        lines = self.lint("frontend/utility/DockLayout.cpp.hpp\0", runner=erring)
        self.assertIn("  dock-layout: exit 1: Error: no checks enabled.", lines)

    def test_main_exits_0_on_any_error_and_writes_the_step_summary(self):
        summary = self.build / "summary.md"
        with redirect_stdout(io.StringIO()) as out, \
                mock.patch.dict(os.environ, {"GITHUB_STEP_SUMMARY": str(summary)}), \
                mock.patch.object(c, "lint", side_effect=RuntimeError("boom")):
            self.assertEqual(c.main(["--build", str(self.build)]), 0)
        self.assertIn("cpp-lint: report failed: RuntimeError('boom')", out.getvalue())
        report = c.Report()
        with redirect_stdout(io.StringIO()):
            report("a finding")
        with redirect_stdout(io.StringIO()), \
                mock.patch.dict(os.environ, {"GITHUB_STEP_SUMMARY": str(summary)}), \
                mock.patch.object(c, "lint", return_value=report):
            self.assertEqual(c.main(["--build", str(self.build)]), 0)
        self.assertIn("a finding", summary.read_text(encoding="utf-8"))
        self.assertIn("report only", summary.read_text(encoding="utf-8"))


class ConfigTest(unittest.TestCase):
    def test_the_config_holds_the_agreed_checks(self):
        text = (ROOT / c.CONFIG).read_text(encoding="utf-8")
        for check in ("-*", "bugprone-*", "-bugprone-easily-swappable-parameters", "-bugprone-narrowing-conversions",
                      "performance-*", "-performance-enum-size", "clang-analyzer-*", "modernize-use-override",
                      "modernize-use-nullptr", "cppcoreguidelines-pro-type-reinterpret-cast",
                      "readability-function-cognitive-complexity"):
            self.assertIn(check, text)
        self.assertNotIn("WarningsAsErrors: '*", text)

    def test_the_tools_are_pinned(self):
        self.assertRegex(c.CLANG_TIDY, r"^clang-tidy==\d+\.\d+\.\d+$")
        self.assertRegex(c.LIZARD, r"^lizard==\d+\.\d+\.\d+$")


if __name__ == "__main__":
    unittest.main()
