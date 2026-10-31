#!/usr/bin/env -S uv run --script
"""Unity groups follow BUILD.md.

    uv run python -m unittest ci/test_build_groups.py

- Every implementation file (foo.c.h, foo.cpp.hpp, foo.m.h, foo.mm.hpp) is
  in a group, so it can't drop out of the unity build.
- A file is in one group per build: it may be in several groups only when
  each is for a different host (the per-host no-op groups).
- A group file holds comments, its precompiled header (first, a
  <name>.pch.h/.pch.hpp next to it), then includes of existing implementation
  files of its own language, in alphabetical order.
- Every implementation file in a group includes exactly one header: the
  group's precompiled header. Anything else it needs is in that header, so the
  PCH is the whole of what each file depends on. Exceptions, both after the
  code starts: a host file including the no-op capability headers it builds
  (obs-host/noop/*.hpp, #1051), and an X-macro table (a header without
  #pragma once or an include guard) included as data.
- Every macro an implementation file #defines is #undef'd later in the same
  file (BUILD.md rule 5), so it can't leak into the next file of its group.
- libobs's groups are partitioned into lib-modules (#3256, #3258): each
  libobs/build/modules.d/<module>.json names its groups by fnmatch pattern
  and the modules it depends on. Every libobs group matches exactly one
  module, every pattern matches a group, and the dependencies name existing
  modules without a cycle.
- Every quoted include of a repo file in a group, a precompiled header, an
  implementation file or a forwarding file is a repo-root path
  ("plugins/obs-ffmpeg/build/obs-ffmpeg.pch.h"), never relative to the file.
"""

from __future__ import annotations

import fnmatch
import json
import re
import subprocess
import unittest
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
IMPL = {".c.h": "c", ".cpp.hpp": "cpp", ".m.h": "m", ".mm.hpp": "mm"}
GROUP = {"+.h.c": "c", "+.hpp.cpp": "cpp", "+.h.m": "m", "+.hpp.mm": "mm"}
PCH = (".pch.h", ".pch.hpp")
HOSTS = ("windows", "macos", "linux")
INCLUDE = re.compile(r'^\s*#\s*(?:include|import)\s*([<"])([^">]+)[">]')
DIRECTIVE = re.compile(r"^\s*#")


def tracked() -> list[str]:
    out = subprocess.run(["git", "ls-files", "-z"], cwd=ROOT, check=True, capture_output=True).stdout
    return [p for p in out.decode().split("\0") if p and (ROOT / p).is_file()]


def kind(path: str, table: dict[str, str]) -> str | None:
    return next((lang for suffix, lang in table.items() if path.endswith(suffix)), None)


def host(group: str) -> str | None:
    name = Path(group).name
    return next((h for h in HOSTS if re.search(rf"\b{h}\b", name)), None)


def code_lines(text: str):
    """(line number, line, code started) for each line outside comments."""
    comment = False
    started = False
    lines = text.splitlines()
    i = 0
    while i < len(lines):
        line = lines[i]
        stripped = line.strip()
        number = i + 1
        i += 1
        if comment:
            comment = "*/" not in stripped
            continue
        if stripped.startswith("/*"):
            comment = "*/" not in stripped
            if comment or stripped.endswith("*/"):
                continue
        if not stripped or stripped.startswith("//"):
            continue
        if not DIRECTIVE.match(line):
            started = True
        yield number, line, started
        while line.rstrip().endswith("\\") and i < len(lines):
            line = lines[i]
            i += 1


def guarded(header: Path) -> bool:
    text = header.read_text(errors="replace")
    return "#pragma once" in text or re.search(r"^\s*#\s*ifndef\s+(\w+)\s*\n\s*#\s*define\s+\1[ \t]*$", text, re.M) is not None


def resolve(target: str, *bases: Path) -> Path | None:
    for base in bases:
        path = (base / target).resolve()
        if path.is_file():
            return path
    return None


