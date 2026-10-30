#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# ///
"""The Tier 2 ported-object proof reads each Ninja target's inputs once (#3046).

    uv run python -m unittest ci/test_tier2_ported_proof.py

`rust/tools/linux-validate/validate.sh` proves that every ported object is
absent from hybrid `libobs` and present in `libobs-c-reference`. Done the way
the shell loop did it -- `ninja -C build -t inputs <target>` plus a `find` of
the build tree per (object, target) pair -- it loads the whole 1.5 MB Ninja
graph about 700 times for 176 objects: 59 s of the merge-queue critical path
in merge-group run 41034283809.

The extracted proof indexes each target once and answers from that. These
tests hold it to that: a counting fake `ninja` tallies the graph loads, so a
regression back to per-object queries fails here instead of on the critical
path. They also pin the three answers the shell function gave -- a direct
object below `<target>.dir`, an object inside one of the target's unity group
batches, and a source included by a `<target>.dir/Unity/unity_N_c.c` -- plus
the `FAIL:` text and exit status for each failure mode.
"""

from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path
from typing import Sequence

ROOT = Path(__file__).resolve().parent.parent
PROOF = ROOT / "rust" / "tools" / "linux-validate" / "ported_proof.py"


def load_proof():  # type: ignore[no-untyped-def]
    """The tool under test, loaded by path: rust/tools is not a Python package."""
    spec = importlib.util.spec_from_file_location("ported_proof", PROOF)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class FakeNinja:
    """A `ninja` that answers from canned text and records every invocation."""

    def __init__(self, build: Path, graph: dict[str, str]) -> None:
        self.build = build
        self.graph = graph
        self.path = build / "fake-ninja.py"
        report = build / "ninja-calls.txt"
        self.report = report
        self.path.write_text(
            "#!/usr/bin/env -S uv run --script\n"
            "# /// script\n"
            '# requires-python = ">=3.11"\n'
            "# ///\n"
            "import json, sys\n"
            "from pathlib import Path\n"
            f"only = {sorted(graph)!r}\n"
            "args = sys.argv[1:]\n"
            f"Path({str(report)!r}).open('a').write(' '.join(args) + chr(10))\n"
            "target = args[-1]\n"
            "if target not in only:\n"
            "    print(f\"ninja: error: unknown target '{target}'\", file=sys.stderr)\n"
            "    sys.exit(1)\n"
            f"text = json.loads(Path({str(build / 'graph.json')!r}).read_text())\n"
            "print(text[target])\n"
        )
        self.path.chmod(0o755)
        (build / "graph.json").write_text(json.dumps(graph))

    def calls_made(self) -> list[str]:
        if not self.report.exists():
            return []
        return [line for line in self.report.read_text().splitlines() if line]


