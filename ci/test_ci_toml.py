#!/usr/bin/env -S uv run --script
"""ci.toml declares every cache family, workflow trigger and test binary (#957).

    uv run python -m unittest ci/test_ci_toml.py

ci.toml is the zackees/ci.yml schema 3 contract. The cache janitor
(.github/workflows/ci-pre.yml) deletes every Actions cache whose key matches
no [cache.family], so a cache key added to a workflow without a family would
be deleted on the next PR run. [allow].workflows must list each workflow with
its exact triggers (GEN-008). `ci-lint precheck` checks the rest; it needs the
network, so this offline test covers the three things a workflow, manifest or
test-binary edit can break.
"""

from __future__ import annotations

import importlib.util
import re
import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CI_TOML = ROOT / "ci.toml"
GITHUB = ROOT / ".github"
WORKFLOWS = GITHUB / "workflows"
PIN = "a1917ce76598904ce515cd7eec83450c317ea6f2"

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


def cargo_test_binaries() -> set[str]:
    """Every test binary Cargo builds, named as `[rust.tests].binaries` names it.

    Uses ci/test_binary_cap.py's manifest discovery, the same inventory the
    testing policy's category map is checked against, so ci.toml's budget and
    the category binaries cannot drift apart.
    """
    spec = importlib.util.spec_from_file_location(
        "test_binary_cap", Path(__file__).with_name("test_binary_cap.py")
    )
    cap = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(cap)
    found = set()
    for package, name, kinds in cap.workspace_test_targets():
        kinds = kinds.split(",")
        if "test" in kinds:
            found.add(f"{package}:test:{name}")
        elif "bin" in kinds:
            found.add(f"{package}:bin:{name}")
        else:
            found.add(f"{package}:lib")
    return found


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
    def test_projected_state_publishes_once_after_pruning_and_validators(self):
        workflow = (WORKFLOWS / "build-project.yaml").read_text()
        job = workflow.split("  flatpak-build:\n", 1)[1].split("  flatpak-compiler-consumer:", 1)[0]
        action = job.split("      - name: Build Flatpak Manifest", 1)[1].split("      - name: Report Flatpak", 1)[0]
        self.assertIn("flatpak-builder@79327416609af08178ad73b352877e51450790b3", action)
        self.assertRegex(action, r"(?m)^          cache: true$")
        self.assertRegex(action, r"(?m)^          restore-cache: false$")
        self.assertRegex(action, r"(?m)^          save-cache: false$")
        publisher = job.split('      - name: Publish Flatpak Projected State', 1)[1].split('\n      - ', 1)[0]
        for guard in ['!inputs.flatpak-cold-build', "github.ref == 'refs/heads/master'",
                      "steps.ccache-stats.outcome == 'success'", "steps.flatpak-state-cache.outputs.cache-hit != 'true'"]:
            self.assertIn(guard, publisher)
        paths = publisher.split('          path: |\n', 1)[1].split('          key:', 1)[0]
        self.assertEqual(paths.split(), ['.flatpak-builder/cache', '.flatpak-builder/ccache'])
        for marker in ['name: Ccache Stats', 'name: Validate AppStream', 'name: Validate build directory', 'name: Validate repository']:
            self.assertLess(job.index(marker), job.index('name: Publish Flatpak Projected State'))
        self.assertEqual(job.count('uses: actions/cache/save@'), 1)

    def test_projected_state_has_no_legacy_overlay(self):
        workflow = (WORKFLOWS / "build-project.yaml").read_text()
        job = workflow.split("  flatpak-build:\n", 1)[1].split("  flatpak-compiler-consumer:", 1)[0]
        state = job.split('id: flatpak-state-cache', 1)[1].split('\n      - ', 1)[0]
        key = "flatpak-builder-state-v1-x86_64-${{ hashFiles('build-aux/io.github.obs_rust.OBSStudioRust.json') }}-${{ github.sha }}"
        self.assertIn('key: ' + key, state)
        self.assertIn('flatpak-builder-state-v1-x86_64-', state)
        for marker in ['id: flatpak-builder-cache-lookup', 'name: Restore legacy Flatpak builder seed', 'id: flatpak-compiler-cache']:
            self.assertFalse(marker in job, 'The observed projected/cold routes no longer need migration overlays')
        publisher = job.split('name: Publish Flatpak Projected State', 1)[1].split('\n      - ', 1)[0]
        self.assertIn('key: ' + key, publisher)

    def test_flatpak_compiler_statistics_cover_only_this_build(self):
        workflow = (WORKFLOWS / "build-project.yaml").read_text()
        job = workflow.split("  flatpak-build:\n", 1)[1].split("\n  windows-build:", 1)[0]
        marker = "- name: Reset Flatpak Compiler Counters"
        self.assertIn(marker, job)
        reset = job.split(marker, 1)[1].split("\n      - ", 1)[0]
        self.assertIn("--zero-stats", reset)
        self.assertIn('CCACHE_DIR=', reset)
        self.assertNotIn('continue-on-error', reset)
        self.assertLess(job.index('id: flatpak-state-cache'), job.index(marker))
        self.assertLess(job.index(marker), job.index('- name: Build Flatpak Manifest'))
        self.assertIn('Flatpak Ccache Stats (this build)', job)

    def test_manual_cold_flatpak_neither_restores_nor_publishes_build_caches(self):
        workflow = (WORKFLOWS / "build-project.yaml").read_text()
        caller = (WORKFLOWS / "ci.yml").read_text()
        for text in (workflow, caller):
            self.assertRegex(text, r'flatpak-cold-build:\n        description: [^\n]+\n        type: boolean\n        default: false')
        self.assertIn("flatpak-cold-build: ${{ github.event_name == 'workflow_dispatch' && inputs.flatpak-cold-build }}", caller)
        job = workflow.split("  flatpak-build:\n", 1)[1].split("\n  windows-build:", 1)[0]
        steps = job.split('\n      - ')
        for marker in ('id: flatpak-state-cache',):
            matches = [step for step in steps if marker in step]
            self.assertTrue(matches, marker)
            for step in matches:
                self.assertIn('!inputs.flatpak-cold-build', step, marker)
        self.assertIn('name: Publish Flatpak Projected State', job)
        self.assertIn('save-cache: false', job)
        self.assertIn('cache: true', job)
        self.assertIn('restore-cache: false', job)
        for validator in ('Validate AppStream', 'Validate build directory', 'Validate repository'):
            self.assertIn(validator, job)

    def test_retired_standalone_compiler_cache_has_no_reader_writer_or_reservation(self):
        for workflow in WORKFLOWS.glob('*.y*ml'):
            with self.subTest(workflow=workflow.name):
                self.assertFalse('flatpak-ccache-' in workflow.read_text(), 'Retired standalone cache still has a consumer')
        self.assertFalse('flatpak-ccache' in load()['cache']['family'], 'Retired standalone reservation remains')


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


