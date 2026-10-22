#!/usr/bin/env -S uv run --script
"""ci.toml declares every cache family and workflow trigger (#957).

    uv run python -m unittest ci/test_ci_toml.py

ci.toml is the zackees/ci.yml schema 3 contract. The cache janitor
(.github/workflows/ci-pre.yml) deletes every Actions cache whose key matches
no [cache.family], so a cache key added to a workflow without a family would
be deleted on the next PR run. [allow].workflows must list each workflow with
its exact triggers (GEN-008). `ci-lint precheck` checks the rest; it needs the
network, so this offline test covers the two things a workflow edit can break.
"""

from __future__ import annotations

import re
import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CI_TOML = ROOT / "ci.toml"
GITHUB = ROOT / ".github"
WORKFLOWS = GITHUB / "workflows"
PIN = "1a970f01574447d681571ff3d0b8de56cb983dcc"

# Prefixes of the families a `via` names (zackees/ci.yml ci_lint/cache/families.py).
VIA_PREFIXES = {
    "setup-soldr:cargo-registry": "setup-soldr-cargoregistry-v1-",
    "setup-soldr:cook": "cook-base-v2-",
    "setup-soldr:build-cache": "setup-soldr-buildcache-v2-",
    "setup-soldr:solo-toolchain": "solo-toolchain-v3-",
    "setup-soldr:soldr-mini": "soldr-mini-v2-",
    "setup-uv": "setup-uv-2-",
}


def load() -> dict:
    return tomllib.loads(CI_TOML.read_text())


class UvCacheWriterTest(unittest.TestCase):
    def test_uv_remote_saves_are_master_only_and_restores_remain_enabled(self):
        files = sorted(WORKFLOWS.glob("*.y*ml")) + sorted(GITHUB.glob("actions/*/action.y*ml"))
        calls = 0
        for path in files:
            lines = path.read_text().splitlines()
            for index, line in enumerate(lines):
                if not re.search(r"uses: astral-sh/setup-uv@", line):
                    continue
                calls += 1
                # A setup action's inputs end at the next step or job.
                indent = len(line) - len(line.lstrip())
                end = index + 1
                while end < len(lines):
                    candidate = lines[end]
                    if candidate.strip() and not candidate.lstrip().startswith("#"):
                        depth = len(candidate) - len(candidate.lstrip())
                        if depth < indent or (depth == indent and candidate.lstrip().startswith("- ")):
                            break
                    end += 1
                block = "\n".join(lines[index:end])
                with self.subTest(path=path.relative_to(ROOT), line=index + 1):
                    self.assertIn("save-cache: ${{ github.ref == 'refs/heads/master' }}", block)
                    # Tier 2 containers already disable uv caching entirely;
                    # preserve that independent setting. Enabled consumers
                    # retain setup-uv's default restore behavior.
                    self.assertNotRegex(block, r"restore-cache:\s*['\"]?false")
        self.assertGreater(calls, 0)


class FlatpakCacheWriterTest(unittest.TestCase):
    def test_dependency_saves_are_master_only_without_disabling_compiler_cache(self):
        workflow = (WORKFLOWS / "build-project.yaml").read_text()
        block = workflow.split("      - name: Build Flatpak Manifest", 1)[1].split("      # The SDK", 1)[0]
        # v6.8 adds a save switch independent of cache/restore. v6.7
        # ignores this input, so the pin is part of the writer contract.
        self.assertIn("flatpak-builder@79327416609af08178ad73b352877e51450790b3", block)
        self.assertIn("save-cache: ${{ github.ref == 'refs/heads/master' && steps.flatpak-builder-cache-lookup.outputs.cache-hit != 'true' }}", block)
        self.assertRegex(block, r"(?m)^          cache: true$")
        # The dependency snapshot must be extracted before the newer compiler
        # snapshot. A second restore here overwrites mutable ccache manifests.
        self.assertRegex(block, r"(?m)^          restore-cache: false$")
        dependency = workflow.split("id: flatpak-builder-cache-lookup", 1)[1].split("\n      - ", 1)[0]
        self.assertNotIn("lookup-only: true", dependency)
        self.assertIn("path: .flatpak-builder", dependency)
        self.assertLess(workflow.index("id: flatpak-builder-cache-lookup"), workflow.index("id: flatpak-compiler-cache"))

    def test_separate_compiler_snapshot_saves_only_on_master(self):
        workflow = (WORKFLOWS / "build-project.yaml").read_text()
        marker = "key: flatpak-ccache-x86_64-${{ github.sha }}"
        save = workflow[workflow.index(marker) + len(marker):].split(marker, 1)[0]
        save = save[save.rfind("      - uses: actions/cache/save@"):]
        self.assertIn("github.ref == 'refs/heads/master'", save)
        self.assertIn("steps.ccache-stats.outcome == 'success'", save)
        self.assertIn("steps.flatpak-compiler-cache.outputs.cache-hit != 'true'", save)


