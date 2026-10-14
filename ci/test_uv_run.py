#!/usr/bin/env -S uv run --script
"""Python is run through `uv run`, never bare `python`/`python3` (#637).

Usage:
    uv run python -m unittest ci/test_uv_run.py

Scans every tracked text file for *invocations* of bare `python`/`python3`:
command lines in shell scripts, workflows, composite actions, local-gate.toml,
CMake `COMMAND`s, Dockerfiles, usage lines in Python docstrings, and Markdown
code (fenced blocks and inline `code`). A line is accepted when `uv run`,
`uvx` or `uv tool run` precedes the interpreter (on that line or on the
previous, backslash-continued one), e.g. `uv run python -m unittest ...`.

Shebangs: a standalone script uses `#!/usr/bin/env -S uv run --script`; the
plain `#!/usr/bin/env python3` form is flagged like any other invocation.

Allowlist (keep small): trees that are not ours to rewrite.
"""

import re
import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# Path prefixes / files that are exempt, each with the reason.
ALLOWLIST = {
    "deps/": "vendored third-party trees",
    "plugins/": "upstream plugins and git submodules (obs-websocket, ...)",
    "ci/test_uv_run.py": "this file spells out the patterns it forbids",
    "ci/test_tool_guard.py": "spells out the bare commands the tool guard denies",
}

SCANNED_SUFFIXES = {
    ".sh", ".bash", ".zsh", ".yaml", ".yml", ".toml", ".md", ".py", ".cmake",
    ".ps1", ".bat", ".cmd", ".mk", ".cfg", ".in",
}  # fmt: skip
SCANNED_NAMES = {"lint", "unittest", "Dockerfile", "Makefile", "Justfile", "justfile",
                 "CMakeLists.txt"}  # fmt: skip

# A bare interpreter token: not part of a path, option, package or CMake name.
_TOKEN = r"(?<![\w./:+-])python3?(?:\.\d+)?(?![\w./:+-])"
# ... used as a command: followed by an option, a script, a heredoc/stdin, or
# closed as a list element ("python3", / 'python3'] ).
_CALL = re.compile(_TOKEN + r"(?=\s+(?:-\w|-\s|-$|[\w./-]+\.py\b|<|[\"$])|[\"'],?)")
_SHEBANG = re.compile(r"^#!.*[ /]python3?\b")
_CMAKE_CALL = re.compile(r"\bCOMMAND\s+" + _TOKEN)
_UV = re.compile(r"\buv\s+(?:tool\s+)?run\b|\buvx\b|-S\s+uv\s+run")
_FENCE = re.compile(r"^\s*(```|~~~)")
_INLINE = re.compile(r"`([^`]+)`")


def _allowed(path: str) -> bool:
    return any(path == a or (a.endswith("/") and path.startswith(a)) for a in ALLOWLIST)


def _tracked() -> list[str]:
    out = subprocess.run(
        ["git", "ls-files", "-z"], cwd=ROOT, check=True, capture_output=True
    ).stdout.decode()
    return [p for p in out.split("\0") if p]


def _scanned(path: str) -> bool:
    p = Path(path)
    return p.suffix in SCANNED_SUFFIXES or p.name in SCANNED_NAMES


def _flag(line: str, prev: str, cmake: bool) -> bool:
    if _UV.search(line) or (prev.rstrip().endswith("\\") and _UV.search(prev)):
        return False
    if _SHEBANG.match(line):
        return True
    return bool((_CMAKE_CALL if cmake else _CALL).search(line))


def find_violations(path: str, text: str) -> list[str]:
    cmake = path.endswith((".cmake", "CMakeLists.txt"))
    md = path.endswith(".md")
    hits, prev, fenced = [], "", False
    for n, line in enumerate(text.splitlines(), 1):
        if md:
            if _FENCE.match(line):
                fenced = not fenced
                prev = line
                continue
            candidates = [line] if fenced else _INLINE.findall(line)
        else:
            candidates = [line]
        for c in candidates:
            if _flag(c, prev if fenced or not md else "", cmake):
                hits.append(f"{path}:{n}: {line.strip()}")
                break
        prev = line
    return hits


class UvRunTest(unittest.TestCase):
    def test_detector(self):
        bad = [
            "python3 -m unittest ci/test_x.py",
            "  python foo.py",
            "python3 -c 'print(1)'",
            "python3 - <<EOF",
            'run = ["python3", "ci/local_gate.py"]',
            "#!/usr/bin/env python3",
            "TIER4_PORT=1 python3 run.py",
        ]
        good = [
            "uv run python -m unittest ci/test_x.py",
            "uv run tools/leaderboard/generate.py",
            "uv run --script x.py",
            "uvx --from spec python -c code",
            "#!/usr/bin/env -S uv run --script",
            "brew install --quiet python3",
            "libluajit-5.1-dev python3-dev",
            "include(cmake/python.cmake)",
            "Python is great",
        ]
        for s in bad:
            self.assertTrue(_flag(s, "", False), s)
        for s in good:
            self.assertFalse(_flag(s, "", False), s)
        self.assertTrue(_flag("COMMAND python3 gen.py", "", True))
        self.assertFalse(_flag("LANGUAGE python", "", True))
        self.assertFalse(_flag("python3 -m x", "uv run \\", False))
        self.assertEqual(find_violations("a.md", "prose python3 here\n`python3 x.py`\n"), ["a.md:2: `python3 x.py`"])

    def test_no_bare_python_invocations(self):
        hits = []
        for path in _tracked():
            if _allowed(path) or not _scanned(path):
                continue
            f = ROOT / path
            if not f.is_file():
                continue
            try:
                text = f.read_text(encoding="utf-8")
            except (UnicodeDecodeError, OSError):
                continue
            hits += find_violations(path, text)
        self.assertEqual(hits, [], f"{len(hits)} bare python invocation(s); use `uv run`:\n" + "\n".join(hits))

    def test_allowlist_is_small(self):
        self.assertLessEqual(len(ALLOWLIST), 5)


if __name__ == "__main__":
    unittest.main()