class CacheEntryBoundTest(unittest.TestCase):
    """A family whose single entry is large says how many entries may exist.

    `max` sizes one entry, so on its own it cannot stop a family from growing
    with every commit that writes it. `evict = "lru"` is what lets the janitor
    keep the family at `max` x cardinality (ci-lint CACHE-029).
    """

    def test_large_families_bound_their_entry_count(self):
        unbounded = [name for name, family in load()["cache"]["family"].items()
                     if size_bytes(family["max"]) >= 256 * 1024 ** 2 and family.get("evict") != "lru"]
        self.assertEqual(unbounded, [], "declare evict = 'lru' on a family this large")

    def test_size_parser(self):
        for text, expected in (("300MB", 300 * 1024 ** 2), ("1.5GB", int(1.5 * 1024 ** 3)), ("10GB", 10 * 1024 ** 3)):
            with self.subTest(text=text):
                self.assertEqual(size_bytes(text), expected)


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


def size_bytes(value: str) -> int:
    """A `[cache]` size string ("300MB", "1.5GB") in bytes."""
    match = re.fullmatch(r"([0-9.]+)(MB|GB)", value)
    assert match, f"unparseable size {value!r}"
    return int(float(match[1]) * 1024 ** (2 if match[2] == "MB" else 3))


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


