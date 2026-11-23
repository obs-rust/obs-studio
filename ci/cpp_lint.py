#!/usr/bin/env -S uv run --script
"""C++ lint report for the Qt frontend (#5453): clang-tidy on the unity
groups a change touches, and a lizard complexity table of the changed files.

    uv run --no-project ci/cpp_lint.py --build BUILD_DIR [--base REV]

BUILD_DIR is a configured frontend build with compile_commands.json (the Qt
lane's build tree; ci/linux_qt_frontend.py exports it). The checks are in
frontend/.clang-tidy. clang-tidy and lizard come pinned through uvx, like
clang-format.

Report only: it prints its findings, and writes them to
$GITHUB_STEP_SUMMARY when that is set. It exits 0 whatever the tools find
and however they fail (only a wrong command line exits 2). The baseline and
the gate on new findings follow in a later PR (#5453 step 3).

Which files changed: --base REV, else the GitHub event in $GITHUB_EVENT_PATH
(the PR's base, the merge group's base, the push's `before`), else the
merge-base with origin/master plus the working tree and untracked files.

Which groups: the frontend compiles in unity groups (BUILD.md); clang-tidy
runs on frontend/build/frontend.<group>+.hpp.cpp. A changed implementation
file runs its group; a changed header runs the groups whose precompiled
header includes it; a header that no precompiled header includes, or a
change to frontend/.clang-tidy or this script, runs every group.
"""

from __future__ import annotations

import argparse
import csv
import io
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
from collections import Counter
from collections.abc import Callable, Iterable
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CLANG_TIDY = "clang-tidy==22.1.8"
LIZARD = "lizard==1.26.1"
CONFIG = "frontend/.clang-tidy"
# A change to these lints every group.
EVERYTHING = frozenset({CONFIG, "ci/cpp_lint.py"})
GROUP_TU = re.compile(r"^frontend/build/frontend\.(?P<group>[^/]+)\+\.hpp\.cpp$")
PCH = re.compile(r"^frontend/build/[^/]+\.pch\.hpp$")
INCLUDE = re.compile(r'^[ \t]*#[ \t]*include[ \t]+"(frontend/[^"]+)"', re.MULTILINE)
SOURCE = (".c", ".cpp", ".cxx", ".h", ".hpp", ".mm")
HEADER = (".h", ".hpp")
# The obs-studio target's object of a group TU; the test targets compile
# some groups too, with other defines, and clang-tidy takes the first entry.
OBS_STUDIO = re.compile(r"(?:^|[\s/])frontend/CMakeFiles/obs-studio\.dir/")
WARNING = re.compile(
    r"^(?P<path>[^\s:][^:]*):(?P<line>\d+):(?P<col>\d+): warning: (?P<message>.*) \[(?P<check>[\w.,-]+)\]$"
)
NULL_SHA = "0" * 40
# Seconds for one group, and for all of clang-tidy: what is not started by
# then is listed as not run.
GROUP_TIMEOUT = 300
BUDGET = 480
# lizard's own warning limits; the table marks functions over them.
CCN_LIMIT = 10
NLOC_LIMIT = 60
TABLE_ROWS = 40
LISTED = 20

Git = Callable[..., "str | None"]
Runner = Callable[[list[str], float], "subprocess.CompletedProcess[str]"]


@dataclass(frozen=True, order=True)
class Finding:
    path: str
    line: int
    col: int
    check: str
    message: str

    def key(self) -> str:
        """The finding without its position, so moved code is not new."""
        return f"{self.path}: {self.check}: {self.message}"


@dataclass(frozen=True)
class Function:
    path: str
    name: str
    ccn: int
    nloc: int
    line: int


def git(*args: str) -> str | None:
    result = subprocess.run(["git", *args], cwd=ROOT, capture_output=True, text=True)
    return result.stdout if result.returncode == 0 else None


def run(argv: list[str], timeout: float) -> subprocess.CompletedProcess[str]:
    return subprocess.run(argv, cwd=ROOT, capture_output=True, text=True, timeout=timeout)


def nul_split(text: str | None) -> list[str]:
    return [path for path in (text or "").split("\0") if path]


def event_base(event: object) -> str | None:
    """The commit a GitHub event's change starts from, or None."""
    if not isinstance(event, dict):
        return None
    if isinstance(event.get("pull_request"), dict):
        base = event["pull_request"].get("base")
        sha = base.get("sha") if isinstance(base, dict) else None
    elif isinstance(event.get("merge_group"), dict):
        sha = event["merge_group"].get("base_sha")
    else:
        sha = event.get("before")
    if not isinstance(sha, str) or not re.fullmatch(r"[0-9a-f]{40}", sha) or sha == NULL_SHA:
        return None
    return sha


