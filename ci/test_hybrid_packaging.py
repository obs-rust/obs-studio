"""Hybrid packages keep offline inputs pinned and publish only after approval."""

import importlib.util
import json
import os
import re
import runpy
import shutil
import subprocess
import sys
import tempfile
import tomllib
import unittest
from unittest.mock import patch
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


class HybridPackagingTest(unittest.TestCase):
    def load_flatpak_cache_probe(self):
        spec = importlib.util.spec_from_file_location('flatpak_cache_probe', ROOT / 'ci/flatpak_cache_probe.py')
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module

    def flatpak_probe_fixture(self, root, fail=False):
        state = root / '.flatpak-builder'
        for name, size in [('downloads/archive', 100000), ('ccache/object', 50000), ('cache/object', 1000)]:
            path = state / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(os.urandom(size))
        manifest = root / 'manifest.json'
        manifest.write_text('{}')
        builder = root / 'flatpak-builder'
        builder.write_text('''#!/bin/sh
if [ "$1" = --version ]; then echo 'flatpak-builder fixture'; exit 0; fi
state=
while [ "$#" -gt 0 ]; do
  if [ "$1" = --state-dir ]; then shift; state=$1; fi
  shift
done
mkdir -p "$state/downloads"
printf reacquired > "$state/downloads/archive"
''' + ('exit 17\n' if fail else 'exit 0\n'))
        builder.chmod(0o755)
        temporary = root / 'temporary'
        temporary.mkdir()
        return state, manifest, builder, temporary

    def test_flatpak_capacity_probe_preserves_original_and_measures_exclusions(self):
        flatpak_cache_probe = self.load_flatpak_cache_probe()
        with tempfile.TemporaryDirectory() as directory:
            state, manifest, builder, temporary = self.flatpak_probe_fixture(Path(directory))
            before = {str(p): (p.read_bytes(), p.stat().st_mtime_ns)
                      for p in state.rglob('*') if p.is_file()}
            result = flatpak_cache_probe.measure(state, manifest, 'x86_64', builder, temporary)
            self.assertEqual(result['status'], 'success')
            self.assertGreaterEqual(result['downloads_apparent_bytes'], 100000)
            self.assertGreaterEqual(result['warm_download_seconds'], 0)
            self.assertGreaterEqual(result['fresh_download_seconds'], 0)
            sizes = result['compression']
            self.assertGreater(sizes['full']['bytes'], sizes['without_downloads']['bytes'])
            self.assertGreater(sizes['without_downloads']['bytes'], sizes['without_downloads_or_compiler']['bytes'])
            self.assertEqual(before, {str(p): (p.read_bytes(), p.stat().st_mtime_ns)
                                    for p in state.rglob('*') if p.is_file()})
            self.assertEqual(list(temporary.iterdir()), [])
            self.assertIn('proxy', result['limits'].lower())

    def test_flatpak_capacity_probe_failure_preserves_original_and_cleans_scratch(self):
        flatpak_cache_probe = self.load_flatpak_cache_probe()
        with tempfile.TemporaryDirectory() as directory:
            state, manifest, builder, temporary = self.flatpak_probe_fixture(Path(directory), fail=True)
            original = (state / 'downloads/archive').read_bytes()
            with self.assertRaises(subprocess.CalledProcessError):
                flatpak_cache_probe.measure(state, manifest, 'x86_64', builder, temporary)
            self.assertEqual((state / 'downloads/archive').read_bytes(), original)
            self.assertEqual(list(temporary.iterdir()), [])

    def test_flatpak_capacity_probe_rejects_links_and_scratch_inside_original(self):
        flatpak_cache_probe = self.load_flatpak_cache_probe()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            state, manifest, builder, temporary = self.flatpak_probe_fixture(root)
            with self.assertRaises(ValueError):
                flatpak_cache_probe.measure(state, manifest, 'x86_64', builder, state)
            external = root / 'external'
            external.write_text('preserve')
            (state / 'downloads/link').symlink_to(external)
            with self.assertRaises(ValueError):
                flatpak_cache_probe.measure(state, manifest, 'x86_64', builder, temporary)
            self.assertEqual(external.read_text(), 'preserve')
            self.assertEqual(list(temporary.iterdir()), [])

    def test_flatpak_capacity_probe_is_default_off_and_keeps_full_validation(self):
        pull = (ROOT / '.github/workflows/ci.yml').read_text()
        build = (ROOT / '.github/workflows/build-project.yaml').read_text()
        for text in (pull, build):
            self.assertIn('flatpak-cache-probe:', text)
            block = re.search(r'(?m)^      flatpak-cache-probe:\n((?:        .*\n)+)', text)[1]
            self.assertIn('type: boolean', block)
            self.assertIn('default: false', block)
        self.assertIn("github.event_name == 'workflow_dispatch' && inputs.flatpak-cache-probe", pull)
        job = build.split('  flatpak-build:\n', 1)[1].split('\n  windows-build:', 1)[0]
        marker = '- name: Measure Flatpak Cache Capacity'
        self.assertIn(marker, job)
        self.assertLess(job.index('- name: Validate repository'), job.index(marker))
        probe = job.split(marker, 1)[1].split('\n      - ', 1)[0]
        self.assertIn('inputs.flatpak-cache-probe', probe)
        self.assertIn('uv run ci/flatpak_cache_probe.py', probe)
        self.assertNotIn('continue-on-error', probe)

    def test_windows_nvenc_reference_has_separate_linker_outputs(self):
        compiler = shutil.which("cc")
        if not compiler or not shutil.which("ninja"):
            self.skipTest("CMake generation fixture needs cc and Ninja")
        source = (ROOT / "plugins/obs-nvenc/CMakeLists.txt").read_text()
        properties = re.search(r"set_target_properties\(\s*obs-nvenc-c-reference\s+PROPERTIES.*?\n\s*\)", source, re.S)[0]
        for generator in ("Ninja", "Ninja Multi-Config"):
            with self.subTest(generator=generator), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                build = root / "build"
                (root / "empty.c").write_text("void nvenc_fixture(void) {}\n")
                (root / "CMakeLists.txt").write_text('''cmake_minimum_required(VERSION 3.28)
project(nvenc_reference C)
add_library(obs-nvenc MODULE empty.c)
add_library(obs-nvenc-c-reference MODULE empty.c)
set_target_properties(obs-nvenc PROPERTIES PREFIX "")
''' + properties)
                result = subprocess.run(["cmake", "-S", directory, "-B", str(build), "-G", generator,
                                         "-DCMAKE_SYSTEM_NAME=Windows", "-DCMAKE_C_COMPILER=" + compiler,
                                         "-DCMAKE_C_COMPILER_FORCED=TRUE", "-DCMAKE_BUILD_TYPE=RelWithDebInfo"],
                                        capture_output=True, text=True)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                configs = ("Debug", "Release", "RelWithDebInfo") if generator.endswith("Multi-Config") else (None,)
                for config in configs:
                    rules = (build / "CMakeFiles" / f"impl-{config}.ninja" if config else build / "build.ninja").read_text()
                    imports = re.findall(r"^\s*TARGET_IMPLIB = (.+)$", rules, re.M)
                    self.assertEqual(len(imports), 2)
                    self.assertNotEqual(imports[0], imports[1], "production and reference import/export files collide")
                    self.assertNotIn("obs-nvenc-c-reference/", imports[0])
                    self.assertIn("obs-nvenc-c-reference/", imports[1])

    def test_flatpak_has_the_pinned_compiler_and_offline_cargo_inputs(self):
        manifest = json.loads((ROOT / "build-aux/com.obsproject.Studio.json").read_text())
        version = tomllib.loads((ROOT / "rust-toolchain.toml").read_text())["toolchain"]["channel"]
        compiler = next(module for module in manifest["modules"] if module["name"] == "obs-rust-toolchain")
        self.assertIn(f"rust-{version}-x86_64-unknown-linux-gnu.tar.xz", compiler["sources"][0]["url"])
        self.assertRegex(compiler["sources"][0]["sha256"], r"^[a-f0-9]{64}$")
        self.assertEqual(compiler["build-options"], {"strip": False, "no-debuginfo": True},
                         "Flatpak must preserve the prebuilt compiler's loadable ELF segments")
        obs = next(module for module in manifest["modules"] if module["name"] == "obs")
        self.assertEqual(obs["build-options"]["env"]["CARGO_NET_OFFLINE"], "true")
        self.assertIn("-DOBS_LIBOBS_BACKEND=HYBRID", obs["config-opts"])
        self.assertTrue(any(source.get("dest") == ".flatpak-rust/corrosion" and source.get("sha256") for source in obs["sources"]))
        workflow = (ROOT / ".github/workflows/build-project.yaml").read_text()
        self.assertLess(workflow.index("run: build-aux/prepare-flatpak-rust.sh"), workflow.index("- name: Build Flatpak Manifest"))

    def test_flatpak_keeps_debug_symbols_without_recompressing_elf(self):
        manifest = json.loads((ROOT / "build-aux/com.obsproject.Studio.json").read_text())
        obs = next(module for module in manifest["modules"] if module["name"] == "obs")
        self.assertTrue(obs["build-options"].get("no-debuginfo-compression", False),
                        "the OBS module must not repeat costly ELF debug compression before OSTree export (#1760)")
        for options in (manifest.get("build-options", {}), obs["build-options"]):
            self.assertFalse(options.get("no-debuginfo", False), "keep the separate Debug extension")
            self.assertFalse(options.get("strip", False), "do not replace debug extraction with destructive stripping")
        self.assertIn("-DCMAKE_BUILD_TYPE=RelWithDebInfo", obs["config-opts"])
        self.assertIn("-DENABLE_BROWSER=ON", obs["config-opts"])

    def test_flatpak_caches_fast_cef_debug_compression(self):
        manifest = json.loads((ROOT / "build-aux/com.obsproject.Studio.json").read_text())
        cef = next(module for module in manifest["modules"] if module["name"] == "cef")
        self.assertIn("eu-elfcompress -t zstd /app/cef/Release/libcef.so", cef["build-commands"],
                      "compress immutable CEF debug sections once in its cached dependency module (#1760)")
        self.assertTrue(cef["build-options"]["no-debuginfo"],
                        "the OBS install must still split CEF symbols at their installed runtime path")

    def test_flatpak_pch_cache_uses_launcher_and_preserves_input_freshness(self):
        manifest = json.loads((ROOT / "build-aux/com.obsproject.Studio.json").read_text())
        obs = next(module for module in manifest["modules"] if module["name"] == "obs")
        self.assertIn("-DENABLE_CCACHE=ON", obs["config-opts"],
                      "activate the existing PCH-aware CMake compiler launcher")
        self.assertIn("-DCMAKE_C_COMPILER=/usr/bin/cc", obs["config-opts"])
        self.assertIn("-DCMAKE_CXX_COMPILER=/usr/bin/c++", obs["config-opts"],
                      "avoid nesting the SDK masquerading wrapper inside the launcher")
        env = obs["build-options"]["env"]
        self.assertEqual(env.get("CCACHE_DEPEND"), "true",
                         "PCH creation must not reuse a preprocessor result that omits changed macro definitions")
        self.assertEqual(env.get("CCACHE_COMPILERCHECK"), "content",
                         "a compiler replacement with preserved size/mtime must invalidate cached objects")
        self.assertNotIn("CCACHE_SLOPPINESS", env,
                         "keep the existing PCH helper's bounded settings; do not weaken header freshness")

    def test_flatpak_pch_probe_preserves_failed_tool_diagnostics(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "evidence"
            script = ROOT / "ci/flatpak_pch_probe.py"
            error = subprocess.CalledProcessError(17, ["ccache", "--version"],
                                                  output="fixture SDK identity unavailable\n")
            with patch.object(sys, "argv", [str(script), "--output", str(output)]), \
                 patch("subprocess.check_output", side_effect=["configured", "fixture compiler", error]):
                with self.assertRaises(subprocess.CalledProcessError):
                    runpy.run_path(str(script), run_name="__main__")
            log = output / "failed-command.log"
            self.assertTrue(log.exists(), "preserve SDK tool stderr instead of only an opaque CalledProcessError")
            self.assertIn("fixture SDK identity unavailable", log.read_text())
            self.assertIn('"ccache", "--version"', log.read_text())

    def test_flatpak_custom_key_matches_pinned_action_architecture_fallback(self):
        workflow = (ROOT / ".github/workflows/build-project.yaml").read_text()
        job = workflow.split("  flatpak-build:\n", 1)[1].split("\n  windows-build:", 1)[0]
        template = re.search(r"cache_key='([^']*(?:'build-aux/com.obsproject.Studio.json')[^']*)'", job)[1]
        expression = "${{ hashFiles('build-aux/com.obsproject.Studio.json') }}"
        self.assertIn(expression, template, "preserve the original manifest identity")
        # v6.8 appends the architecture to custom keys and falls back to this
        # architecture-first prefix. Different manifests must find the same seed.
        seed = template.replace(expression, "a" * 64) + "-x86_64"
        changed = template.replace(expression, "b" * 64) + "-x86_64"
        self.assertNotEqual(seed, changed)
        self.assertTrue(seed.startswith("flatpak-builder-x86_64"), seed)
        self.assertTrue(changed.startswith("flatpak-builder-x86_64"), changed)
        self.assertFalse(seed.startswith("flatpak-builder-aarch64"))

    def test_flatpak_namespace_migration_preserves_legacy_seed_conditionally(self):
        workflow = (ROOT / ".github/workflows/build-project.yaml").read_text()
        job = workflow.split("  flatpak-build:\n", 1)[1].split("\n  windows-build:", 1)[0]
        self.assertIn("id: flatpak-builder-cache-lookup", job)
        lookup = job.split("id: flatpak-builder-cache-lookup", 1)[1].split("\n      - ", 1)[0]
        self.assertNotIn("lookup-only: true", lookup, "restore the builder before its newer standalone compiler cache")
        self.assertIn("key: flatpak-builder-x86_64-${{ hashFiles('build-aux/com.obsproject.Studio.json') }}-x86_64", lookup)
        self.assertIn("restore-keys: flatpak-builder-x86_64", lookup)
        marker = "- name: Restore legacy Flatpak builder seed"
        self.assertIn(marker, job)
        legacy = job.split(marker, 1)[1].split("\n      - ", 1)[0]
        self.assertIn("steps.flatpak-builder-cache-lookup.outputs.cache-matched-key == ''", legacy)
        self.assertIn("path: .flatpak-builder", legacy)
        self.assertIn("key: flatpak-builder-7bb71f0a48a885ca4bfe218d34814d0c108cb0ee80df664119c10c6a24945d77-x86_64", legacy)
        self.assertNotIn("restore-keys:", legacy, "legacy bridge must not select another architecture")
        self.assertLess(job.index(marker), job.index("id: flatpak-compiler-cache"))
        self.assertLess(job.index("id: flatpak-compiler-cache"), job.index("- name: Build Flatpak Manifest"))

    def flatpak_cache_inventory_script(self):
        workflow = (ROOT / ".github/workflows/build-project.yaml").read_text()
        job = workflow.split("  flatpak-build:\n", 1)[1].split("\n  windows-build:", 1)[0]
        marker = "- name: Report Flatpak Cache Contents"
        self.assertTrue(marker in job, "measure the actual builder state before proposing pruning")
        self.assertLess(job.index("- name: Build Flatpak Manifest"), job.index(marker))
        self.assertLess(job.index(marker), job.index("- name: Ccache Stats"))
        step = job.split(marker, 1)[1].split("\n      - ", 1)[0]
        body = re.search(r"        run: \|\n((?:          [^\n]*\n|\n)*)", step)[1]
        return "\n".join(line[10:] for line in body.splitlines())

    def test_flatpak_inventory_reports_bytes_without_mutation_or_symlink_escape(self):
        script = self.flatpak_cache_inventory_script()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            state = root / ".flatpak-builder"
            for name, size in (("downloads/input", 11), ("cache/objects/object", 17),
                               ("ccache/object", 23), ("build/obs/object", 31)):
                path = state / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(b"x" * size)
            outside = root / "outside"
            outside.mkdir()
            (outside / "payload").write_bytes(b"x" * 1_000_000)
            (state / "external").symlink_to(outside, target_is_directory=True)
            def snapshot():
                paths = sorted(str(path.relative_to(root)) for path in root.rglob("*"))
                files = {str(path.relative_to(root)): (path.read_bytes(), path.stat().st_mtime_ns)
                         for path in root.rglob("*") if path.is_file()}
                return paths, files
            before = snapshot()
            result = subprocess.run(["bash", "-euo", "pipefail", "-c", script], cwd=root,
                                    env={**os.environ, "JOB_STATUS": "success"}, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertEqual(snapshot(), before, "measurement must not prune or alter state")
            rows = {path: int(amount) for line in result.stdout.splitlines() if "\t" in line
                    for amount, path in [line.split("\t", 1)]}
            self.assertGreaterEqual(rows[".flatpak-builder/downloads"], 11)
            self.assertGreaterEqual(rows[".flatpak-builder/cache/objects"], 17)
            self.assertGreaterEqual(rows[".flatpak-builder/ccache"], 23)
            self.assertLess(rows[".flatpak-builder"], 1_000_000, "do not follow outside symlinks")
            self.assertIn("job_status=success units=apparent-bytes", result.stdout)

    def test_flatpak_inventory_marks_absent_state_without_creating_it(self):
        script = self.flatpak_cache_inventory_script()
        with tempfile.TemporaryDirectory() as directory:
            result = subprocess.run(["bash", "-euo", "pipefail", "-c", script], cwd=directory,
                                    env={**os.environ, "JOB_STATUS": "failure"}, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertIn("flatpak-cache state=absent job_status=failure", result.stdout)
            self.assertEqual(list(Path(directory).iterdir()), [])

    def test_every_publication_path_checks_hybrid_release_approval(self):
        for name, publish in (("release-rust.yaml", "- name: Create or repair release"),
                              ("appimage.yaml", "- name: Attach to the tag's release")):
            source = (ROOT / ".github/workflows" / name).read_text().split("\n  release:", 1)[1]
            with self.subTest(workflow=name):
                self.assertIn("ci/hybrid_release_gate.py", source)
                self.assertLess(source.index("ci/hybrid_release_gate.py"), source.index(publish))

    def test_macos_application_build_does_not_request_the_removed_twin(self):
        script = (ROOT / ".github/scripts/.build.zsh").read_text()
        self.assertNotIn("libobs-with-rust", script)
        self.assertNotIn("-target libobs-c-reference", script)

    def test_cold_vendor_bootstrap_output_cannot_contaminate_cargo_configuration(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "build-aux").mkdir()
            (root / "bin").mkdir()
            script = root / "build-aux/prepare-flatpak-rust.sh"
            shutil.copyfile(ROOT / "build-aux/prepare-flatpak-rust.sh", script)
            stub = root / "bin/soldr"
            stub.write_text('#!/bin/sh\ncat <<\'CONFIG\'\nRust is installed now. Great!\n[source.crates-io]\nreplace-with = "vendored-sources"\n\n[source.vendored-sources]\ndirectory = ".flatpak-rust/vendor"\nCONFIG\n')
            stub.chmod(0o755)
            result = subprocess.run(["bash", str(script)], env={**os.environ, "PATH": str(root / "bin") + os.pathsep + os.environ["PATH"]}, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            config = tomllib.loads((root / ".flatpak-rust/cargo/config.toml").read_text())
            self.assertEqual(config["source"]["vendored-sources"]["directory"], "/run/build/obs/.flatpak-rust/vendor")


class FlatpakPrunedBuildTest(unittest.TestCase):
    def load_probe(self):
        import importlib.util
        spec = importlib.util.spec_from_file_location('flatpak_pruned_build_probe', ROOT / 'ci/flatpak_pruned_build_probe.py')
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module

    def fixture(self, root):
        state = root / 'state'
        for name in ('cache', 'downloads', 'ccache'):
            (state / name).mkdir(parents=True)
            (state / name / 'original').write_text(name)
        manifest = root / 'manifest.json'
        manifest.write_text(json.dumps({'id': 'com.obsproject.Studio', 'command': 'obs', 'modules': [
            {'name': 'dependency'}, {'name': 'obs', 'post-install': ['existing-command']},
        ]}))
        # flatpak-builder --state-dir scopes refs to the absolute manifest path.
        from urllib.parse import quote
        branch = quote('x86_64-' + str(manifest.resolve())[1:], safe='').replace('%', '_')
        refs = state / 'cache/refs/heads' / branch
        refs.mkdir(parents=True)
        (refs / 'build-dependency').write_text('a' * 64 + '\n')
        return state, manifest

    def test_pruned_rebuild_isolated_and_validates_outputs(self):
        probe = self.load_probe()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            state, manifest = self.fixture(root)
            original = manifest.read_bytes()
            # Transient build trees are not dependency repository inputs.
            (state / 'build').mkdir()
            (state / 'build/transient-link').symlink_to(state / 'cache/original')
            objects = state / 'cache/objects/ab'
            objects.mkdir(parents=True)
            object_link = objects / ('1' * 62 + '.file')
            object_link.symlink_to('/app/lib/example.so')
            calls = []

            def execute(command, seconds):
                calls.append(command)
                if 'flatpak-builder' in command:
                    self.assertIn('--mirror-screenshots-url=https://dl.flathub.org/media', command)
                    self.assertIn('--compose-url-policy=full', command)
                    copied = Path(command[command.index('--state-dir') + 1])
                    self.assertNotEqual(copied, state)
                    self.assertTrue((copied / 'cache/original').exists())
                    self.assertEqual(sorted(path.name for path in copied.iterdir()), ['cache'])
                    copied_link = copied / object_link.relative_to(state)
                    self.assertTrue(copied_link.is_symlink())
                    self.assertEqual(os.readlink(copied_link), '/app/lib/example.so')
                    self.assertFalse((copied / 'downloads').exists())
                    self.assertFalse((copied / 'ccache').exists())
                    recipe = Path(command[-1])
                    from urllib.parse import quote
                    branch = quote('x86_64-' + str(recipe.resolve())[1:], safe='').replace('%', '_')
                    selected = copied / 'cache/refs/heads' / branch / 'build-dependency'
                    self.assertTrue(selected.is_file(), 'temporary manifest must select copied dependency refs')
                    self.assertEqual(selected.read_text(), 'a' * 64 + '\n')
                    self.assertEqual(recipe.parent, manifest.parent)
                    prepared = json.loads(recipe.read_text())
                    self.assertEqual(prepared['modules'][0], {'name': 'dependency'})
                    self.assertEqual(prepared['modules'][-1]['post-install'][0], 'existing-command')
                    build = Path(command[-2])
                    for path in ('bin/obs', 'share/app-info/icons/flatpak/128x128/com.obsproject.Studio.png',
                                 'share/app-info/xmls/com.obsproject.Studio.xml.gz'):
                        target = build / 'files' / path
                        target.parent.mkdir(parents=True, exist_ok=True)
                        target.write_text('fixture')
                    return 'Cache hit for dependency, skipping build\nOBS_PRUNED_REBUILD_EXECUTED\n'
                return '{}'

            result = probe.measure(state, manifest, root, execute=execute)
            self.assertEqual(result['status'], 'success')
            self.assertEqual(manifest.read_bytes(), original)
            self.assertEqual((state / 'cache/original').read_text(), 'cache')
            self.assertEqual(sorted(p.name for p in root.iterdir()), ['manifest.json', 'state'])
            lint_kinds = [command[-2] for command in calls if command[0] == 'flatpak-builder-lint']
            self.assertEqual(lint_kinds, ['appstream', 'builddir', 'repo'])
            fsck_index = next(i for i, command in enumerate(calls) if command[0] == 'ostree' and 'fsck' in command)
            builder_index = next(i for i, command in enumerate(calls) if 'flatpak-builder' in command)
            self.assertLess(fsck_index, builder_index)
            screenshots = next(command for command in calls if command[0] == 'ostree' and 'commit' in command)
            self.assertIn('--canonical-permissions', screenshots)
            self.assertIn('--branch=screenshots/x86_64', screenshots)
            self.assertTrue(screenshots[-1].endswith('/build/files/share/app-info/media'))
            self.assertLess(calls.index(screenshots), next(i for i, command in enumerate(calls) if command[0] == 'flatpak-builder-lint'))

    def test_compiler_probe_requires_substantial_reuse_after_forced_rebuild(self):
        probe = self.load_probe()
        for hits, misses, cached_app, succeeds in ((300, 2, False, True), (0, 330, False, False),
                                                  (5, 0, False, False), (100, 230, False, False),
                                                  (300, 2, True, False)):
            with self.subTest(hits=hits, misses=misses, cached_app=cached_app), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                state, manifest = self.fixture(root)
                recipe = json.loads(manifest.read_text())
                recipe.update(sdk='org.freedesktop.Sdk', **{'runtime-version': '26.08'})
                manifest.write_text(json.dumps(recipe))
                original = manifest.read_bytes()
                calls, builds = [], []
                def execute(command, seconds):
                    calls.append(command)
                    if 'flatpak-builder' in command:
                        isolated = Path(command[command.index('--state-dir') + 1])
                        prepared = json.loads(Path(command[-1]).read_text())
                        builds.append(prepared)
                        if len(builds) == 1:
                            self.assertFalse((isolated / 'ccache').exists())
                            (isolated / 'ccache').mkdir()
                            (isolated / 'ccache/fresh').write_text('new compiler results')
                        else:
                            self.assertTrue((isolated / 'ccache/fresh').exists())
                            self.assertFalse((isolated / 'ccache/original').exists())
                            self.assertNotEqual(prepared, builds[0], 'warm build must invalidate application module')
                        build = Path(command[-2])
                        for relative in ('bin/obs', 'share/app-info/icons/flatpak/128x128/com.obsproject.Studio.png',
                                         'share/app-info/xmls/com.obsproject.Studio.xml.gz'):
                            path = build / 'files' / relative
                            path.parent.mkdir(parents=True, exist_ok=True)
                            path.write_text('fixture')
                        if len(builds) == 2 and cached_app:
                            return 'Cache hit for obs, skipping build\n'
                        return ('Cache hit for dependency, skipping build\nOBS_PRUNED_REBUILD_EXECUTED\n'
                                + ('OBS_COMPILER_REUSE_EXECUTED\n' if len(builds) == 2 else ''))
                    if '--print-stats' in command:
                        return f'direct_cache_hit {hits}\npreprocessed_cache_hit 0\ncache_miss {misses}\n'
                    return '{}'
                if succeeds:
                    result = probe.measure(state, manifest, root, execute=execute, compiler_reuse=True)
                    self.assertEqual(result['compiler_reuse']['hits'], 300)
                    self.assertEqual(result['compiler_reuse']['misses'], 2)
                    self.assertEqual(len(builds), 2)
                    reset = next(i for i, command in enumerate(calls) if '--zero-stats' in command)
                    builder_calls = [i for i, command in enumerate(calls) if 'flatpak-builder' in command]
                    self.assertLess(builder_calls[0], reset)
                    self.assertLess(reset, builder_calls[1])
                    self.assertEqual(sum(command[0] == 'flatpak-builder-lint' for command in calls), 6)
                else:
                    with self.assertRaises(ValueError):
                        probe.measure(state, manifest, root, execute=execute, compiler_reuse=True)
                self.assertEqual(manifest.read_bytes(), original)
                self.assertEqual((state / 'ccache/original').read_text(), 'ccache')
                self.assertEqual(sorted(path.name for path in root.iterdir()), ['manifest.json', 'state'])

    def test_pruned_rebuild_failure_cleans_temporary_state_and_manifest(self):
        probe = self.load_probe()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            state, manifest = self.fixture(root)
            original = manifest.read_bytes()
            def execute(command, seconds):
                raise subprocess.CalledProcessError(1, command)
            with self.assertRaises(subprocess.CalledProcessError):
                probe.measure(state, manifest, root, execute=execute)
            self.assertEqual(manifest.read_bytes(), original)
            self.assertEqual(sorted(p.name for p in root.iterdir()), ['manifest.json', 'state'])

    def test_pruned_rebuild_rejects_missing_rebuild_signal(self):
        probe = self.load_probe()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            state, manifest = self.fixture(root)
            with self.assertRaisesRegex(ValueError, 'rebuild'):
                probe.measure(state, manifest, root, execute=lambda command, seconds: 'Cache hit for obs, skipping build')
            self.assertEqual(sorted(p.name for p in root.iterdir()), ['manifest.json', 'state'])

    def test_pruned_rebuild_manual_default_off_after_validation(self):
        for file in ('ci.yml', 'build-project.yaml'):
            workflow = (ROOT / '.github/workflows' / file).read_text()
            self.assertRegex(workflow, r'flatpak-pruned-build-probe:\n(?:[^\n]*\n){0,4}\s+default: false')
        workflow = (ROOT / '.github/workflows/build-project.yaml').read_text()
        self.assertLess(workflow.index('- name: Validate repository'), workflow.index('- name: Probe Pruned Flatpak Rebuild'))
        self.assertIn('if: inputs.flatpak-pruned-build-probe', workflow)
        self.assertIn('uv run ci/flatpak_pruned_build_probe.py', workflow)
        self.assertIn('--compiler-reuse', workflow)

    def test_pruned_rebuild_still_rejects_links_in_dependency_repository(self):
        probe = self.load_probe()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            state, manifest = self.fixture(root)
            (state / 'cache/escaped-link').symlink_to(state / 'cache/original')
            def forbidden(command, seconds):
                self.fail('commands must not run for linked dependency state')
            with self.assertRaisesRegex(ValueError, 'cache/escaped-link'):
                probe.measure(state, manifest, root, execute=forbidden)
            self.assertEqual((state / 'cache/original').read_text(), 'cache')
            self.assertEqual(sorted(p.name for p in root.iterdir()), ['manifest.json', 'state'])


class FlatpakCompilerTransferTest(unittest.TestCase):
    def load_transfer(self):
        spec = importlib.util.spec_from_file_location('flatpak_compiler_cache', ROOT / 'ci/flatpak_compiler_cache.py')
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module

    def test_roundtrip_and_reject_tampered_or_wrong_profile_before_overlay(self):
        module = self.load_transfer()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / 'source'
            (source / 'a').mkdir(parents=True)
            (source / 'a/result').write_bytes(b'actual compiler bytes')
            (source / 'bin').mkdir()
            for compiler in ('cc', 'c++', 'gcc', 'g++'):
                (source / 'bin' / compiler).symlink_to('/usr/bin/ccache')
            profile = {'source': 'a' * 40, 'sdk': 'sdk-commit', 'compiler': 'compiler-sha', 'manifest': 'manifest-sha'}
            artifact = root / 'artifact'
            report = module.pack(source, artifact, profile)
            self.assertEqual(report['payload_bytes'], 21)
            target = root / 'target'
            (target / 'bin').mkdir(parents=True)
            for compiler in ('cc', 'c++', 'gcc', 'g++'):
                (target / 'bin' / compiler).symlink_to('/usr/bin/ccache')
            module.restore(artifact, target, profile)
            self.assertTrue((target / 'bin/cc').is_symlink())
            self.assertEqual(report['omitted_builder_wrappers'], ['bin/c++', 'bin/cc', 'bin/g++', 'bin/gcc'])
            self.assertEqual((target / 'a/result').read_bytes(), b'actual compiler bytes')
            with self.assertRaisesRegex(ValueError, 'profile'):
                module.restore(artifact, root / 'wrong', dict(profile, compiler='changed'))
            self.assertFalse((root / 'wrong').exists())
            outside = root / 'outside'
            outside.write_bytes(b'preserve')
            wrong_wrapper = root / 'wrong-wrapper'
            (wrong_wrapper / 'bin').mkdir(parents=True)
            (wrong_wrapper / 'bin/cc').symlink_to(outside)
            with self.assertRaisesRegex(ValueError, 'links'):
                module.restore(artifact, wrong_wrapper, profile)
            self.assertEqual(outside.read_bytes(), b'preserve')
            with self.assertRaisesRegex(ValueError, 'link|wrapper'):
                module.pack(wrong_wrapper, root / 'invalid-wrapper-artifact', profile)
            archive = artifact / 'compiler-cache.tar'
            archive.write_bytes(archive.read_bytes() + b'changed')
            with self.assertRaisesRegex(ValueError, 'digest|size'):
                module.restore(artifact, root / 'tampered', profile)
            self.assertFalse((root / 'tampered').exists())

    def test_archive_paths_links_and_limits_fail_before_destination_mutation(self):
        module = self.load_transfer()
        import hashlib
        import io
        import tarfile
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name, kind in [('../escape', tarfile.REGTYPE), ('/absolute', tarfile.REGTYPE), ('bin/cc', tarfile.REGTYPE),
                               ('link', tarfile.SYMTYPE), ('hardlink', tarfile.LNKTYPE)]:
                artifact = root / ('artifact-' + str(len(list(root.iterdir()))))
                artifact.mkdir()
                archive = artifact / 'compiler-cache.tar'
                with tarfile.open(archive, 'w') as output:
                    entry = tarfile.TarInfo(name)
                    entry.type = kind
                    entry.linkname = '../escape' if kind != tarfile.REGTYPE else ''
                    entry.size = 1 if kind == tarfile.REGTYPE else 0
                    output.addfile(entry, io.BytesIO(b'x') if entry.size else None)
                metadata = {'format': 1, 'profile': {}, 'archive_bytes': archive.stat().st_size,
                            'sha256': hashlib.sha256(archive.read_bytes()).hexdigest(), 'payload_bytes': 1, 'files': 1}
                (artifact / 'manifest.json').write_text(json.dumps(metadata))
                with self.assertRaises(ValueError):
                    module.restore(artifact, root / 'destination', {})
                self.assertFalse((root / 'destination').exists())
            source = root / 'source'
            source.mkdir()
            (source / 'link').symlink_to('/etc/passwd')
            with self.assertRaisesRegex(ValueError, 'link|regular'):
                module.pack(source, root / 'linked', {})
            (source / 'link').rename(source / 'rejected-link')
            source = root / 'bounded-source'
            source.mkdir()
            (source / 'large').write_bytes(b'ab')
            with patch.object(module, 'MAX_PAYLOAD_BYTES', 1), self.assertRaisesRegex(ValueError, 'limit|size'):
                module.pack(source, root / 'oversize', {})

    def test_forced_consumer_changes_only_application_post_install(self):
        module = self.load_transfer()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = root / 'manifest.json'
            recipe = {'sdk': 'sdk', 'modules': [{'name': 'dependency'}, {'name': 'obs',
                      'config-opts': ['-DENABLE_CCACHE=ON'], 'post-install': ['original']}]}
            manifest.write_text(json.dumps(recipe))
            argv = ['flatpak_compiler_cache.py', 'force', '--manifest', str(manifest),
                    '--cache', str(root / 'cache'), '--artifact', str(root)]
            with patch.object(sys, 'argv', argv):
                module.main()
            actual = json.loads(manifest.read_text())
            marker = actual['modules'][-1]['post-install'].pop()
            self.assertIn('OBS_COMPILER_CONSUMER_EXECUTED', marker)
            self.assertEqual(actual, recipe)

    def test_consumer_counters_reject_trivial_or_low_reuse(self):
        module = self.load_transfer()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = root / 'manifest.json'
            manifest.write_text(json.dumps({'sdk': 'org.freedesktop.Sdk', 'runtime-version': '26.08'}))
            argv = ['flatpak_compiler_cache.py', 'verify', '--manifest', str(manifest),
                    '--cache', str(root / 'cache'), '--artifact', str(root)]
            for hits, misses, accepted in [(0, 330, False), (5, 0, False), (150, 100, False), (328, 2, True)]:
                with self.subTest(hits=hits, misses=misses), patch.object(sys, 'argv', argv), \
                     patch.object(module.subprocess, 'check_output', return_value=f'direct_cache_hit {hits}\ncache_miss {misses}\n'):
                    if accepted:
                        module.main()
                    else:
                        with self.assertRaisesRegex(ValueError, 'insufficient'):
                            module.main()
                    result = json.loads((root / 'consumer-result.json').read_text())
                    self.assertEqual(result['status'], 'success' if accepted else 'failed')

    def test_consumer_prepares_sdk_before_checking_transferred_profile(self):
        workflow = (ROOT / '.github/workflows/build-project.yaml').read_text()
        consumer = workflow.split('  flatpak-compiler-consumer:\n', 1)[1].split('  flatpak-runtime-test:', 1)[0]
        prepare = consumer.index('      - name: Prepare Compiler Consumer SDK\n')
        verify = consumer.index('      - name: Verify and Restore Compiler Artifact Last\n')
        self.assertLess(prepare, verify)
        block = consumer[prepare:verify]
        self.assertIn('--install-deps-only', block)
        self.assertIn('--install-deps-from=flathub', block)
        self.assertIn('timeout 300s flatpak-builder', block)
        self.assertIn('--state-dir "$RUNNER_TEMP/flatpak-sdk-prep"', block)

    def test_sdk_mismatch_reports_both_profiles_without_restoring_cache(self):
        module = self.load_transfer()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / 'source'
            source.mkdir()
            (source / 'result').write_bytes(b'cached output')
            artifact = root / 'artifact'
            producer = {'sdk': 'updated-sdk', 'compiler': 'updated-compiler'}
            consumer = {'sdk': 'image-sdk', 'compiler': 'image-compiler'}
            module.pack(source, artifact, producer)
            with self.assertRaisesRegex(ValueError, 'profile'):
                module.restore(artifact, root / 'destination', consumer)
            self.assertFalse((root / 'destination').exists())
            report = json.loads((artifact / 'profile-mismatch.json').read_text())
            self.assertEqual(report['producer'], producer)
            self.assertEqual(report['consumer'], consumer)
            self.assertEqual(report['different_fields'], ['compiler', 'sdk'])

    def test_consumer_bootstrap_path_is_applied_before_vendor_step(self):
        workflow = (ROOT / '.github/workflows/build-project.yaml').read_text()
        consumer = workflow.split('  flatpak-compiler-consumer:\n', 1)[1].split('  flatpak-runtime-test:', 1)[0]
        self.assertRegex(consumer, r'cmake -P cmake/common/prepare-flatpak-soldr\.cmake\n'
                         r'      - name: Vendor Flatpak Compiler Consumer Sources\n'
                         r'        run: build-aux/prepare-flatpak-rust\.sh')

    def test_container_artifacts_use_one_directory_to_preserve_the_cache_payload(self):
        workflow = (ROOT / '.github/workflows/build-project.yaml').read_text()
        upload = workflow.split('      - name: Upload Experimental Flatpak Compiler Artifact\n', 1)[1].split('      - name:', 1)[0]
        # The hosted multiline container input uploaded only metadata beneath /.
        # One controlled directory resolves once and includes both required files.
        self.assertIn('path: ${{ runner.temp }}/flatpak-compiler-transfer\n', upload)
        self.assertNotIn('path: |', upload)
        consumer = workflow.split('  flatpak-compiler-consumer:\n', 1)[1].split('  flatpak-runtime-test:', 1)[0]
        self.assertIn('Stage Compiler Consumer Measurements', consumer)
        self.assertIn('path: ${{ runner.temp }}/flatpak-compiler-result\n', consumer)
        self.assertIn('*.json', consumer)
        self.assertNotIn('path: ${{ runner.temp }}/flatpak-compiler-transfer\n',
                         consumer.split('      - name: Upload Compiler Consumer Measurements', 1)[1])

    def test_transfer_is_explicit_manual_default_off_and_fresh_consumer(self):
        workflow = (ROOT / '.github/workflows/build-project.yaml').read_text()
        caller = (ROOT / '.github/workflows/ci.yml').read_text()
        for text in [workflow, caller]:
            declaration = text.split('      flatpak-compiler-transfer-probe:', 1)[1].split('      flatpak-', 1)[0]
            self.assertIn('default: false', declaration)
        self.assertIn("github.event_name == 'workflow_dispatch' && inputs.flatpak-compiler-transfer-probe", caller)
        consumer = workflow.split('  flatpak-compiler-consumer:', 1)[1].split('  flatpak-runtime-test:', 1)[0]
        self.assertIn('needs: flatpak-build', consumer)
        self.assertIn('if: inputs.flatpak-compiler-transfer-probe', consumer)
        self.assertIn('save-cache: false', consumer)
        self.assertIn('restore-cache: false', consumer)
        self.assertIn('ci/flatpak_compiler_cache.py pack', workflow)
        self.assertIn('mirror-screenshots-url: https://dl.flathub.org/media', consumer)
        self.assertIn('Prepare build space', consumer)
        self.assertIn('required_gib=40', consumer)
        self.assertIn('/usr/local/lib/android:/to_clean/android', consumer)
        gate = tomllib.loads((ROOT / 'local-gate.toml').read_text())
        self.assertIn('flatpak-compiler-consumer', gate['gate']['verify-exempt'])


class FlatpakDependencyTransferTest(unittest.TestCase):
    def load_transfer(self):
        spec = importlib.util.spec_from_file_location('flatpak_dependency_cache', ROOT / 'ci/flatpak_dependency_cache.py')
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module

    def test_real_archive_roundtrip_preserves_opaque_objects_and_checks_profile(self):
        module = self.load_transfer()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / 'source'
            object_path = source / 'objects/ab' / ('c' * 62 + '.file')
            object_path.parent.mkdir(parents=True)
            object_path.symlink_to('/opaque/payload/not/a/structural/path')
            (source / 'config').write_text('[core]\nmode=bare-user-only\n')
            profile = {'source': 'source-sha', 'sdk': 'sdk-sha'}
            artifact = root / 'artifact'
            with patch.object(module, 'fsck') as fsck:
                report = module.pack(source, artifact, profile)
                destination = root / 'destination'
                module.restore(artifact, destination, profile)
                self.assertEqual(fsck.call_count, 2)
            self.assertEqual((destination / object_path.relative_to(source)).readlink(), object_path.readlink())
            self.assertEqual((destination / 'config').read_bytes(), (source / 'config').read_bytes())
            self.assertGreater(report['archive_bytes'], 0)
            self.assertEqual(report['symbolic_objects'], 1)
            with self.assertRaisesRegex(ValueError, 'profile'):
                module.restore(artifact, root / 'wrong', {'source': 'source-sha', 'sdk': 'changed'})
            self.assertFalse((root / 'wrong').exists())
            archive = artifact / 'dependency-cache.tar.zst'
            archive.write_bytes(archive.read_bytes() + b'corrupted')
            with self.assertRaisesRegex(ValueError, 'digest|size'):
                module.restore(artifact, root / 'corrupt', profile)
            self.assertFalse((root / 'corrupt').exists())

    def test_structural_links_and_unsafe_members_reject_before_destination_mutation(self):
        module = self.load_transfer()
        import tarfile
        for name, kind in [('config', tarfile.SYMTYPE), ('../escape', tarfile.REGTYPE),
                           ('/absolute', tarfile.REGTYPE), ('objects/a/file', tarfile.LNKTYPE),
                           ('objects/ab/' + 'c' * 62 + '.file/child', tarfile.REGTYPE)]:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                source = root / 'source'
                source.mkdir()
                (source / 'config').symlink_to(root / 'outside')
                with self.assertRaisesRegex(ValueError, 'structural|link'):
                    module.pack(source, root / 'artifact', {})
                member = tarfile.TarInfo(name)
                member.type = kind
                member.linkname = '/outside'
                with self.assertRaisesRegex(ValueError, 'unsafe|structural|link'):
                    module.validate_member(member)

    def test_nonportable_repository_metadata_is_rejected_and_failed_fsck_cleans_staging(self):
        module = self.load_transfer()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / 'source'
            source.mkdir()
            (source / 'config').write_text('[core]\nmode=bare-user\n')
            with patch.object(module, 'fsck'), self.assertRaisesRegex(ValueError, 'mode'):
                module.pack(source, root / 'unsupported', {})
            self.assertFalse((root / 'unsupported').exists())
            (source / 'config').write_text('[core]\nmode=bare-user-only\n')
            artifact = root / 'artifact'
            with patch.object(module, 'fsck'):
                module.pack(source, artifact, {})
            with patch.object(module, 'fsck', side_effect=ValueError('fsck failed')):
                with self.assertRaisesRegex(ValueError, 'fsck'):
                    module.restore(artifact, root / 'destination', {})
            self.assertFalse((root / 'destination').exists())
            self.assertFalse(list(root.glob('flatpak-dependency-*')))
            with patch.object(module, 'MAX_BYTES', 1), self.assertRaisesRegex(ValueError, 'limit'):
                module.pack(source, root / 'oversized', {})

    def test_dependency_probe_is_manual_and_restores_before_compiler(self):
        workflow = (ROOT / '.github/workflows/build-project.yaml').read_text()
        caller = (ROOT / '.github/workflows/ci.yml').read_text()
        for text in [workflow, caller]:
            block = text.split('      flatpak-dependency-transfer-probe:', 1)[1].split('      flatpak-', 1)[0]
            self.assertIn('default: false', block)
        self.assertIn("github.event_name == 'workflow_dispatch' && inputs.flatpak-dependency-transfer-probe", caller)
        consumer = workflow.split('  flatpak-compiler-consumer:', 1)[1].split('  flatpak-runtime-test:', 1)[0]
        self.assertIn('!inputs.flatpak-dependency-transfer-probe', consumer)
        self.assertLess(consumer.index('Restore Experimental Dependency Artifact'),
                        consumer.index('Verify and Restore Compiler Artifact Last'))
        self.assertIn('Verify All Dependency Modules Were Reused', consumer)
        self.assertIn('ci/flatpak_dependency_cache.py verify', consumer)

    def test_builder_capture_preserves_arguments_failure_and_dependency_rebuild_guards(self):
        module = self.load_transfer()
        import contextlib
        import io
        import types
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            builder = root / 'real builder'
            builder.write_text('#!/bin/bash\nprintf "%s\\n" "$@"\nexit 23\n')
            builder.chmod(0o700)
            artifact = root / 'artifact with spaces'
            manifest = root / 'manifest.json'
            dependencies = ['base', 'devel', 'tools', 'qt', 'cef', 'rust-toolchain']
            manifest.write_text(json.dumps({'modules': [{'name': name} for name in dependencies + ['obs']]}))
            argv = ['flatpak_dependency_cache.py', 'instrument', '--manifest', str(manifest),
                    '--cache', str(root / 'cache'), '--artifact', str(artifact)]
            with patch.dict(sys.modules, {'flatpak_compiler_cache': types.SimpleNamespace(context=lambda path: {})}):
                with patch.object(sys, 'argv', argv), patch.object(module.shutil, 'which', return_value=str(builder)):
                    with contextlib.redirect_stdout(io.StringIO()):
                        module.main()
                result = subprocess.run([str(artifact / 'bin/flatpak-builder'), 'literal $(false)'],
                                        capture_output=True, text=True, check=False)
                self.assertEqual(result.returncode, 23)
                self.assertEqual((artifact / 'builder.log').read_text().strip(), 'literal $(false)')
                good = ''.join('Cache hit for ' + name + ', skipping build\n' for name in dependencies)
                for output, accepted in [(good + 'OBS_COMPILER_CONSUMER_EXECUTED\n', True),
                                         (good, False),
                                         (good + 'OBS_COMPILER_CONSUMER_EXECUTED\nCache hit for obs, skipping build\n', False)]:
                    (artifact / 'builder.log').write_text(output)
                    argv[1] = 'verify'
                    with patch.object(sys, 'argv', argv), contextlib.redirect_stdout(io.StringIO()):
                        if accepted:
                            module.main()
                        else:
                            with self.assertRaisesRegex(ValueError, 'reuse|rebuild'):
                                module.main()
                    report = json.loads((artifact / 'dependency-result.json').read_text())
                    self.assertEqual(report['status'], 'success' if accepted else 'failed')