def native_key_variants(value: str, lines: list[str], index: int) -> list[str]:
    """Resolve actual native matrix axes, including release include rows."""
    if not value.startswith(('windows-ccache-', 'macOS-xcode-')):
        return [value]
    axis = re.search(r'\$\{\{ matrix\.(architecture|target) \}\}', value)
    if not axis:
        return [value]
    jobs = [i for i, line in enumerate(lines) if re.match(r'^  [a-zA-Z0-9_-]+:', line)]
    start = max(i for i in jobs if i <= index)
    end = next((i for i in jobs if i > index), len(lines))
    job = '\n'.join(lines[start:end])
    inline = re.search(r'(?m)^\s+' + axis[1] + r': \[([^\]]+)\]', job)
    if inline:
        variants = re.findall(r'[\w-]+', inline[1])
    else:
        platform = 'windows' if value.startswith('windows-ccache-') else 'macos'
        rows = re.findall(r'(?ms)^          - os: ([^\n]+)\n(.*?)(?=^          - os:|\Z)', job)
        variants = [re.search(r'(?m)^\s+target: ([\w-]+)$', row)[1]
                    for os_name, row in rows if os_name.strip() == platform]
    if not variants:
        raise AssertionError('unresolved native matrix axis: ' + value)
    return [value.replace(axis[0], variant) for variant in variants]


def step_cache_values(step: list[str]) -> list[str]:
    """The key and restore-key values of one actions/cache step, plus any
    flatpak-builder cache_key it builds."""
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
    return values + [m.group(1) for line in step for m in [re.search(r"cache_key='([^']*)'", line)] if m]


def cache_keys() -> list[tuple[str, str]]:
    """(file, literal key prefix) of every actions/cache key and restore key,
    and of every flatpak-builder cache key the workflows build."""
    out = []
    files = sorted(WORKFLOWS.glob("*.y*ml")) + sorted(GITHUB.glob("actions/*/action.y*ml"))
    for path in files:
        lines = path.read_text().splitlines()
        starts = [i for i, line in enumerate(lines) if re.match(r"\s+- ", line)] + [len(lines)]
        for start, end in zip(starts, starts[1:]):
            for value in step_cache_values(lines[start:end]):
                value = value.replace("${{ runner.os }}", runner_os(lines, start))
                for variant in native_key_variants(value, lines, start):
                    out.append((path.relative_to(ROOT).as_posix(), variant.split("${{")[0]))
    return out


def job_permission_scopes(path: Path) -> set[tuple[str, str]]:
    """(job id, 'scope: access') of every job-level permissions block."""
    lines = path.read_text().splitlines()
    out: set[tuple[str, str]] = set()
    job = ""
    for index, line in enumerate(lines):
        header = re.match(r"^  ([A-Za-z0-9_-]+):\s*$", line)
        if header:
            job = header.group(1)
            continue
        if not re.match(r"^    permissions:\s*$", line):
            continue
        for entry in lines[index + 1 :]:
            granted = re.match(r"^      ([a-z-]+): (read|write)\s*$", entry)
            if not granted:
                break
            out.add((job, f"{granted.group(1)}: {granted.group(2)}"))
    return out


def triggers(path: Path) -> list[str]:
    text = path.read_text()
    m = re.search(r"^(?:on|'on'|\"on\"):\n((?:[ \t]+.*\n|\n)*)", text, re.M)
    assert m, f"{path.name} has no on: block"
    return re.findall(r"^  ([a-z_]+):", m.group(1), re.M)