def parse(group: str) -> tuple[Path | None, list[str], list[str]]:
    """(precompiled header, members as repo paths, errors) of one group file."""
    pch, members, errors = None, [], []
    base = (ROOT / group).parent
    for number, line, started in code_lines((ROOT / group).read_text()):
        match = INCLUDE.match(line)
        if started or not match or match.group(1) != '"':
            errors.append(f"{group}:{number}: only comments and #include \"...\" lines belong in a group")
            continue
        target = match.group(2)
        path = resolve(target, ROOT)
        if path is None:
            errors.append(f"{group}:{number}: {target} does not exist")
        elif kind(target, IMPL):
            members.append(path.relative_to(ROOT).as_posix())
        elif target.endswith(PCH) and path.parent == base.resolve():
            if members or pch:
                errors.append(f"{group}:{number}: the precompiled header comes first, once")
            pch = path
        else:
            errors.append(f"{group}:{number}: {target} is neither an implementation file nor this directory's .pch.h")
    if pch is None:
        errors.append(f"{group}: no precompiled header (BUILD.md: every group has one)")
    return pch, members, errors


def member_errors(member: str, pch: Path, component: Path) -> list[str]:
    errors, headers = [], []
    path = ROOT / member
    for number, line, started in code_lines(path.read_text()):
        match = INCLUDE.match(line)
        if not match:
            continue
        target = match.group(2)
        found = resolve(target, ROOT, path.parent, component) if match.group(1) == '"' else None
        if started and found is not None and (found.parent.name == "noop" and found.suffix == ".hpp"):
            continue  # a no-op capability the host builds
        if started and found is not None and not guarded(found):
            continue  # an X-macro table, included as data
        headers.append((number, target, found))
    if len(headers) != 1 or headers[0][2] != pch:
        listed = ", ".join(f"{t} (line {n})" for n, t, _ in headers) or "none"
        errors.append(
            f"{member}: includes {listed}; it must include exactly its group's precompiled header "
            f"{pch.relative_to(component).as_posix()} and move the rest into it (BUILD.md)"
        )
    return errors


class BuildGroupsTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        files = tracked()
        cls.impls = [p for p in files if kind(p, IMPL)]
        cls.groups = [p for p in files if kind(p, GROUP)]
        cls.parsed = {g: parse(g) for g in cls.groups}

    def test_group_files_are_well_formed(self):
        for group, (_, members, errors) in self.parsed.items():
            with self.subTest(group=group):
                self.assertEqual(errors, [])
                self.assertTrue(members, "a group includes at least one implementation file")
                language = kind(group, GROUP)
                self.assertEqual({kind(m, IMPL) for m in members}, {language}, "one language per group")
                self.assertEqual(members, sorted(members), "members in alphabetical order")

    def test_members_include_only_the_precompiled_header(self):
        for group, (pch, members, _) in self.parsed.items():
            if pch is None:
                continue
            component = (ROOT / group).parent.parent.resolve()
            for member in members:
                with self.subTest(file=member):
                    self.assertEqual(member_errors(member, pch, component), [])

    def test_file_local_macros_are_undefined_at_the_end(self):
        for _, members, _ in self.parsed.values():
            for member in members:
                text = (ROOT / member).read_text(errors="replace")
                undefined = set()
                leaked = []
                lines = [l for l in text.splitlines() if l.strip()]
                code_after = False
                for i in range(len(lines) - 1, -1, -1):
                    line = lines[i]
                    u = re.match(r"\s*#\s*undef\s+(\w+)", line)
                    if u:
                        undefined.add(u.group(1))
                        continue
                    d = re.match(r"\s*#\s*define\s+(\w+)", line)
                    if d and d.group(1) not in undefined:
                        # Restoring a header's macro at the very end (#undef X,
                        # then #define X as the header has it) is not a leak.
                        restore = not code_after and i > 0 and re.match(rf"\s*#\s*undef\s+{d.group(1)}\b", lines[i - 1])
                        if not restore:
                            leaked.append(d.group(1))
                    if not re.match(r"\s*(#|/\*|\*|//)", line):
                        code_after = True
                with self.subTest(file=member):
                    self.assertEqual(sorted(set(leaked)), [], "#undef these at the end of the file (BUILD.md rule 5)")

    def test_includes_are_repo_root_paths(self):
        files = [p for p in tracked() if kind(p, IMPL) or kind(p, GROUP) or p.endswith(PCH)]
        files += [
            p
            for p in tracked()
            if p.endswith((".c", ".cpp", ".m", ".mm")) and "Forwarding file (BUILD.md)" in (ROOT / p).read_text(errors="replace")
        ]
        for path in files:
            relative = []
            for number, line in enumerate((ROOT / path).read_text(errors="replace").splitlines(), 1):
                match = INCLUDE.match(line)
                if not match or match.group(1) != '"':
                    continue
                target = match.group(2)
                local = ((ROOT / path).parent / target).resolve()
                if local.is_file() and local.is_relative_to(ROOT) and local.relative_to(ROOT).as_posix() != target:
                    relative.append(f"{path}:{number}: {target} -> {local.relative_to(ROOT).as_posix()}")
            with self.subTest(file=path):
                self.assertEqual(relative, [], "write repo-root paths (BUILD.md)")

    def test_every_implementation_file_is_in_a_group(self):
        grouped = {m for _, members, _ in self.parsed.values() for m in members}
        self.assertEqual(sorted(set(self.impls) - grouped), [], "add these to a group (BUILD.md rule 4)")

    def test_one_group_per_build(self):
        homes = defaultdict(list)
        for group, (_, members, _) in self.parsed.items():
            for member in members:
                homes[member].append(group)
        for member, groups in sorted(homes.items()):
            if len(groups) == 1:
                continue
            with self.subTest(file=member):
                hosts = [host(g) for g in groups]
                self.assertNotIn(None, hosts, f"in several groups that aren't per host: {groups}")
                self.assertEqual(len(hosts), len(set(hosts)), f"twice for one host: {groups}")


