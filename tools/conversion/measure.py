#!/usr/bin/env -S uv run --script
"""Measure how much of OBS is converted to Rust (#2242).

    uv run tools/conversion/measure.py --json conversion.json --svg conversion.svg
    uv run tools/conversion/measure.py --json conversion.json --svg conversion.svg --check

The honest measure is the hybrid build: a libobs implementation file is
ported when the hybrid build drops it, which is when only the
`libobs/build/libobs.ported-*` unity groups include it. The groups are read
by rust/tools/ported_sources.py, the same definition the Tier 2 check uses,
so the two cannot drift.

1. libobs: lines of implementation that the C reference build compiles per
   host, the ported share of them, and the same split per area.
2. Whole app: libobs plus frontend/, plugins/ and shared/. obs-websocket is
   kept in C++ by policy (AGENTS.md) and is listed apart, as kept by design.
3. EXCLUDED below is left out of every count.
4. Rust lines (non-test, non-oracle) are a secondary number.
5. Same tree, same numbers: walks are sorted and nothing reads the clock.
   The date and commit come from git HEAD unless given. --check exits 1 when
   the files on disk differ from a fresh run.
"""
from __future__ import annotations

import argparse
import importlib.util
import json
import os
import subprocess
import sys
from pathlib import Path
from xml.sax.saxutils import escape

ROOT = Path(__file__).resolve().parents[2]


