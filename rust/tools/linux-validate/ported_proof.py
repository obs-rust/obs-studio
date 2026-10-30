#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# ///
"""The Tier 2 ported-object proof, answered from one Ninja graph load per target (#3046).

    uv run --no-project rust/tools/linux-validate/ported_proof.py \
        --build /build/tree --src /ro --objects /build/ported-objects.txt --nvenc

`validate.sh` proves that every ported object is absent from hybrid `libobs`
and present in `libobs-c-reference`. Querying that per (object, target) pair
with `ninja -t inputs` plus a `find` of the build tree loaded the whole Ninja
graph about 700 times for 176 objects: 59 s of the merge-queue critical path
in merge-group run 41034283809. Nothing in a target's input list changes
while the loop runs, so it is read once here and the loop answers from it.

The three answers are the ones the shell function gave:

* a direct object below `<target>.dir` (Ninja names it in the target's inputs);
* a source included by one of the target's unity group batches -- the
  `build/....+.h.c.o` objects BUILD.md's generated groups compile;
* a source included by `<target>.dir/Unity/unity_N_c.c`, when
  ENABLE_UNITY_BUILD batches the target's objects.

None of it reads the filesystem for object files: the persistent `/build`
volume can hold a stale `.o` from an older checkout, so the proof asks the
graph, as before.
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path
from typing import Callable, Sequence

Runner = Callable[[Sequence[str]], str]


def ninja_runner(build: Path, ninja: str) -> Runner:
    """`Runner` from the real `ninja -C <build>` (or the test's fake one)."""

    def run(args: Sequence[str]) -> str:
        try:
            done = subprocess.run(
                [ninja, "-C", str(build), *args],
                capture_output=True,
                text=True,
                check=False,
            )
        except OSError:
            return ""
        return done.stdout

    return run


class Proof:
    """One ported-object proof over a configured build tree."""

    def __init__(
        self,
        build: Path,
        src: Path,
        *,
        ninja: str = "ninja",
        run: Runner | None = None,
    ) -> None:
        self.build = Path(build)
        self.src = Path(src)
        self._run: Runner = run or ninja_runner(self.build, ninja)
        self._inputs: dict[str, list[str]] = {}
        self._unity: dict[str, set[str]] = {}

    def inputs(self, target: str) -> list[str]:
        """The target's Ninja input paths, read from the graph once."""
        if target not in self._inputs:
            text = self._run(["-t", "inputs", target])
            self._inputs[target] = text.splitlines() if text else []
        return self._inputs[target]

    def unity_includes(self, target: str) -> set[str]:
        """`#include "..."` paths of the target's generated unity batches, read once."""
        if target in self._unity:
            return self._unity[target]
        includes: set[str] = set()
        unity = self.build / "libobs" / "CMakeFiles" / f"{target}.dir" / "Unity"
        if not unity.is_dir():
            unity = next(
                iter(sorted(self.build.rglob(f"CMakeFiles/{target}.dir/Unity"))),
                None,
            )
        if unity is not None:
            for batch in sorted(unity.glob("*.c")):
                for line in batch.read_text(errors="replace").splitlines():
                    found = re.match(r'^#include "(.*)"$', line)
                    if found:
                        includes.add(found.group(1))
        self._unity[target] = includes
        return includes

    def compiles(self, target: str, source: str, object_name: str) -> bool:
        """Does `target` compile `source` into `object_name` (or inside its batch)?"""
        inputs = self.inputs(target)
        if any(f"{target}.dir/{object_name}" in line for line in inputs):
            return True
        group_re = re.compile(
            rf"CMakeFiles/{re.escape(target)}\.dir/(build/.+?\+\.h\.[cm])\.o$"
        )
        for line in inputs:
            found = group_re.search(line)
            if not found:
                continue
            group = self.src / "libobs" / found.group(1)
            try:
                if f'"{source}"' in group.read_text(errors="replace"):
                    return True
            except OSError:
                continue
        return any(path.endswith(f"/{source}") for path in self.unity_includes(target))


def check_objects(proof: Proof, objects: Sequence[str]) -> list[str]:
    """The ported-object proof: absent from hybrid libobs, present in the C reference."""
    failures = []
    for obj in objects:
        source = f"libobs/{obj.removesuffix('.o')}"
        if proof.compiles("libobs", source, obj):
            failures.append(f"FAIL: {obj} was compiled into hybrid libobs")
        if not proof.compiles("libobs-c-reference", source, obj):
            failures.append(f"FAIL: {obj} was not compiled into libobs-c-reference")
    return failures


def check_nvenc(proof: Proof) -> list[str]:
    """Issue #645: neither obs-nvenc target may compile the other's cuda loader."""
    failures = []
    for target in ("obs-nvenc", "obs-nvenc-c-reference"):
        if target == "obs-nvenc":
            want, other = "cuda-load-rust", "cuda-load"
        else:
            want, other = "cuda-load", "cuda-load-rust"
        # Both targets' unity groups are named obs-nvenc.*: the twin compiles
        # the same groups with its own loader swapped in.
        compiles_want = proof.compiles(target, "-", f"build/obs-nvenc.{want}+.h.c.o")
        compiles_other = proof.compiles(target, "-", f"build/obs-nvenc.{other}+.h.c.o")
        if not compiles_want or compiles_other:
            failures.append(f"FAIL: {target} must compile {want} and not {other}")
    return failures


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--build", required=True, type=Path, help="configured build tree"
    )
    parser.add_argument("--src", required=True, type=Path, help="source checkout")
    parser.add_argument(
        "--objects",
        type=Path,
        help="file listing the ported objects, one per line or space separated",
    )
    parser.add_argument(
        "--nvenc", action="store_true", help="also check the cuda loaders"
    )
    parser.add_argument(
        "--ninja", default="ninja", help="ninja executable (tests use a fake)"
    )
    args = parser.parse_args(argv)

    proof = Proof(args.build, args.src, ninja=args.ninja)
    listing: Path | None = args.objects
    objects = listing.read_text().split() if listing else []
    print(
        f"== ported-object proof: {len(objects)} objects, "
        f"{len({o.split('/')[0] for o in objects})} areas",
        flush=True,
    )
    failures = check_objects(proof, objects)
    if args.nvenc:
        failures += check_nvenc(proof)
    for failure in failures:
        print(failure)
    if failures:
        return 1
    print("== ported-object proof: every ported object is excluded from hybrid libobs")
    return 0


if __name__ == "__main__":
    sys.exit(main())
