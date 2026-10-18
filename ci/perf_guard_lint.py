#!/usr/bin/env -S uv run --script
"""Perf guards: code shapes that a measured performance fix depends on (#1303).

    uv run ci/perf_guard_lint.py

Some fixes are fast only because of how the compiler translates one shape of
code, not because the algorithm changed. #1303 is the first: two calls with a
constant start make LLVM keep a branch; the "simpler" `1 + extension_flag`
makes it emit a select, and the AV1 keyframe walk is 1.6x the C code again on
x86_64. Nothing fails when that comes back, so ci/perf-guards.toml lists such
shapes and this lint fails when a guarded function loses one. The message
gives the reason and the bench to run, so whoever changes the code (a person
or an LLM) measures before the guard is moved.

Checks per guard, on the Rust function found by name in the file:

- `attributes`: each must be among the attributes right above the `fn`;
- `require`: each text must be in the body;
- `forbid`: no text may be in the body;
- a `PERF GUARD #<issue>` comment in the lines above the `fn` or in the body.

Bodies are compared with comments removed (a shape in a comment does not
count) and with all whitespace removed (rustfmt changes do not matter). A
guard whose function is gone fails too: move or drop the guard together with
the code, after measuring.
"""

from __future__ import annotations

import re
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
REGISTRY = "ci/perf-guards.toml"

REQUIRED_KEYS = {"issue", "file", "function", "reason", "bench"}
LIST_KEYS = ("attributes", "require", "forbid")


def load_guards(path: Path) -> list[dict]:
    """The `[[guard]]` entries of `path`, checked and with empty lists filled in."""
    data = tomllib.loads(path.read_text())
    guards = data.get("guard", [])
    if set(data) - {"guard"}:
        raise ValueError(f"{path}: unknown top-level keys {sorted(set(data) - {'guard'})}")
    out = []
    for n, guard in enumerate(guards, 1):
        where = f"{path}: guard {n}"
        unknown = set(guard) - REQUIRED_KEYS - set(LIST_KEYS)
        missing = REQUIRED_KEYS - set(guard)
        if unknown or missing:
            raise ValueError(f"{where}: unknown keys {sorted(unknown)}, missing keys {sorted(missing)}")
        if not isinstance(guard["issue"], int):
            raise ValueError(f"{where}: issue must be an issue number")
        guard = {**{key: [] for key in LIST_KEYS}, **guard}
        if not any(guard[key] for key in LIST_KEYS):
            raise ValueError(f"{where}: a guard needs attributes, require or forbid")
        out.append(guard)
    return out


def strip_comments(text: str) -> str:
    """`text` without `//` and `/* */` comments; string and char literals are kept."""
    out = []
    i, n = 0, len(text)
    while i < n:
        if text.startswith("//", i):
            while i < n and text[i] != "\n":
                i += 1
        elif text.startswith("/*", i):
            end = text.find("*/", i + 2)
            end = n if end < 0 else end + 2
            # Keep the line breaks: find_function maps lines back to the source.
            out.append(" " + "\n" * text.count("\n", i, end))
            i = end
        elif text[i] == '"':
            end = i + 1
            while end < n and text[end] != '"':
                end += 2 if text[end] == "\\" else 1
            out.append(text[i : end + 1])
            i = end + 1
        else:
            out.append(text[i])
            i += 1
    return "".join(out)


def squeeze(text: str) -> str:
    return re.sub(r"\s+", "", text)


def without_literals(text: str) -> str:
    """`text` (comments already removed) with string and brace char literals emptied."""
    text = re.sub(r'"(?:\\.|[^"\\])*"', lambda m: '""' + "\n" * m.group().count("\n"), text)
    return re.sub(r"'[{}]'", "''", text)


def find_function(text: str, name: str) -> tuple[list[str], str] | None:
    """The lines right above `fn name` (attributes and comments) and its body."""
    lines = text.splitlines()
    pattern = re.compile(rf"^\s*(pub(\([^)]*\))?\s+)?(const\s+|unsafe\s+|extern\s+\"C\"\s+)*fn\s+{re.escape(name)}\b")
    for at, line in enumerate(lines):
        if not pattern.match(line):
            continue
        above = []
        for prev in reversed(lines[:at]):
            stripped = prev.strip()
            if stripped.startswith(("#[", "//")):
                above.insert(0, stripped)
            else:
                break
        rest = "\n".join(lines[at:])
        code = without_literals(strip_comments(rest))
        start = code.find("{")
        if start < 0:
            return above, ""
        depth = 0
        for end, char in enumerate(code[start:], start):
            depth += {"{": 1, "}": -1}.get(char, 0)
            if depth == 0:
                break
        # Map the brace span back onto the source: both texts keep line breaks.
        body_lines = code[: end + 1].count("\n") + 1
        return above, "\n".join(lines[at : at + body_lines])
    return None


def check_source(text: str, guard: dict) -> list[str]:
    """Problems with `guard` in the source `text`."""
    guard = {**{key: [] for key in LIST_KEYS}, **guard}
    name = guard["function"]
    hint = (
        f"  perf guard #{guard['issue']}: {guard['reason']}\n"
        f"  measure first: {guard['bench']}\n"
        f"  then change the guard in {REGISTRY} together with the code (#{guard['issue']})"
    )
    found = find_function(text, name)
    if found is None:
        return [f"{guard['file']}: fn {name} not found\n{hint}"]
    above, body = found
    problems = []
    attributes = {squeeze(line) for line in above if line.startswith("#[")}
    for attribute in guard["attributes"]:
        if squeeze(attribute) not in attributes:
            problems.append(f"{guard['file']}: fn {name} lost {attribute}")
    code = squeeze(without_literals(strip_comments(body)))
    for shape in guard["require"]:
        if squeeze(shape) not in code:
            problems.append(f"{guard['file']}: fn {name} no longer has `{shape}`")
    for shape in guard["forbid"]:
        if squeeze(shape) in code:
            problems.append(f"{guard['file']}: fn {name} has `{shape}` again")
    marker = f"PERF GUARD #{guard['issue']}"
    if marker not in "\n".join(above) and marker not in body:
        problems.append(f"{guard['file']}: fn {name} lost its `// {marker}` comment")
    if problems:
        # The reason and the bench once per guard, after its problems.
        problems[-1] += f"\n{hint}"
    return problems


def check_repo(root: Path) -> list[str]:
    problems = []
    for guard in load_guards(root / REGISTRY):
        path = root / guard["file"]
        if not path.is_file():
            problems.append(f"{guard['file']}: file not found (perf guard #{guard['issue']}, {REGISTRY})")
            continue
        problems += check_source(path.read_text(), guard)
    return problems


def main() -> int:
    problems = check_repo(ROOT)
    for problem in problems:
        print(problem, file=sys.stderr)
    if problems:
        print(f"perf_guard_lint: {len(problems)} problem(s), see {REGISTRY}", file=sys.stderr)
        return 1
    print(f"perf_guard_lint: {len(load_guards(ROOT / REGISTRY))} guards hold")
    return 0


if __name__ == "__main__":
    sys.exit(main())