def _load_ported_sources():
    spec = importlib.util.spec_from_file_location("ported_sources", ROOT / "rust/tools/ported_sources.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


ported_sources = _load_ported_sources()

HOSTS = ported_sources.HOSTS
# Non-ported source sets the C reference build compiles, per host.
# LIBOBS_RUST_SOURCES is hybrid-only glue, so it is not part of the reference.
# A "build/..." entry is a group that libobs/cmake/os-linux.cmake adds behind
# a feature option instead of through source-sets.json (pulse, wayland, the
# GlobalShortcuts portal: all on, as ported_sources.py assumes by default).
# build/libobs.null-audio+.h.c is the pulse-off alternative, so it is left out.
REFERENCE_SETS = {
    "linux": ["LIBOBS_SOURCES", "LIBOBS_LINUX_SOURCES", "build/libobs.pulse+.h.c",
              "build/libobs.nix-wayland+.h.c", "build/libobs.nix-portal-shortcuts+.h.c"],
    "windows": ["LIBOBS_SOURCES", "LIBOBS_WINDOWS_SOURCES", "LIBOBS_WINDOWS_OBFUSCATE_SOURCES"],
    "macos": ["LIBOBS_SOURCES", "LIBOBS_MACOS_SOURCES"],
}
AREAS = (("util/", "util"), ("media-io/", "media-io"), ("graphics/", "graphics"))

# Whole-app trees walked on disk (libobs is counted from its build groups).
APP_DIRS = ("frontend", "plugins", "shared")
KEPT_BY_DESIGN = ("plugins/obs-websocket",)
IMPLEMENTATION = (".c", ".cpp", ".m", ".mm", ".c.h", ".cpp.hpp", ".m.h", ".mm.h", ".mm.hpp")

# Left out of every count. Prefixes are relative to the repo root; parts
# match any path component.
EXCLUDED = (
    "rust/obs-c-oracle/legacy/ (Tier 3 oracle copy of the original C)",
    "test/ and any tests/ or test/ directory (tests, not product code)",
    "rust/*/tests/ and rust/target/ (Rust tests, build output)",
    "deps/ (vendored third party: blake2, glad (generated), json11, libcaption, libdshowcapture, w32-pthreads)",
    "plugins/obs-outputs/librtmp/ (vendored librtmp)",
    "plugins/obs-browser/deps/ (vendored)",
    "unity groups (*+.h.c, *+.hpp.cpp, ...) and precompiled headers (*.pch.h): include lists, not code",
    "headers (*.h, *.hpp): declarations; implementation in build-role names (foo.c.h) is counted",
)
EXCLUDED_PREFIXES = ("rust/obs-c-oracle/legacy/", "test/", "deps/", "rust/target/",
                     "plugins/obs-outputs/librtmp/", "plugins/obs-browser/deps/")
EXCLUDED_PARTS = {"test", "tests", "target", ".git"}


def count_lines(path: Path) -> int:
    return path.read_bytes().count(b"\n")


def percent(part: int, whole: int) -> float:
    return round(100.0 * part / whole, 2) if whole else 0.0


def _is_group(entry: str) -> bool:
    return "+." in entry


def _members(root: Path, sets: dict, names: list[str]) -> set[str]:
    found = set()
    for name in names:
        for entry in [name] if name.startswith("build/") else sets.get(name, []):
            if _is_group(entry) and (root / "libobs" / entry).is_file():
                found.update(ported_sources.group_members(root, entry))
    return found


def _area(member: str) -> str:
    for prefix, name in AREAS:
        if member.startswith(prefix):
            return name
    return "core"


def _split(root: Path, files: set[str], ported: set[str]) -> dict:
    lines = {f: count_lines(root / "libobs" / f) for f in files}
    total = sum(lines.values())
    done = sum(lines[f] for f in ported)
    areas = {}
    for _, name in AREAS + (("", "core"),):
        mine = [f for f in files if _area(f) == name]
        a_total = sum(lines[f] for f in mine)
        a_done = sum(lines[f] for f in mine if f in ported)
        areas[name] = {"total_lines": a_total, "ported_lines": a_done, "percent": percent(a_done, a_total)}
    return {"total_lines": total, "ported_lines": done, "hybrid_lines": total - done,
            "percent": percent(done, total), "files": len(files), "ported_files": sorted(ported),
            "areas": areas}


def measure_libobs(root: Path) -> dict:
    sets = ported_sources.source_sets(root)
    hosts, all_files, all_ported = {}, set(), set()
    for host in HOSTS:
        kept = _members(root, sets, REFERENCE_SETS[host])
        ported = _members(root, sets, ported_sources.ported_set_names(host)) - kept
        files = kept | ported
        hosts[host] = _split(root, files, ported)
        all_files |= files
        all_ported |= ported
    # A file ported on one host but compiled from a non-ported group on
    # another is not ported everywhere.
    all_ported -= set().union(*(_members(root, sets, REFERENCE_SETS[h]) for h in HOSTS))
    everything = _split(root, all_files, all_ported)
    del everything["ported_files"]
    return {"hosts": hosts, "all": everything}


def _excluded(rel: str) -> bool:
    return rel.startswith(EXCLUDED_PREFIXES) or bool(EXCLUDED_PARTS & set(rel.split("/")[:-1]))


def _walk(root: Path, top: str, suffixes: tuple[str, ...]):
    for dirpath, dirnames, filenames in os.walk(root / top):
        dirnames.sort()
        rel_dir = Path(dirpath).relative_to(root).as_posix()
        for name in sorted(filenames):
            rel = f"{rel_dir}/{name}"
            if _excluded(rel) or "+." in name or ".pch." in name or not name.endswith(suffixes):
                continue
            path = Path(dirpath) / name
            if path.is_file() and not path.is_symlink():
                yield rel, path


def measure(root: Path, commit: str | None = None, date: str | None = None) -> dict:
    root = Path(root)
    if commit is None or date is None:
        git_commit, git_date = _git_head(root)
        commit = commit or git_commit
        date = date or git_date
    libobs = measure_libobs(root)
    by_dir = {"libobs": libobs["all"]["total_lines"]}
    kept = {k: 0 for k in KEPT_BY_DESIGN}
    for top in APP_DIRS:
        for rel, path in _walk(root, top, IMPLEMENTATION):
            owner = next((k for k in KEPT_BY_DESIGN if rel.startswith(k + "/")), None)
            if owner:
                kept[owner] += count_lines(path)
            else:
                by_dir[top] = by_dir.get(top, 0) + count_lines(path)
    total = sum(by_dir.values())
    ported = libobs["all"]["ported_lines"]
    rust = sum(count_lines(p) for _, p in _walk(root, "rust", (".rs",)))
    return {
        "commit": commit,
        "date": date,
        "libobs": libobs,
        "app": {"total_lines": total, "ported_lines": ported, "percent": percent(ported, total),
                "by_dir": by_dir},
        "kept_by_design": kept,
        "rust": {"lines": rust},
        "excluded": list(EXCLUDED),
    }


def _git_head(root: Path) -> tuple[str, str]:
    try:
        out = subprocess.run(["git", "-C", str(root), "log", "-1", "--format=%h %cs"],
                             capture_output=True, text=True, check=True).stdout.split()
        return out[0], out[1]
    except (OSError, subprocess.CalledProcessError, IndexError):
        return "unknown", "unknown"


def render_svg(data: dict) -> str:
    """Text-only SVG; colors follow the viewer's light or dark theme."""
    lib = data["libobs"]["all"]["percent"]
    app = data["app"]["percent"]
    hosts = data["libobs"]["hosts"]
    host_line = "  ".join(f"{h} {hosts[h]['percent']:.1f}%" for h in HOSTS)
    ws = data["kept_by_design"].get("plugins/obs-websocket", 0)
    foot = (f"Rust: {data['rust']['lines']:,} lines · obs-websocket kept in C++ by design "
            f"({ws:,} lines) · {data['date']} · {data['commit']}")
    w, h = 640, 190

    def bar(y: int, pct: float) -> str:
        fill = round(560 * min(max(pct, 0.0), 100.0) / 100.0, 1)
        return (f'<rect class="track" x="40" y="{y}" width="560" height="10" rx="5"/>'
                f'<rect class="fill" x="40" y="{y}" width="{fill}" height="10" rx="5"/>')

    parts = [
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" '
        f'role="img" aria-label="libobs {lib}% converted to Rust, whole app {app}%">',
        "<style>",
        ".bg{fill:#ffffff;stroke:#d0d7de}.t{fill:#1f2328}.m{fill:#59636e}"
        ".track{fill:#d0d7de}.fill{fill:#b7410e}",
        "text{font-family:-apple-system,'Segoe UI',Helvetica,Arial,sans-serif}",
        "@media (prefers-color-scheme: dark){.bg{fill:#0d1117;stroke:#3d444d}.t{fill:#f0f6fc}"
        ".m{fill:#9198a1}.track{fill:#3d444d}.fill{fill:#f0883e}}",
        "</style>",
        f'<rect class="bg" x="0.5" y="0.5" width="{w - 1}" height="{h - 1}" rx="8"/>',
        '<text class="t" x="40" y="34" font-size="16" font-weight="600">OBS converted to Rust</text>',
        f'<text class="t" x="40" y="70" font-size="14">libobs</text>'
        f'<text class="t" x="600" y="70" font-size="20" font-weight="700" text-anchor="end">{lib}%</text>',
        bar(78, lib),
        f'<text class="t" x="40" y="118" font-size="14">Whole app</text>'
        f'<text class="t" x="600" y="118" font-size="20" font-weight="700" text-anchor="end">{app}%</text>',
        bar(126, app),
        f'<text class="m" x="40" y="160" font-size="11">libobs per host: {escape(host_line)}</text>',
        f'<text class="m" x="40" y="176" font-size="11">{escape(foot)}</text>',
        "</svg>",
    ]
    return "\n".join(parts) + "\n"


def render_json(data: dict) -> str:
    return json.dumps(data, indent=2, sort_keys=True) + "\n"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--json", type=Path, help="write the measurement as JSON")
    parser.add_argument("--svg", type=Path, help="write the README card as SVG")
    parser.add_argument("--commit", help="commit to stamp (default: git HEAD)")
    parser.add_argument("--date", help="date to stamp (default: HEAD's commit date)")
    parser.add_argument("--check", action="store_true", help="exit 1 if --json/--svg differ from a fresh run")
    args = parser.parse_args(argv)
    data = measure(args.root, args.commit, args.date)
    outputs = [(p, text) for p, text in ((args.json, render_json(data)), (args.svg, render_svg(data))) if p]
    if not outputs:
        sys.stdout.write(render_json(data))
        return 0
    stale = []
    for path, text in outputs:
        if args.check:
            if not path.is_file() or path.read_text() != text:
                stale.append(str(path))
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text)
    if stale:
        print("out of date: " + ", ".join(stale), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
