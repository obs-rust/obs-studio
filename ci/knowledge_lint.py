#!/usr/bin/env -S uv run --script
"""Knowledge bundle lint (#3438): docs/knowledge/ keeps the OKF core.

    uv run ci/knowledge_lint.py [bundle-dir] [--fix]

Every markdown file in the bundle starts with YAML frontmatter (`---` lines)
holding a non-empty `type` and `description`, and every file other than
index.md also a `title`. The concept list in index.md, between the two marker
lines, is generated from that frontmatter, sorted by file name: a concept PR
adds its file and the regenerated list, and when two of them collide in the
merge queue, the fix is a rebase and `--fix` (`./lint --fix` runs it). Stdlib
only: the frontmatter is flat `key: value` lines, so no YAML parser is needed.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
REQUIRED = ("type", "description")
START = "<!-- concepts: generated, `./lint --fix` writes this list -->"
END = "<!-- /concepts -->"
LIST_RE = re.compile(re.escape(START) + r"\n.*?" + re.escape(END), re.DOTALL)


def frontmatter(text: str) -> dict[str, str] | None:
    lines = text.splitlines()
    if not lines or lines[0].strip() != "---":
        return None
    fields = {}
    for line in lines[1:]:
        if line.strip() == "---":
            return fields
        m = re.match(r"^([A-Za-z_][\w-]*):\s*(.*?)\s*$", line)
        if m:
            fields[m.group(1)] = m.group(2)
    return None  # no closing ---


def plain(value: str) -> str:
    """A frontmatter value without the quotes around it."""
    return value[1:-1] if len(value) > 1 and value[0] == value[-1] and value[0] in "\"'" else value


def is_comment_cut(value: str) -> bool:
    """YAML ends a plain value at ` #`: the rest would be a comment, not text."""
    return value[:1] not in ("'", '"') and re.search(r"\s#", value) is not None


def concept_list(bundle: Path) -> str:
    """The generated part of index.md, markers included."""
    lines = [START]
    for path in sorted(bundle.rglob("*.md")):
        rel = path.relative_to(bundle).as_posix()
        fields = frontmatter(path.read_text(encoding="utf-8"))
        if rel != "index.md" and fields:
            lines.append(f"- [{plain(fields.get('title', ''))}]({rel}) — {plain(fields.get('description', ''))}")
    lines.append(END)
    return "\n".join(lines)


def file_problems(bundle: Path) -> list[str]:
    out = []
    if not (bundle / "index.md").is_file():
        out.append("docs/knowledge/index.md: missing")
    for path in sorted(bundle.rglob("*.md")):
        rel = path.relative_to(bundle).as_posix()
        fields = frontmatter(path.read_text(encoding="utf-8"))
        if fields is None:
            out.append(f"docs/knowledge/{rel}: no YAML frontmatter (--- ... ---) at the top")
            continue
        required = REQUIRED if rel == "index.md" else (*REQUIRED, "title")
        out += [f"docs/knowledge/{rel}: frontmatter has no non-empty `{k}`" for k in required if not fields.get(k)]
        out += [
            f"docs/knowledge/{rel}: quote `{k}`: YAML ends a plain value at ` #`"
            for k in ("title", "description")
            if is_comment_cut(fields.get(k, ""))
        ]
    return out


def problems(bundle: Path) -> list[str]:
    out = file_problems(bundle)
    if out:
        return out
    text = (bundle / "index.md").read_text(encoding="utf-8")
    found = LIST_RE.search(text)
    if found is None:
        return [f"docs/knowledge/index.md: no concept list between the marker lines {START} and {END}"]
    if found.group(0) != concept_list(bundle):
        return ["docs/knowledge/index.md: the concept list does not match the files; run `./lint --fix`"]
    return []


def fix(bundle: Path) -> None:
    """Write the concept list, when the files it comes from are sound."""
    index = bundle / "index.md"
    if file_problems(bundle):
        return
    text = index.read_text(encoding="utf-8")
    if LIST_RE.search(text):
        index.write_text(LIST_RE.sub(lambda _: concept_list(bundle), text), encoding="utf-8")


def main(argv: list[str]) -> int:
    args = [a for a in argv[1:] if a != "--fix"]
    bundle = Path(args[0]) if args else ROOT / "docs" / "knowledge"
    if "--fix" in argv[1:]:
        fix(bundle)
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
