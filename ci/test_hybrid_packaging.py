"""Hybrid packages keep offline inputs pinned and publish only after approval."""

import importlib.util
import json
import os
import re
import shutil
import subprocess
import tempfile
import tomllib
import unittest
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
        self.assertIn("lookup-only: true", lookup)
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