MODULES = ROOT / "libobs" / "build" / "modules.d"


def libobs_groups() -> list[str]:
    """libobs's unity group names: libobs/build/libobs.<name>+.h.c (or .m)."""
    names = []
    for path in sorted((ROOT / "libobs" / "build").iterdir()):
        if kind(path.name, GROUP) and path.name.startswith("libobs."):
            names.append(path.name[len("libobs.") :].split("+", 1)[0])
    return names


def load_modules() -> dict[str, dict]:
    return {path.stem: json.loads(path.read_text(encoding="utf-8")) for path in sorted(MODULES.glob("*.json"))}


class LibobsModulesTest(unittest.TestCase):
    """The lib-module manifest (#3258): the unit the module cache keys on."""

    def setUp(self):
        self.modules = load_modules()

    def test_manifest_exists(self):
        self.assertTrue(self.modules, f"{MODULES.relative_to(ROOT)} has no module files")

    def test_every_group_is_in_exactly_one_module(self):
        errors = []
        for group in libobs_groups():
            owners = [name for name, module in self.modules.items() if any(fnmatch.fnmatchcase(group, pattern) for pattern in module["groups"])]
            if len(owners) != 1:
                errors.append(f"libobs.{group}: in {owners or 'no module'}")
        self.assertEqual(errors, [], "each libobs group belongs to exactly one module")

    def test_every_pattern_matches_a_group(self):
        groups = libobs_groups()
        stale = [
            f"{name}: {pattern}"
            for name, module in self.modules.items()
            for pattern in module["groups"]
            if not any(fnmatch.fnmatchcase(group, pattern) for group in groups)
        ]
        self.assertEqual(stale, [], "a module pattern that matches no group is stale")

    def test_dependencies_exist_and_have_no_cycle(self):
        for name, module in self.modules.items():
            for dependency in module.get("depends", []):
                self.assertIn(dependency, self.modules, f"{name} depends on unknown module {dependency}")
        state: dict[str, str] = {}

        def visit(name: str, path: list[str]) -> None:
            if state.get(name) == "done":
                return
            self.assertNotEqual(state.get(name), "visiting", f"module cycle: {' -> '.join(path + [name])}")
            state[name] = "visiting"
            for dependency in self.modules[name].get("depends", []):
                visit(dependency, path + [name])
            state[name] = "done"

        for name in self.modules:
            visit(name, [])


if __name__ == "__main__":
    unittest.main()