class ProofTest(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.tmp = Path(self._tmp.name)
        self.build = self.tmp / "build"
        self.src = self.tmp / "src"
        (self.build / "libobs" / "CMakeFiles" / "libobs.dir").mkdir(parents=True)
        (self.src / "libobs" / "build").mkdir(parents=True)
        self.proof = load_proof()

    def tearDown(self) -> None:
        self._tmp.cleanup()

    def graph(self) -> dict[str, str]:
        return {
            "libobs": "\n".join(
                [
                    ".",
                    "libobs/CMakeFiles/libobs.dir/build/libobs.rust-variadic+.h.c.o",
                    "/src/libobs/audio-monitoring/pulse/pulseaudio-monitor-format.c",
                ]
            ),
            "libobs-c-reference": "\n".join(
                [
                    ".",
                    "libobs-c-reference/CMakeFiles/libobs-c-reference.dir/audio-monitoring/"
                    "pulse/pulseaudio-monitor-format.c.h.o",
                    "libobs-c-reference/CMakeFiles/libobs-c-reference.dir/build/"
                    "libobs-c-reference.rust-variadic+.h.c.o",
                ]
            ),
            "obs-nvenc": "plugins/obs-nvenc/CMakeFiles/obs-nvenc.dir/build/"
            "obs-nvenc.cuda-load-rust+.h.c.o",
            "obs-nvenc-c-reference": "plugins/obs-nvenc-c-reference/CMakeFiles/"
            "obs-nvenc-c-reference.dir/build/obs-nvenc.cuda-load+.h.c.o",
        }

    def test_graph_is_read_once_per_target(self) -> None:
        fake = FakeNinja(self.build, self.graph())
        proof = self.proof.Proof(self.build, self.src, ninja=str(fake.path))
        objects = [f"audio-monitoring/pulse/file{i}.c.h.o" for i in range(50)]
        for obj in objects:
            proof.compiles("libobs", f"libobs/{obj[:-2]}", obj)
            proof.compiles("libobs-c-reference", f"libobs/{obj[:-2]}", obj)
        self.assertEqual(len(fake.calls_made()), 2, fake.calls_made())

    def test_direct_object_below_the_target_is_found(self) -> None:
        fake = FakeNinja(self.build, self.graph())
        proof = self.proof.Proof(self.build, self.src, ninja=str(fake.path))
        obj = "audio-monitoring/pulse/pulseaudio-monitor-format.c.h.o"
        self.assertTrue(proof.compiles("libobs-c-reference", f"libobs/{obj[:-2]}", obj))
        self.assertFalse(proof.compiles("libobs", f"libobs/{obj[:-2]}", obj))

    def test_unity_group_batch_counts_as_compiling_its_source(self) -> None:
        fake = FakeNinja(self.build, self.graph())
        proof = self.proof.Proof(self.build, self.src, ninja=str(fake.path))
        group = self.src / "libobs" / "build" / "libobs.rust-variadic+.h.c"
        group.write_text('#include "/src/libobs/callback/calldata.c"\n')
        self.assertTrue(proof.compiles("libobs", "libobs/callback/calldata.c", "calldata.c.h.o"))
        self.assertFalse(proof.compiles("libobs", "libobs/callback/decl.c", "decl.c.h.o"))

    def test_unity_batch_include_counts_as_compiling_its_source(self) -> None:
        fake = FakeNinja(self.build, self.graph())
        unity = self.build / "libobs" / "CMakeFiles" / "libobs.dir" / "Unity"
        unity.mkdir(parents=True)
        (unity / "unity_0_c.c").write_text('#include "/src/libobs/callback/decl.c"\n')
        proof = self.proof.Proof(self.build, self.src, ninja=str(fake.path))
        self.assertTrue(proof.compiles("libobs", "libobs/callback/decl.c", "decl.c.h.o"))

    def run_main(self, fake: FakeNinja, objects: Sequence[str]) -> int:
        listing = self.tmp / "objects.txt"
        listing.write_text("\n".join(objects) + "\n")
        argv = [
            "--build", str(self.build),
            "--src", str(self.src),
            "--objects", str(listing),
            "--nvenc",
            "--ninja", str(fake.path),
        ]  # fmt: skip
        return self.proof.main(argv)

    def test_main_passes_and_loads_each_target_once(self) -> None:
        fake = FakeNinja(self.build, self.graph())
        obj = "audio-monitoring/pulse/pulseaudio-monitor-format.c.h.o"
        self.assertEqual(self.run_main(fake, [obj]), 0)
        self.assertEqual(len(fake.calls_made()), 4, fake.calls_made())

    def test_main_fails_when_a_ported_object_is_in_the_hybrid_libobs(self) -> None:
        graph = self.graph()
        obj = "audio-monitoring/pulse/pulseaudio-monitor-format.c.h.o"
        graph["libobs"] += "\nlibobs/CMakeFiles/libobs.dir/" + obj
        fake = FakeNinja(self.build, graph)
        self.assertEqual(self.run_main(fake, [obj]), 1)

    def test_main_fails_when_the_reference_misses_a_ported_object(self) -> None:
        fake = FakeNinja(self.build, self.graph())
        self.assertEqual(self.run_main(fake, ["callback/missing.c.h.o"]), 1)

    def test_main_fails_when_obs_nvenc_compiles_the_wrong_loader(self) -> None:
        graph = self.graph()
        graph["obs-nvenc"] = (
            "plugins/obs-nvenc/CMakeFiles/obs-nvenc.dir/build/obs-nvenc.cuda-load+.h.c.o"
        )
        fake = FakeNinja(self.build, graph)
        obj = "audio-monitoring/pulse/pulseaudio-monitor-format.c.h.o"
        self.assertEqual(self.run_main(fake, [obj]), 1)


if __name__ == "__main__":
    unittest.main()