class CiTomlTest(unittest.TestCase):
    def test_declared_test_binaries_are_the_workspace_test_targets(self):
        """[rust.tests].binaries is the test-binary budget (#962, RUST-005).

        The testing policy allows one amalgamated test binary per category, so
        a new integration-test target is a deliberate edit to ci.toml rather
        than a binary that appears unnoticed. ci-lint needs the network, so
        the list is checked here against the Cargo manifests instead.
        """
        declared = set(load().get("rust", {}).get("tests", {}).get("binaries", []))
        self.assertEqual(declared, cargo_test_binaries())

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
                # A runtime-bound key (literal part then ${{ ... }}) binds to
                # its family at runtime; any family under the literal part may
                # claim it, as the janitor does with the actual key.
                self.assertTrue(any(key.startswith(p) or p.startswith(key) for p in prefixes),
                                f"no [cache.family] prefix matches {key!r}")

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


class AllowPermissionsTest(unittest.TestCase):
    """A job's permission scope is granted in ci.toml's [allow].permissions (SEC-002).

    release-rust.yaml's jobs ask for contents: write. The publishers (release,
    certify, nightly_feed) write the release itself, and appimage/gate have to
    grant it on to the reusable workflows they call: a called workflow may not
    ask for more than its caller grants, and granting less made GitHub refuse
    to start the whole run (startup_failure, appimage's comment). ci-lint
    checks the grant with the network; this offline half keeps the workflow
    and ci.toml from drifting apart.
    """

    def test_release_rust_job_scopes_are_granted(self):
        # [allow].permissions records the elevated scopes a job may hold; a
        # read scope is inside the default grant and ci-lint does not ask for
        # it (SEC-002 reports the ': write' ones).
        scopes = {entry for entry in job_permission_scopes(WORKFLOWS / "release-rust.yaml")
                  if entry[1].endswith(": write")}
        grants = load()["allow"]["permissions"]
        self.assertTrue(scopes, "release-rust.yaml's jobs declare permission scopes")
        for job, scope in sorted(scopes):
            with self.subTest(job=job, scope=scope):
                self.assertIn(job, grants.get(scope, []), f"{scope} is not granted to job {job}")

    def test_release_rust_has_no_sec_002_exception(self):
        stale = [entry for entry in load()["exceptions"]
                 if entry["rule"] == "SEC-002" and entry["path"] == ".github/workflows/release-rust.yaml"]
        self.assertEqual(stale, [], "the grants replace this exception (#962)")

    def test_claim_bot_job_scopes_are_granted(self):
        # claim-bot.yaml's only job calls the org claim bot, which assigns the
        # issue and reads its pull requests. Both scopes sit on that job rather
        # than the top-level block, which asked for more than the repo max
        # (contents: read, actions: read) and so tripped SEC-002.
        scopes = {entry for entry in job_permission_scopes(WORKFLOWS / "claim-bot.yaml")
                  if entry[1].endswith(": write")}
        grants = load()["allow"]["permissions"]
        self.assertTrue(scopes, "claim-bot.yaml's job declares its permission scopes")
        for job, scope in sorted(scopes):
            with self.subTest(job=job, scope=scope):
                self.assertIn(job, grants.get(scope, []), f"{scope} is not granted to job {job}")

    def test_build_project_job_scopes_are_granted(self):
        """build-project.yaml's native-cache-transport job writes caches (SEC-002).

        ci-lint checks the grant with the network; this offline half keeps the
        workflow and ci.toml from drifting apart.
        """
        scopes = {entry for entry in job_permission_scopes(WORKFLOWS / "build-project.yaml")
                  if entry[1].endswith(": write")}
        grants = load()["allow"]["permissions"]
        self.assertEqual(scopes, {("native-cache-transport", "contents: write")},
                         "native-cache-transport's contents: write is build-project.yaml's only elevated scope")
        for job, scope in sorted(scopes):
            with self.subTest(job=job, scope=scope):
                self.assertIn(job, grants.get(scope, []), f"{scope} is not granted to job {job}")

    def test_claim_bot_has_no_sec_002_exception(self):
        stale = [entry for entry in load()["exceptions"]
                 if entry["rule"] == "SEC-002" and entry["path"] == ".github/workflows/claim-bot.yaml"]
        self.assertEqual(stale, [], "the job-level scopes replace this exception (#962)")

    def test_build_project_has_no_sec_002_exception(self):
        stale = [entry for entry in load()["exceptions"]
                 if entry["rule"] == "SEC-002"
                 and entry["path"] == ".github/workflows/build-project.yaml"]
        self.assertEqual(stale, [], "the grants replace this exception (#962)")

    def test_ci_yml_job_write_grants_are_recorded_not_excepted(self):
        """ci.yml's job write grants live in [allow].permissions (#962, SEC-002).

        ci.yml's job-level write grants are real (its jobs cancel superseded
        runs, write caches and write check results), so SEC-002's first fix
        applies: record each ci.yml job id under its scope in ci.toml's
        [allow].permissions and drop that file's SEC-002 exception. This pins
        both halves so ci.yml cannot silently re-enter the baseline.
        """
        writes = {entry for entry in job_permission_scopes(WORKFLOWS / "ci.yml")
                  if entry[1].endswith(": write")}
        self.assertEqual(
            writes,
            {
                ("cache-janitor", "actions: write"),
                ("cancel-superseded", "actions: write"),
                ("ci-pre", "actions: write"),
                ("build-project", "contents: write"),
                ("validate-data", "checks: write"),
            },
            "ci.yml's job-level write scopes")
        grants = load()["allow"]["permissions"]
        for job, scope in sorted(writes):
            with self.subTest(job=job, scope=scope):
                self.assertIn(job, grants.get(scope, []), f"{scope} is not granted to job {job}")
        stale = [entry for entry in load()["exceptions"]
                 if entry["rule"] == "SEC-002" and entry["path"] == ".github/workflows/ci.yml"]
        self.assertEqual(stale, [], "ci.yml has no SEC-002 finding left to except")


