"""Tier 2 Linux build setup: a bounded zccache and no unused cook (#3026, #3027)."""
from pathlib import Path
import os
import re
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = ROOT / ".github/workflows/build-project.yaml"
VALIDATE = ROOT / "rust/tools/linux-validate/validate.sh"


def build_job():
    text = WORKFLOW.read_text()
    return re.search(r"^  rust-linux-tier2-build:\n(.*?)(?=^  [a-z0-9-]+:\n)", text, re.M | re.S)[1]


def step(job, name):
    return re.search(rf"- name: {re.escape(name)}.*?(?=\n      - |\Z)", job, re.S)[0]


class Tier2BuildSetupTest(unittest.TestCase):
    def test_only_persistent_host_disables_the_launcher(self):
        build = step(build_job(), "Build Tier 2 (libobs and libobs-c-reference) 🧱")
        # An empty true branch in `condition && '' || 'zccache'` always falls
        # through. Put the nonempty hosted value first instead.
        self.assertIn("OBS_TIER2_LAUNCHER: ${{ steps.pcache.outputs.zccache == '' && 'zccache' || '' }}", build)

    def test_launcher_default_override_and_empty(self):
        source = VALIDATE.read_text()
        setup = source.split('# Compiler cache:', 1)[1].split('# Read the same canonical', 1)[0]
        setup = '# Compiler cache:' + setup
        configure = source.split('  cmake -S "$SRC" -B "$build" -G Ninja', 1)[1].split('  # -k 0:', 1)[0]
        configure = 'cmake -S "$SRC" -B "$build" -G Ninja' + configure
        stats = source[source.index('echo "== zccache"'):]
        # Include the conditional that guards the final stats, when present.
        stats = source[source.rfind('\nif ', 0, source.index('echo "== zccache"')) + 1:] if source.rstrip().endswith('fi') else stats
        for launcher in (None, '', 'ccache'):
            with self.subTest(launcher=launcher), tempfile.TemporaryDirectory() as directory:
                env = dict(os.environ, SRC=str(ROOT), build=directory + '/build', BUILD=directory)
                env.pop('OBS_TIER2_LAUNCHER', None)
                if launcher is not None:
                    env['OBS_TIER2_LAUNCHER'] = launcher
                result = subprocess.run(['bash', '-euo', 'pipefail', '-c', '''
cmake() { printf '%s\\n' "$@" > "$build.args"; }
zccache() { echo "$*" >> "$build.zccache"; }
build_number=()
runpath_flags=()
''' + setup + configure + stats], env=env, capture_output=True, text=True)
                self.assertEqual(result.returncode, 0, result.stderr)
                args = Path(env['build'] + '.args').read_text().splitlines()
                expected = 'zccache' if launcher is None else launcher
                for language in ('C', 'CXX'):
                    flag = f'-DCMAKE_{language}_COMPILER_LAUNCHER='
                    self.assertEqual([arg for arg in args if arg.startswith(flag)],
                                     [flag + expected] if expected else [])
                calls = Path(env['build'] + '.zccache')
                self.assertEqual(calls.read_text().splitlines() if calls.exists() else [],
                                 ['status', 'stop'] if expected == 'zccache' else [])
                if not expected:
                    # A kept local build tree must not retain yesterday's launcher.
                    fixture = Path(directory, 'src')
                    fixture.mkdir()
                    (fixture / 'CMakeLists.txt').write_text('cmake_minimum_required(VERSION 3.24)\nproject(launcher NONE)\n')
                    cached = Path(directory, 'cached')
                    for flags in (['-DCMAKE_C_COMPILER_LAUNCHER=zccache', '-DCMAKE_CXX_COMPILER_LAUNCHER=zccache'],
                                  [arg for arg in args if arg.startswith(('-U', '-DCMAKE_C_COMPILER_LAUNCHER', '-DCMAKE_CXX_COMPILER_LAUNCHER'))]):
                        subprocess.run(['cmake', '-S', str(fixture), '-B', str(cached), *flags], check=True, capture_output=True)
                    self.assertNotIn('COMPILER_LAUNCHER:', (cached / 'CMakeCache.txt').read_text())

    def test_persistent_cache_requires_marker_and_runner_slot(self):
        job = build_job()
        self.assertIn("- /var/cache/obs-runner:/var/cache/obs-runner", job)
        select = step(job, "Select Persistent Cache 🗄️")
        self.assertIn("id: pcache", select)
        self.assertIn('if [ -f "$root/.persistent-cache" ] && [ -n "${RUNNER_NAME:-}" ]; then', select)
        self.assertIn('slot="$root/slots/$RUNNER_NAME"', select)
        self.assertIn('echo "zccache=$slot/tier2-zccache" >> "$GITHUB_OUTPUT"', select)
        self.assertIn('echo "apt=$root/apt-archives" >> "$GITHUB_OUTPUT"', select)
        self.assertIn('echo "no persistent cache on this runner"', select)
        self.assertLess(job.index("id: pcache"), job.index("id: apt-cache"))
        for record in ("daemon-*.lock", "daemon-*.lock.spawn-failed", "daemon.running-process.json"):
            self.assertIn(f'"$slot"/tier2-zccache/v*/{record}', select)
        # Only compiler outputs and .debs persist; never verdicts or module/cost state.
        for forbidden in ("verdict", ".tier2-modcache", ".tier2-cost", "OBS_TIER2_COST_DIR"):
            self.assertNotIn(forbidden, select)

    def test_hosted_apt_path_and_cache_gates_are_preserved(self):
        job = build_job()
        keep = step(job, "Keep apt Downloads 📦")
        self.assertIn('dir="${{ steps.pcache.outputs.apt }}"', keep)
        self.assertIn('[ -n "$dir" ] || dir="$RUNNER_TEMP/apt-archives"', keep)
        self.assertNotIn("runner.temp", keep)
        self.assertIn('echo "TIER2_APT_ARCHIVES=$dir" >> "$GITHUB_ENV"', keep)
        self.assertIn('Dir::Cache::archives=$TIER2_APT_ARCHIVES', step(job, "Install Tier 2 Packages 📦"))
        restore = re.search(r"- uses: ./\.github/actions/cache\n        id: apt-cache.*?(?=\n      - |\Z)", job, re.S)[0]
        self.assertIn("if: steps.pcache.outputs.apt == ''", restore)
        save = step(job, "Save apt Downloads 💾")
        self.assertIn("github.event_name == 'push' && github.ref == 'refs/heads/master'", save)
        self.assertIn("steps.apt-cache.outputs.cache-hit != 'true' && steps.pcache.outputs.apt == ''", save)
        for cache in (restore, save):
            self.assertIn("path: ${{ runner.temp }}/apt-archives", cache)

    def test_hosted_zccache_path_and_cache_gates_are_preserved(self):
        job = build_job()
        restore = re.search(r"- uses: ./\.github/actions/cache\n        if: steps.pcache.outputs.zccache == ''.*?(?=\n      - |\Z)", job, re.S)
        self.assertIsNotNone(restore)
        self.assertIn("mode: restore", restore[0])
        self.assertIn("path: ${{ github.workspace }}/.tier2-zccache", restore[0])
        save = step(job, "Save Tier 2 zccache 💾")
        self.assertIn("github.event_name != 'pull_request' && github.ref == 'refs/heads/master'", save)
        self.assertIn("steps.tier2-build.outputs.cache-healthy != 'false' && steps.pcache.outputs.zccache == ''", save)
        self.assertIn("path: ${{ github.workspace }}/.tier2-zccache", save)
        selected = "${{ steps.pcache.outputs.zccache || format('{0}/.tier2-zccache', github.workspace) }}"
        self.assertIn(f"ZCCACHE_CACHE_DIR: {selected}", step(job, "Build Tier 2 (libobs and libobs-c-reference) 🧱"))
        self.assertIn(f"cache-dir: {selected}", step(job, "Cache ledger 📒"))
        bound = step(job, "Bound zccache 🧹")
        self.assertIn('dir="${{ steps.pcache.outputs.zccache }}"', bound)
        self.assertLess(bound.index('exit 0; }'), bound.index('dir="$GITHUB_WORKSPACE/.tier2-zccache"'))

    def test_module_cost_and_artifact_state_stays_in_workspace(self):
        job = build_job()
        build = step(job, "Build Tier 2 (libobs and libobs-c-reference) 🧱")
        self.assertIn("OBS_LIBOBS_MODULE_CACHE_DIR: ${{ github.workspace }}/.tier2-modcache", build)
        self.assertIn("OBS_TIER2_COST_DIR: ${{ github.workspace }}/.tier2-cost", build)
        self.assertIn("path: .tier2-cost", job)
        self.assertIn("path: ${{ github.workspace }}/.tier2-modcache", job)
        for name in ("Save Tier 2 Module Cache 💾", "Upload Tier 2 Tree 🧾"):
            self.assertNotIn("pcache", step(job, name))

    def test_bound_reads_the_container_workspace(self):
        # The job runs in a container: ${{ github.workspace }} in a run
        # script is the host path, absent there, so the bound never ran and
        # the restored archive grew to 2.1 GB for a 1.3 GB build (#3026).
        bound = step(build_job(), "Bound zccache 🧹")
        run = bound.split("run: |", 1)[1]
        self.assertNotIn("github.workspace", run)
        self.assertIn('"$GITHUB_WORKSPACE/.tier2-zccache"', run)
        self.assertIn("du -sm", run)

    def test_bound_reports_size_on_every_run(self):
        bound = step(build_job(), "Bound zccache 🧹")
        self.assertNotIn("if: github.event_name", bound.split("run: |", 1)[0])
        self.assertIn("refs/heads/master", bound.split("run: |", 1)[1])

    def test_no_cook_for_corrosion_build(self):
        # Corrosion builds in its own target dir; the cooked workspace
        # target/ was loaded (12 s) and never used (#3027).
        soldr = step(build_job(), "Set Up soldr 🦀")
        self.assertRegex(soldr, r"\n\s+prebuild-deps: none\n")
        self.assertNotIn("prebuild-deps-flags", soldr)
        self.assertRegex(soldr, r"\n\s+cargo-registry-cache: true\n")


if __name__ == "__main__":
    unittest.main()