def changed_files(git: Git, base: str | None, event_path: str | None) -> tuple[str, list[str]] | None:
    """(base, changed paths), or None when there is no base to diff against."""
    if base is None and event_path:
        try:
            event = json.loads(Path(event_path).read_text(encoding="utf-8"))
        except (OSError, ValueError):
            return None
        base = event_base(event)
        if base is None:
            return None
    if base is not None:
        out = git("diff", "--no-ext-diff", "--no-renames", "--name-only", "-z", base, "HEAD", "--")
        return None if out is None else (base, nul_split(out))
    merge_base = (git("merge-base", "HEAD", "origin/master") or "").strip()
    if not merge_base:
        return None
    paths = nul_split(git("diff", "--no-ext-diff", "--no-renames", "--name-only", "-z", merge_base, "--"))
    paths += nul_split(git("ls-files", "--others", "--exclude-standard", "-z"))
    return merge_base, sorted(set(paths))


def group_index(tree: Iterable[str], read: Callable[[str], str | None]) -> tuple[set[str], dict[str, set[str]]]:
    """(every group, file -> groups): a group TU's own includes (its files and
    its precompiled header), and the headers each precompiled header includes."""
    groups: set[str] = set()
    owners: dict[str, set[str]] = {}
    pch_users: dict[str, set[str]] = {}
    for path in tree:
        match = GROUP_TU.match(path)
        if not match:
            continue
        group = match["group"]
        groups.add(group)
        owners.setdefault(path, set()).add(group)
        for include in INCLUDE.findall(read(path) or ""):
            owners.setdefault(include, set()).add(group)
            if PCH.match(include):
                pch_users.setdefault(include, set()).add(group)
    for pch, users in pch_users.items():
        for include in INCLUDE.findall(read(pch) or ""):
            owners.setdefault(include, set()).update(users)
    return groups, owners


def groups_for(changed: Iterable[str], tree: Iterable[str], read: Callable[[str], str | None]) -> tuple[set[str], list[str]]:
    """(groups to lint, changed frontend sources in no group)."""
    changed = list(changed)
    groups, owners = group_index(tree, read)
    if EVERYTHING & set(changed):
        return set(groups), []
    selected: set[str] = set()
    unowned: list[str] = []
    for path in changed:
        if not path.startswith("frontend/") or not path.endswith(SOURCE):
            continue
        if path in owners:
            selected |= owners[path]
        elif path.endswith(HEADER) and not path.endswith(".cpp.hpp"):
            # Reached only through another header: every group may see it.
            return set(groups), []
        else:
            unowned.append(path)
    return selected, unowned


def obs_studio_entries(database: list[dict]) -> list[dict]:
    """The group TUs' compile commands of the obs-studio target, one each."""
    entries: dict[str, dict] = {}
    for entry in database:
        file = str(entry.get("file", ""))
        if not re.search(r"/frontend/build/frontend\.[^/]+\+\.hpp\.cpp$", file) or file in entries:
            continue
        command = entry.get("command") or " ".join(entry.get("arguments") or [])
        if OBS_STUDIO.search(str(entry.get("output", "")) + " " + command):
            entries[file] = entry
    return [entries[file] for file in sorted(entries)]


def parse_tidy(text: str, src: Path) -> list[Finding]:
    """The warnings in the source tree's frontend/, relative to it."""
    prefix = str(src).rstrip("/") + "/frontend/"
    findings = set()
    for line in text.splitlines():
        match = WARNING.match(line)
        if not match or not match["path"].startswith(prefix):
            continue
        findings.add(
            Finding(match["path"][len(str(src).rstrip("/")) + 1 :], int(match["line"]), int(match["col"]),
                    match["check"], match["message"])
        )
    return sorted(findings)


def parse_lizard(text: str) -> list[Function]:
    """lizard --csv: NLOC, CCN, tokens, parameters, length, location, file,
    name, long name, start, end."""
    functions = []
    for row in csv.reader(io.StringIO(text)):
        if len(row) < 11 or not row[0].isdigit():
            continue
        functions.append(Function(row[6], row[8], int(row[1]), int(row[0]), int(row[9])))
    return functions


def complexity_rows(head: list[Function], base: list[Function]) -> list[tuple[Function, Function | None]]:
    """The new and the changed functions, most complex first."""
    before = {(function.path, function.name): function for function in base}
    rows = []
    for function in head:
        old = before.get((function.path, function.name))
        if old is None or (old.ccn, old.nloc) != (function.ccn, function.nloc):
            rows.append((function, old))
    return sorted(rows, key=lambda row: (-row[0].ccn, -row[0].nloc, row[0].path, row[0].line))