class NativeArchitectureRetentionTest(unittest.TestCase):
    def test_current_architecture_seeds_have_independent_honest_allowances(self):
        # Real observed producer archives; a newer other architecture cannot
        # substitute for any of these seeds (#1269).
        observed = {
            'windows-ccache-x64-RelWithDebInfo-source': 1063380057,
            'windows-ccache-arm64-RelWithDebInfo-source': 901055569,
            'macOS-xcode-x86_64-RelWithDebInfo-source': 675951544,
            'macOS-xcode-arm64-RelWithDebInfo-source': 659815401,
        }
        families = load()['cache']['family']
        matched = []
        for key, archive_bytes in observed.items():
            family_id = next(name for name, value in families.items()
                             if key.startswith(value.get('prefix', VIA_PREFIXES.get(value.get('via'), '\0'))))
            matched.append(family_id)
            family = families[family_id]
            unit = re.fullmatch(r'([0-9.]+)(MB|GB)', family['max'])
            allowance = int(float(unit[1]) * (1024 ** (2 if unit[2] == 'MB' else 3)))
            self.assertGreaterEqual(allowance, archive_bytes, key)
            self.assertEqual(family['evict'], 'lru')
            self.assertNotIn('shapes', family)
        self.assertEqual(len(set(matched)), 4, 'architectures must not share newest-entry protection')

    def test_declared_workflow_architectures_are_resolved_before_family_checks(self):
        keys = [key for _, key in cache_keys()]
        for prefix in ('windows-ccache-x64-', 'windows-ccache-arm64-',
                       'macOS-xcode-x86_64-', 'macOS-xcode-arm64-'):
            self.assertTrue(any(key.startswith(prefix) for key in keys), prefix)


