#!/usr/bin/env -S uv run --script
"""Knowledge bundle lint (#3438): docs/knowledge/ keeps the OKF core.

    uv run ci/knowledge_lint.py [bundle-dir]

Every markdown file in the bundle starts with YAML frontmatter (`---` lines)
holding a non-empty `type` and `description`, and every file other than
index.md is linked from index.md. Stdlib only: the frontmatter is flat
`key: value` lines, so no YAML parser is needed.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
REQUIRED = ("type", "description")


def frontmatter(text: str) -> dict[str, str] | None:
    lines = text.splitlines()
    if not lines or lines[0].strip() != "---":
        return None
    fields = {}
    for line in lines[1:]:
        if line.strip() == "---":
            return fields
        m = re.match(r"^([A-Za-z_][\w-]*):\s*(.*?)\s*(?:#.*)?$", line)
        if m:
            fields[m.group(1)] = m.group(2).strip("\"'")
    return None  # no closing ---


def problems(bundle: Path) -> list[str]:
    out = []
    index = bundle / "index.md"
    linked = set(re.findall(r"\]\(([^)#\s]+\.md)\)", index.read_text(encoding="utf-8"))) if index.is_file() else set()
    if not index.is_file():
        out.append(f"{index.relative_to(ROOT) if index.is_relative_to(ROOT) else index}: missing")
    for path in sorted(bundle.rglob("*.md")):
        rel = path.relative_to(bundle).as_posix()
        fields = frontmatter(path.read_text(encoding="utf-8"))
        if fields is None:
            out.append(f"docs/knowledge/{rel}: no YAML frontmatter (--- ... ---) at the top")
            continue
        out += [f"docs/knowledge/{rel}: frontmatter has no non-empty `{k}`" for k in REQUIRED if not fields.get(k)]
        if rel != "index.md" and rel not in linked:
            out.append(f"docs/knowledge/{rel}: not listed in docs/knowledge/index.md")
    return out


def main(argv: list[str]) -> int:
    bundle = Path(argv[1]) if len(argv) > 1 else ROOT / "docs" / "knowledge"
    found = problems(bundle)
    for p in found:
        print(f"knowledge_lint: {p}", file=sys.stderr)
    if found:
        print("knowledge_lint: see docs/knowledge/index.md for the OKF core the bundle keeps (#3438)", file=sys.stderr)
        return 1
    print(f"knowledge_lint: {len(list(bundle.rglob('*.md')))} files ok")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