class RetiredCacheFamilyTest(unittest.TestCase):
    def test_pre_zccache_reservations_have_no_remaining_consumer(self):
        families = load()["cache"]["family"]
        keys = cache_keys()
        for name, prefix in (("tier2-ccache", "tier2-ccache-linux-"),
                             ("appimage-apt", "apt-appimage-"),
                             ("appimage-ccache", "ccache-appimage-")):
            with self.subTest(family=name):
                self.assertFalse([path for path, key in keys if key.startswith(prefix)],
                                 "retire a family only after its consumers are removed")
                self.assertNotIn(name, families, "obsolete reservation must not consume declared capacity")


class CacheWriterPolicyTest(unittest.TestCase):
    @staticmethod
    def blocks(path):
        lines = path.read_text().splitlines()
        starts = [i for i, line in enumerate(lines) if re.match(r"\s+- ", line)] + [len(lines)]
        return [(start + 1, "\n".join(lines[start:end])) for start, end in zip(starts, starts[1:])]

    def test_owned_cache_steps_restore_independently_and_publish_only_on_master(self):
        files = sorted(WORKFLOWS.glob("*.y*ml")) + sorted(GITHUB.glob("actions/*/action.y*ml"))
        writers = 0
        for path in files:
            for line, block in self.blocks(path):
                if re.search(r"uses: actions/cache@", block):
                    self.fail(f"{path.relative_to(ROOT)}:{line}: separate restore/save to control publication")
                if not re.search(r"uses: actions/cache/save@", block):
                    continue
                writers += 1
                with self.subTest(path=path.relative_to(ROOT), line=line):
                    condition = re.search(r"(?m)^\s+if:\s*(.+)$", block)
                    self.assertIsNotNone(condition, "cache publication must have an explicit guard")
                    self.assertIn("github.ref == 'refs/heads/master'", condition[1])
        self.assertGreater(writers, 0)

    def test_split_dependency_and_timestamp_caches_preserve_keys_and_paths(self):
        cases = [(WORKFLOWS / "build-project.yaml", key)
                 for key in ("libs-qt-", "libs-ffmpeg-", "vcpkg-bin-cmocka-")]
        cases.append((GITHUB / "actions/services-validator/action.yaml", "service-check-"))
        for path, prefix in cases:
            with self.subTest(prefix=prefix):
                selected = [block for _, block in self.blocks(path)
                            if re.search(rf"(?m)^\s+key: {re.escape(prefix)}", block)]
                restore = [block for block in selected if "uses: actions/cache/restore@" in block]
                save = [block for block in selected if "uses: actions/cache/save@" in block]
                self.assertEqual(len(restore), 1)
                self.assertEqual(len(save), 1)
                for field in ("path", "key"):
                    pattern = rf"(?m)^\s+{field}: (.+)$"
                    self.assertEqual(re.search(pattern, restore[0])[1], re.search(pattern, save[0])[1])
                self.assertIn("outputs.cache-hit != 'true'", save[0])


def family_prefixes(ci: dict) -> list[str]:
    out = []
    for fam in ci["cache"]["family"].values():
        out.append(fam["prefix"] if "prefix" in fam else VIA_PREFIXES[fam["via"]])
    return out


def runner_os(lines: list[str], index: int) -> str:
    """runner.os for the job around line `index` (its nearest runs-on)."""
    for line in reversed(lines[:index]):
        m = re.match(r"\s+runs-on: (.*)", line)
        if m:
            label = m.group(1)
            return "macOS" if "macos" in label else "Windows" if "windows" in label else "Linux"
    return "Linux"