class LeaderboardPermissionsTest(unittest.TestCase):
    """SEC-002: leaderboard.yml keeps its one write scope on the job that pushes.

    The `update` job commits generated data to the `leaderboard` branch, so it
    genuinely needs `contents: write`; the top-level block stays read-only so a
    change to the workflow cannot hand a write token to any other step. The job
    is recorded in ci.toml's [allow].permissions, and the file's SEC-002
    [[exceptions]] block is gone (#962). ci-lint needs the network, so the rule
    is mirrored here over the workflow text.
    """

    WORKFLOW = WORKFLOWS / "leaderboard.yml"

    def top_level_permissions(self) -> str:
        text = self.WORKFLOW.read_text(encoding="utf-8")
        block = re.search(r"(?m)^permissions:\n((?:[ \t].*\n|\n)*)", text)
        self.assertIsNotNone(block, "leaderboard.yml needs a top-level permissions block")
        assert block is not None
        return block.group(1)

    def test_top_level_permissions_are_read_only(self):
        body = self.top_level_permissions()
        self.assertRegex(body, r"(?m)^\s+contents: read$")
        self.assertNotRegex(body, r"write", "the top-level block must stay read-only")

    def test_update_job_grants_contents_write_and_is_allowlisted(self):
        text = self.WORKFLOW.read_text(encoding="utf-8")
        job = text.split("  update:\n", 1)[1]
        perms = re.search(r"(?m)^    permissions:\n((?:[ \t].*\n|\n)*)", job)
        self.assertIsNotNone(perms, "the update job must carry its own permissions block")
        assert perms is not None
        self.assertRegex(perms.group(1), r"(?m)^\s+contents: write$")
        allowed = load()["allow"]["permissions"].get("contents: write", [])
        self.assertIn("update", allowed)

    def test_no_sec002_exception_for_this_workflow(self):
        stale = [
            entry
            for entry in load().get("exceptions", [])
            if entry.get("rule") == "SEC-002"
            and entry.get("path") == ".github/workflows/leaderboard.yml"
        ]
        self.assertEqual(stale, [], "the SEC-002 exception for leaderboard.yml must be deleted")


class SignWindowsPermissionsTest(unittest.TestCase):
    """SEC-002: sign-windows.yaml declares a read-only top-level token default.

    The reusable workflow carried no top-level ``permissions`` block, so GitHub
    applied the wider repository default to it. The block is now read-only; the
    caller that invokes the workflow supplies the signing job's token scopes and
    a caller's grant is a ceiling (build-project.yaml's comment). The file's
    SEC-002 [[exceptions]] block is deleted (#962). ci-lint needs the network, so
    the rule is mirrored here over the workflow text and ci.toml.
    """

    WORKFLOW = WORKFLOWS / "sign-windows.yaml"

    def test_top_level_permissions_are_read_only(self):
        text = self.WORKFLOW.read_text(encoding="utf-8")
        block = re.search(r"(?m)^permissions:\n((?:[ \t].*\n|\n)*)", text)
        self.assertIsNotNone(block, "sign-windows.yaml needs a top-level permissions block")
        assert block is not None
        self.assertRegex(block.group(1), r"(?m)^\s+contents: read$")
        self.assertNotRegex(block.group(1), r"write", "the top-level block must stay read-only")

    def test_no_sec002_exception_for_this_workflow(self):
        stale = [
            entry
            for entry in load().get("exceptions", [])
            if entry.get("rule") == "SEC-002"
            and entry.get("path") == ".github/workflows/sign-windows.yaml"
        ]
        self.assertEqual(stale, [], "the SEC-002 exception for sign-windows.yaml must be deleted")


