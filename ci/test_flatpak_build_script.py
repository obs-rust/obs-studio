"""Check the Flatpak OBS compiler cache pins and the sandbox build lifecycle."""

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "build-aux/io.github.obs_rust.OBSStudioRust.json"
WRAPPER = ROOT / "build-aux/flatpak/ninja.cmake"
EXPECTED = {
    "x86_64": "11bf3c33a55c9032f6e153faa21c06d35f137e9ccbe13a91c3a7daa24f56f65d",
    "aarch64": "e4cd3f7123e890f04ef157be375c6c0a90203a675e373cc68a87e14b03f671a4",
}


class FlatpakBuildScriptTest(unittest.TestCase):
    def test_release_is_pinned_per_architecture_outside_app(self):
        manifest = json.loads(MANIFEST.read_text())
        obs = manifest["modules"][-1]
        self.assertEqual(obs["name"], "obs")
        archives = [source for source in obs["sources"] if source.get("dest") == ".flatpak-zccache"]
        self.assertEqual(len(archives), len(EXPECTED))
        self.assertEqual({tuple(s["only-arches"]): s["sha256"] for s in archives},
                         {(arch,): digest for arch, digest in EXPECTED.items()})
        for source in archives:
            arch, = source["only-arches"]
            self.assertEqual(source["type"], "archive")
            self.assertEqual(source["url"], "https://github.com/forge-ci/zccache/releases/download/"
                             f"1.15.22/zccache-v1.15.22-{arch}-unknown-linux-musl.tar.gz")
            self.assertEqual(source["strip-components"], 1)
        self.assertIn("-DOBS_ZCCACHE_LAUNCHER=/run/build/obs/.flatpak-zccache/zccache", obs["config-opts"])
        self.assertEqual(obs["build-options"]["prepend-path"], "/run/build/obs/.flatpak-zccache/bin")
        shim, = [source for source in obs["sources"] if source.get("dest-filename") == "ninja"]
        self.assertEqual(shim, {"type": "script", "dest-filename": "ninja", "dest": ".flatpak-zccache/bin",
                                "commands": ['exec cmake -P /run/build/obs/build-aux/flatpak/ninja.cmake -- "$@"']})
        self.assertTrue(WRAPPER.is_file())

    def test_profile_binds_sdk_compilers_and_release_digest(self):
        from flatpak_compiler_cache import context

        recipe = json.loads(MANIFEST.read_text())
        with tempfile.TemporaryDirectory() as directory:
            manifest = Path(directory) / 'manifest.json'
            manifest.write_text(json.dumps(recipe))
            outputs = ['x86_64', 'source-ref', 'sdk-commit', 'compiler-digests']
            with patch('flatpak_compiler_cache.subprocess.check_output', side_effect=outputs) as query:
                profile = context(manifest)
            self.assertEqual(profile['architecture'], 'x86_64')
            self.assertEqual(profile['source'], 'source-ref')
            self.assertEqual(profile['sdk'], 'sdk-commit')
            self.assertEqual(profile['compiler'], 'compiler-digests')
            self.assertEqual(profile['zccache'], EXPECTED['x86_64'])
            self.assertEqual(query.call_args_list[-1].args[0][-1], 'sha256sum /usr/bin/cc /usr/bin/c++')
            obs = recipe['modules'][-1]
            obs['build-options']['env']['OBS_FLATPAK_ZCCACHE_JOURNAL'] = '1'
            manifest.write_text(json.dumps(recipe, indent=4))
            with patch('flatpak_compiler_cache.subprocess.check_output', side_effect=outputs):
                self.assertEqual(context(manifest), profile)
            obs['config-opts'].append('-DCHANGED_BUILD_INPUT=ON')
            manifest.write_text(json.dumps(recipe))
            with patch('flatpak_compiler_cache.subprocess.check_output', side_effect=outputs):
                self.assertNotEqual(context(manifest)['manifest'], profile['manifest'])

    def test_package_guard_rejects_staged_tools_and_daemon_state(self):
        from flatpak_compiler_cache import verify_package

        with tempfile.TemporaryDirectory() as directory:
            app = Path(directory)
            (app / "bin").mkdir()
            (app / "bin/obs").touch()
            verify_package(app)
            for name in ("zccache", "zccache-fp", "zccache-daemon", ".flatpak-zccache",
                         ".flatpak-zccache-state", "obs-zccache"):
                with self.subTest(name=name):
                    item = app / "bin" / name
                    item.touch()
                    with self.assertRaisesRegex(ValueError, f"leaked.*bin/{name}"):
                        verify_package(app)
                    item.unlink()

    def test_stats_distinguish_cold_build_and_missing_report(self):
        with tempfile.TemporaryDirectory() as directory:
            cache = Path(directory)
            command = [sys.executable, str(ROOT / "ci/flatpak_compiler_cache.py"),
                       "report", "--cache", directory]
            missing = subprocess.run(command, capture_output=True, text=True)
            self.assertNotEqual(missing.returncode, 0)
            self.assertIn("session.json", missing.stderr)
            report = cache / "session.json"
            report.write_text(json.dumps({"status": "ok", "hits": 0, "misses": 20, "compilations": 23,
                                          "non_cacheable": 3, "errors": 0}))
            cold = subprocess.check_output(command, text=True)
            self.assertIn("warm_hits=0 cold_misses=20 uncacheable=3 compilations=23", cold)
            self.assertIn("::warning::Flatpak zccache reported no hits", cold)
            report.write_text('{"status":"ok","hits":0,"misses":0,"compilations":0,"non_cacheable":0,"errors":0}')
            empty = subprocess.run(command, capture_output=True, text=True)
            self.assertNotEqual(empty.returncode, 0)
            self.assertIn("observed zero compilations", empty.stderr)

    def test_ninja_publishes_session_only_after_successful_stop(self):
        wrapper = WRAPPER.read_text()
        for build_status, stop_status, expected in ((0, 0, 0), (17, 0, 17), (0, 23, 23)):
            with self.subTest(build_status=build_status, stop_status=stop_status), tempfile.TemporaryDirectory() as directory:
                module = Path(directory)
                build = module / "_flatpak_build"
                build.mkdir()
                (module / ".flatpak-zccache").mkdir()
                cache_mount = module / "mount"
                cache_mount.mkdir()
                log = module / "calls"
                real_ninja = module / "real-ninja"
                real_ninja.write_text("#!/bin/sh\n"
                                      'printf "ninja:%s:%s\\n" "${ZCCACHE_SESSION_ID-}" "${ZCCACHE_DISABLE-}" >> "$LOG"\n'
                                      'exit "$BUILD_STATUS"\n')
                real_ninja.chmod(0o755)
                zccache = module / ".flatpak-zccache/zccache"
                zccache.write_text("""#!/bin/sh
printf '%s\\n' "$*" >> "$LOG"
test "$ZCCACHE_CACHE_DIR" = "$EXPECTED_CACHE/state" || exit 71
case $1 in
    session-start) printf '%s\\n' '{"session_id":"0123-abcd","started_at":42}' ;;
    session-end) printf '%s\\n' '{"status":"ok"}' ;;
    stop) test ! -e "$EXPECTED_CACHE/session.json" || exit 72
          exit "$STOP_STATUS" ;;
    *) exit 70 ;;
esac
""")
                zccache.chmod(0o755)
                shim = module / "ninja.cmake"
                shim.write_text(wrapper.replace("/usr/bin/ninja", str(real_ninja)).replace("/run/ccache", str(cache_mount)))
                cache = cache_mount / "obs-zccache"
                env = dict(os.environ, FLATPAK_BUILDER_BUILDDIR=directory, ZCCACHE_DISABLE="1",
                           LOG=str(log), EXPECTED_CACHE=str(cache), BUILD_STATUS="0", STOP_STATUS=str(stop_status))
                probe = subprocess.run(["cmake", "-P", shim, "--", "--version"], cwd=build, env=env)
                self.assertEqual(probe.returncode, 0)
                self.assertEqual(log.read_text(), "ninja::1\n")
                env["BUILD_STATUS"] = str(build_status)
                env["OBS_FLATPAK_ZCCACHE_JOURNAL"] = "1"
                result = subprocess.run(["cmake", "-P", shim, "--", "-j4", "-l8"], cwd=build, env=env)
                self.assertEqual(result.returncode, expected)
                self.assertEqual(log.read_text().splitlines(), [
                    "ninja::1", f"session-start --stats --journal {module}/flatpak-compiler.jsonl",
                    "ninja:0123-abcd:", "session-end --json 0123-abcd", "stop",
                ])
                self.assertEqual((cache / "session.json").is_file(), expected == 0)


if __name__ == "__main__":
    unittest.main()
