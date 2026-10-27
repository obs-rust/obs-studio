#!/usr/bin/env -S uv run --script
"""Build libobs with ENABLE_CORE_MODULES=OFF (obs-rust/obs-studio#2280).

    uv run --script ci/core_modules_off.py BUILD_ROOT [extra cmake args...]

Builds each backend (C and the default HYBRID, with the C reference) in its own
build tree under BUILD_ROOT, then checks the generated core module list is
empty: obs_core_modules_count 0 and a NULL-only array.

Extra args go to every configure (e.g. -DRust_TOOLCHAIN=..., -DCMocka_DIR=...).
Env: BACKENDS (default "C HYBRID"), JOBS (ninja -j, default: ninja's own).
"""

from __future__ import annotations

import os
import re
import subprocess
import sys
from pathlib import Path

# The two halves of "no core modules": the count, and an array of NULL alone.
_COUNT = re.compile(r"^const unsigned int obs_core_modules_count = 0;$", re.MULTILINE)
_ARRAY = re.compile(r"obs_core_modules\[\] = \{\n\tNULL\n\};")
# The backend's build settings: C reference only beside the hybrid library.
_BACKENDS = {"C": ("OFF", ["libobs"]), "HYBRID": ("ON", ["libobs", "libobs-c-reference"])}


def configure(src: Path, build: Path, backend: str, reference: str, extra: list[str]) -> None:
    """One build tree, core modules off, with the caller's extra settings."""
    subprocess.run([
        "cmake", "-S", str(src), "-B", str(build), "-G", "Ninja",
        "-DCMAKE_BUILD_TYPE=RelWithDebInfo",
        f"-DOBS_LIBOBS_BACKEND={backend}", f"-DOBS_BUILD_C_REFERENCE={reference}",
        "-DENABLE_CORE_MODULES=OFF", "-DENABLE_FRONTEND=OFF", "-DENABLE_SCRIPTING=OFF",
        "-DENABLE_UNIT_TESTS=OFF", *extra,
    ], check=True)


def build(build: Path, targets: list[str]) -> None:
    """The libraries only: this check reads what they generate."""
    jobs = ["-j", os.environ["JOBS"]] if os.environ.get("JOBS") else []
    subprocess.run(["ninja", *jobs, "-C", str(build), *targets], check=True)


def empty_list(path: Path) -> bool:
    """Does the generated list name no core module at all?"""
    text = path.read_text(encoding="utf-8", errors="replace")
    return bool(_COUNT.search(text)) and bool(_ARRAY.search(text))


def run_backend(src: Path, build_root: Path, backend: str, extra: list[str]) -> None:
    reference, targets = _BACKENDS[backend]
    build_tree = build_root / backend
    print(f"== {backend}: configure")
    configure(src, build_tree, backend, reference, extra)
    print(f"== {backend}: build {' '.join(targets)}")
    build(build_tree, targets)
    generated = build_tree / "libobs" / "obs-core-modules.c"
    if not empty_list(generated):
        sys.stderr.write(f"{backend}: {generated} is not an empty, NULL-terminated list:\n")
        sys.stderr.write(generated.read_text(encoding="utf-8", errors="replace"))
        raise SystemExit(1)
    print(f"== {backend}: ok")


def main(argv: list[str]) -> int:
    if len(argv) < 2:
        print("usage: core_modules_off.py BUILD_ROOT [extra cmake args...]", file=sys.stderr)
        return 2
    build_root, extra = Path(argv[1]), argv[2:]
    src = Path(__file__).resolve().parent.parent
    build_root.mkdir(parents=True, exist_ok=True)
    for backend in os.environ.get("BACKENDS", "C HYBRID").split():
        if backend not in _BACKENDS:
            print(f"unknown backend {backend}", file=sys.stderr)
            return 2
        run_backend(src, build_root, backend, extra)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