def tidy_command(exe: str, database: Path, src: Path, tu: str) -> list[str]:
    return [
        exe, "-p", str(database), "--quiet",
        f"--header-filter=^{re.escape(str(src).rstrip('/'))}/frontend/",
        "--extra-arg=-Wno-unknown-warning-option",
        "--extra-arg=-Wno-unused-command-line-argument",
        # The build's -Werror would make clang's warnings errors, and a GCC
        # precompiled header (cmake_pch.hxx.gch) is one clang cannot read:
        # either drops the whole group.
        "--extra-arg=-Wno-error",
        "--extra-arg=-Wno-ignored-gch",
        tu,
    ]


def uvx_tool(runner: Runner, spec: str, binary: str) -> str | None:
    code = f"import shutil; print(shutil.which({binary!r}) or '')"
    try:
        out = runner(["uvx", "--from", spec, "python", "-c", code], 300)
    except (OSError, subprocess.SubprocessError):
        return None
    path = out.stdout.strip() if out.returncode == 0 else ""
    return path or None


class Report:
    def __init__(self) -> None:
        self.lines: list[str] = []

    def __call__(self, line: str = "") -> None:
        print(line, flush=True)
        self.lines.append(line)

    def summary(self) -> str:
        body = "\n".join(self.lines)
        return f"### C++ lint report (report only, #5453)\n\n```text\n{body}\n```\n"


def group_of(entry: dict) -> str:
    match = GROUP_TU.match(f"frontend/build/{Path(entry['file']).name}")
    return match["group"] if match else ""