def cache_keys() -> list[tuple[str, str]]:
    """(file, literal key prefix) of every actions/cache key and restore key,
    and of every flatpak-builder cache key the workflows build."""
    out = []
    files = sorted(WORKFLOWS.glob("*.y*ml")) + sorted(GITHUB.glob("actions/*/action.y*ml"))
    for path in files:
        lines = path.read_text().splitlines()
        starts = [i for i, line in enumerate(lines) if re.match(r"\s+- ", line)] + [len(lines)]
        for start, end in zip(starts, starts[1:]):
            step = lines[start:end]
            values = []
            if any(re.search(r"uses: actions/cache(/restore|/save)?@", line) for line in step):
                for k, line in enumerate(step):
                    m = re.match(r"\s+(key|restore-keys): *(.*)", line)
                    if not m:
                        continue
                    if m.group(2) in ("|", ">"):
                        for s in step[k + 1 :]:
                            if not s.strip() or s.strip().startswith("#") or re.match(r"\s+[a-z-]+:", s):
                                break
                            values.append(s.strip())
                    else:
                        values.append(m.group(2))
            values += [m.group(1) for line in step for m in [re.search(r"cache_key='([^']*)'", line)] if m]
            for value in values:
                value = value.replace("${{ runner.os }}", runner_os(lines, start))
                out.append((path.relative_to(ROOT).as_posix(), value.split("${{")[0]))
    return out


def triggers(path: Path) -> list[str]:
    text = path.read_text()
    m = re.search(r"^(?:on|'on'|\"on\"):\n((?:[ \t]+.*\n|\n)*)", text, re.M)
    assert m, f"{path.name} has no on: block"
    return re.findall(r"^  ([a-z_]+):", m.group(1), re.M)


class CiTomlTest(unittest.TestCase):
    def test_disabled_store_has_no_retention_protection(self):
        prefixes = family_prefixes(load())
        for platform in ("linux-x64", "macos-arm64", "windows-x64"):
            key = f"setup-soldr-buildcache-v2-{platform}-legacy"
            with self.subTest(key=key):
                self.assertFalse(any(key.startswith(prefix) for prefix in prefixes),
                                 "Disabled archives must be eligible for unmatched-family cleanup")
        for key in ("cook-base-v2-linux-x64-current", "setup-soldr-cargoregistry-v1-linux-current",
                    "solo-toolchain-v3-linux-current", "zccache-tier2-linux-current",
                    "targets-ccache-linux-current", "targets-apt-current"):
            with self.subTest(key=key):
                self.assertTrue(any(key.startswith(prefix) for prefix in prefixes),
                                "Preserve useful dependency and native cache families")

    def test_schema_and_pin(self):
        ci = load()
        self.assertEqual(ci["schema"], 3)
        self.assertEqual(ci["linter"], f"zackees/ci.yml@{PIN}")
        self.assertIn(PIN, (ROOT / "local-gate.toml").read_text())

    def test_every_cache_key_has_a_family(self):
        prefixes = family_prefixes(load())
        keys = cache_keys()
        self.assertGreater(len(keys), 10)
        for path, key in keys:
            with self.subTest(path=path, key=key):
                self.assertTrue(key, "a cache key must start with a literal prefix")
                self.assertTrue(any(key.startswith(p) for p in prefixes), f"no [cache.family] prefix matches {key!r}")

    def test_setup_soldr_and_uv_families_declared(self):
        vias = {fam.get("via") for fam in load()["cache"]["family"].values()}
        # build-cache is optional and disabled at every audited callsite;
        # test_rust_cache_policy checks both that input and its retirement.
        required = set(VIA_PREFIXES) - {"setup-soldr:build-cache"}
        self.assertLessEqual(required, vias)

    def test_workflows_declare_their_triggers(self):
        allowed = load()["allow"]["workflows"]
        files = {p.name: p for p in WORKFLOWS.glob("*.y*ml")}
        self.assertEqual(set(allowed), set(files))
        for name, path in files.items():
            with self.subTest(workflow=name):
                self.assertEqual(sorted(set(triggers(path))), sorted(set(t.split(":")[0] for t in allowed[name])))


if __name__ == "__main__":
    unittest.main()