class DispatchPermissionsTest(unittest.TestCase):
    """SEC-002: dispatch.yaml keeps reads read-only and the one writer on its job.

    `services-validation` is the only job in dispatch.yaml that writes (it opens
    a PR), and it already carries its grants at job level; the top-level block
    stays read-only so a change to the workflow cannot hand a write token to any
    other job (they are all upstream-only, gated on `repository_owner ==
    'obsproject'`). The job is recorded in ci.toml's [allow].permissions, and the
    file's SEC-002 [[exceptions]] block is gone (#962). ci-lint needs the
    network, so the rule is mirrored here over the workflow text.
    """

    WORKFLOW = WORKFLOWS / "dispatch.yaml"
    JOB = "services-validation"

    def top_level_permissions(self) -> str:
        text = self.WORKFLOW.read_text(encoding="utf-8")
        block = re.search(r"(?m)^permissions:\n((?:[ \t].*\n|\n)*)", text)
        self.assertIsNotNone(block, "dispatch.yaml needs a top-level permissions block")
        assert block is not None
        return block.group(1)

    def test_top_level_permissions_are_read_only(self):
        body = self.top_level_permissions()
        self.assertRegex(body, r"(?m)^\s+contents: read$")
        self.assertNotRegex(body, r"write", "the top-level block must stay read-only")

    def test_writer_job_grants_are_allowlisted(self):
        text = self.WORKFLOW.read_text(encoding="utf-8")
        job = text.split(f"  {self.JOB}:\n", 1)[1]
        perms = re.search(r"(?m)^    permissions:\n((?:[ \t].*\n|\n)*)", job)
        self.assertIsNotNone(perms, "the services-validation job must carry its own permissions block")
        assert perms is not None
        for scope in ("checks: write", "contents: write", "pull-requests: write"):
            self.assertRegex(perms.group(1), rf"(?m)^\s+{re.escape(scope)}$")
            allowed = load()["allow"]["permissions"].get(scope, [])
            self.assertIn(self.JOB, allowed, f"{scope} must list {self.JOB}")

    def test_no_sec002_exception_for_this_workflow(self):
        stale = [
            entry
            for entry in load().get("exceptions", [])
            if entry.get("rule") == "SEC-002"
            and entry.get("path") == ".github/workflows/dispatch.yaml"
        ]
        self.assertEqual(stale, [], "the SEC-002 exception for dispatch.yaml must be deleted")


class ScheduledPermissionsTest(unittest.TestCase):
    """SEC-002: scheduled.yaml keeps its top level read-only and each writer's grant on its job.

    The top-level block was `contents: write`, which exceeds the read-only
    maximum a top-level block may hold. Every job that needs an elevated scope
    now mints it on its own block (`services-availability`, `build-project`,
    `report-nightly-failure`, `analyze-project`, `codeql`), each job id is
    recorded in ci.toml's [allow].permissions, and the file's SEC-002
    [[exceptions]] block is gone (#962). The two jobs without a permissions
    block (`upload-language-files`, `steam-upload`) are upstream-only, gated on
    `repository_owner == 'obsproject'`, and declare no write need. ci-lint needs
    the network, so the rule is mirrored here over the workflow text.
    """

    WORKFLOW = WORKFLOWS / "scheduled.yaml"
    # The scopes ci-lint asks a job to have allowlisted (its write scopes, plus
    # `packages: read`, which is not inside the default grant).
    JOB_SCOPES = {
        "services-availability": {"checks: write", "contents: write", "pull-requests: write"},
        "build-project": {"contents: write"},
        "report-nightly-failure": {"issues: write"},
        "analyze-project": {"security-events: write"},
        "codeql": {"security-events: write", "packages: read"},
    }

    def top_level_permissions(self) -> str:
        text = self.WORKFLOW.read_text(encoding="utf-8")
        block = re.search(r"(?m)^permissions:\n((?:[ \t].*\n|\n)*)", text)
        self.assertIsNotNone(block, "scheduled.yaml needs a top-level permissions block")
        assert block is not None
        return block.group(1)

    def test_top_level_permissions_are_read_only(self):
        body = self.top_level_permissions()
        self.assertRegex(body, r"(?m)^\s+contents: read$")
        self.assertNotRegex(body, r"write", "the top-level block must stay read-only")

    def test_writer_job_grants_are_allowlisted(self):
        scopes = job_permission_scopes(self.WORKFLOW)
        grants = load()["allow"]["permissions"]
        for job, required in sorted(self.JOB_SCOPES.items()):
            with self.subTest(job=job):
                for scope in sorted(required):
                    self.assertIn((job, scope), scopes, f"{job} must declare {scope}")
                    self.assertIn(job, grants.get(scope, []), f"{scope} must list {job}")

    def test_no_sec002_exception_for_this_workflow(self):
        stale = [
            entry
            for entry in load().get("exceptions", [])
            if entry.get("rule") == "SEC-002"
            and entry.get("path") == ".github/workflows/scheduled.yaml"
        ]
        self.assertEqual(stale, [], "the SEC-002 exception for scheduled.yaml must be deleted")


if __name__ == "__main__":
    unittest.main()