def group_entries(report: Report, build: Path, groups: set[str]) -> list[dict]:
    """The obs-studio compile commands of `groups`; a group the build left
    out (a disabled feature) is named."""
    try:
        database = json.loads((build / "compile_commands.json").read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        report(f"clang-tidy: no compile DB ({error})")
        return []
    entries = [entry for entry in obs_studio_entries(database) if group_of(entry) in groups]
    if missing := sorted(groups - {group_of(entry) for entry in entries}):
        report(f"clang-tidy: not in this build's compile DB (disabled here): {', '.join(missing)}")
    return entries


def failure(result: subprocess.CompletedProcess[str]) -> str:
    """The exit code and the first error line, so a dropped group says why."""
    lines = (result.stderr or "").splitlines() + (result.stdout or "").splitlines()
    error = next((line.strip() for line in lines if "error" in line.lower()), "")
    return f"exit {result.returncode}" + (f": {error[:200]}" if error else "")


def run_groups(runner: Runner, exe: str, src: Path, entries: list[dict], jobs: int,
               budget: float) -> tuple[list[tuple[str, str]], list[Finding]]:
    """((group, problem) per group, findings): one clang-tidy per group TU,
    `jobs` at a time; a group not started within `budget` seconds is not run."""
    deadline = time.monotonic() + budget
    findings: list[Finding] = []
    with tempfile.TemporaryDirectory(prefix="cpp-lint-") as tmp:
        Path(tmp, "compile_commands.json").write_text(json.dumps(entries), encoding="utf-8")

        def one(entry: dict) -> tuple[str, str]:
            left = deadline - time.monotonic()
            if left <= 0:
                return group_of(entry), "not run: over the time budget"
            try:
                result = runner(tidy_command(exe, Path(tmp), src, entry["file"]), min(GROUP_TIMEOUT, left))
            except subprocess.TimeoutExpired:
                return group_of(entry), "timed out"
            except (OSError, subprocess.SubprocessError) as error:
                return group_of(entry), f"failed: {error}"
            findings.extend(parse_tidy(result.stdout, src))
            return group_of(entry), "" if result.returncode == 0 else failure(result)

        with ThreadPoolExecutor(max_workers=max(1, jobs)) as pool:
            outcomes = list(pool.map(one, entries))
    return outcomes, sorted(set(findings))


def tidy(report: Report, runner: Runner, build: Path, src: Path, groups: set[str], changed: set[str],
         jobs: int, budget: float = BUDGET) -> None:
    entries = group_entries(report, build, groups)
    if not entries:
        report("clang-tidy: no group to run")
        return
    exe = uvx_tool(runner, CLANG_TIDY, "clang-tidy")
    if exe is None:
        report(f"clang-tidy: {CLANG_TIDY} is not available through uvx; skipped")
        return
    start = time.monotonic()
    outcomes, findings = run_groups(runner, exe, src, entries, jobs, budget)
    names = [name for name, _ in outcomes]
    report(f"clang-tidy {CLANG_TIDY.split('==')[1]}: {len(names)} group(s) in {int(time.monotonic() - start)} s: "
           f"{', '.join(names)}")
    for name, problem in outcomes:
        if problem:
            report(f"  {name}: {problem}")
    mine = [finding for finding in findings if finding.path in changed]
    for finding in mine:
        report(f"{finding.path}:{finding.line}:{finding.col}: {finding.check}: {finding.message}")
    report(f"clang-tidy: {len(findings)} finding(s) in these groups, {len(mine)} in the changed files")
    for check, count in Counter(finding.check for finding in findings).most_common():
        report(f"  {count:5} {check}")


def complexity(report: Report, runner: Runner, git: Git, base: str, files: list[str]) -> None:
    if not files:
        return
    exe = uvx_tool(runner, LIZARD, "lizard")
    if exe is None:
        report(f"lizard: {LIZARD} is not available through uvx; skipped")
        return
    present = [path for path in files if (ROOT / path).is_file()]
    try:
        head = parse_lizard(runner([exe, "--csv", *present], 300).stdout) if present else []
        with tempfile.TemporaryDirectory(prefix="cpp-lint-base-") as tmp:
            old = []
            for path in files:
                text = git("show", f"{base}:{path}")
                if text is not None:
                    target = Path(tmp, path)
                    target.parent.mkdir(parents=True, exist_ok=True)
                    target.write_text(text, encoding="utf-8")
                    old.append(path)
            before = []
            if old:
                prefix = str(Path(tmp).resolve()) + "/"
                out = runner([exe, "--csv", *(prefix + path for path in old)], 300).stdout
                before = [Function(f.path.removeprefix(prefix), f.name, f.ccn, f.nloc, f.line)
                          for f in parse_lizard(out)]
    except (OSError, subprocess.SubprocessError) as error:
        report(f"lizard: failed: {error}")
        return
    rows = complexity_rows(head, before)
    report(f"lizard {LIZARD.split('==')[1]}: {len(rows)} new or changed function(s) in the changed files "
           f"(CCN, NLOC; base {short(base)}; * over {CCN_LIMIT} CCN or {NLOC_LIMIT} NLOC)")
    for function, old in rows[:TABLE_ROWS]:
        was = "new" if old is None else f"was {old.ccn:3} {old.nloc:4}"
        mark = "*" if function.ccn > CCN_LIMIT or function.nloc > NLOC_LIMIT else " "
        report(f" {mark} {function.ccn:3} {function.nloc:4}  {was:13} {function.path}:{function.line} {function.name}")
    if len(rows) > TABLE_ROWS:
        report(f"   ... {len(rows) - TABLE_ROWS} more")


def short(rev: str) -> str:
    return rev[:12] if re.fullmatch(r"[0-9a-f]{40}", rev) else rev


def tracked(git: Git) -> list[str]:
    return nul_split(git("ls-files", "-z", "--", "frontend/build"))


def read(path: str) -> str | None:
    try:
        return (ROOT / path).read_text(encoding="utf-8")
    except OSError:
        return None


def lint(build: Path, base: str | None, *, git: Git = git, runner: Runner = run,
         env: dict[str, str] | None = None, which: Callable[[str], str | None] = shutil.which,
         install_clazy: bool = False) -> Report:
    env = dict(os.environ) if env is None else env
    report = Report()
    report(f"== C++ lint report (report only, #5453): {CLANG_TIDY}, {LIZARD}")
    if not (build / "compile_commands.json").is_file():
        report(f"no compile DB in {build}; skipped")
        return report
    change = changed_files(git, base, env.get("GITHUB_EVENT_PATH"))
    if change is None:
        report("no base to diff against (not a PR, merge group or push); skipped")
        return report
    base_sha, paths = change
    frontend = [path for path in paths if path.startswith("frontend/") and path.endswith(SOURCE)]
    groups, unowned = groups_for(paths, tracked(git), read)
    report(f"changed frontend C/C++ files: {len(frontend)} (base {short(base_sha)})")
    for path in frontend[:LISTED]:
        report(f"  {path}")
    if len(frontend) > LISTED:
        report(f"  ... {len(frontend) - LISTED} more")
    for path in unowned:
        report(f"  in no unity group, not linted: {path}")
    if groups:
        jobs = int(env.get("CPP_LINT_JOBS") or os.cpu_count() or 2)
        tidy(report, runner, build, ROOT, groups, set(frontend), jobs)
    elif not frontend:
        report("no frontend C/C++ change; nothing to lint")
    complexity(report, runner, git, base_sha, frontend)
    return report


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--build", type=Path, required=True, help="build dir with compile_commands.json")
    parser.add_argument("--base", help="commit to diff against (default: the GitHub event, or origin/master)")
    parser.add_argument("--install-clazy", action="store_true", help="#5453 step 2")
    args = parser.parse_args(argv)
    try:
        report = lint(args.build.resolve(), args.base)
    except Exception as error:  # report only: never fail the caller
        print(f"cpp-lint: report failed: {error!r}", flush=True)
        return 0
    if summary := os.environ.get("GITHUB_STEP_SUMMARY"):
        try:
            with open(summary, "a", encoding="utf-8") as out:
                out.write(report.summary())
        except OSError as error:
            print(f"cpp-lint: cannot write the step summary: {error}", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
