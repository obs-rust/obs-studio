#!/usr/bin/env -S uv run --script
"""Compile the real frontend config-path functions for both identities (#632).

uv run python -m unittest ci.test_app_config_identity
"""

from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent


class AppConfigIdentityTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory(prefix="obs-config-identity-")
        cls.addClassCleanup(cls.temp.cleanup)
        directory = Path(cls.temp.name)
        source = (ROOT / "frontend/OBSApp.cpp").read_text()
        start = source.index("int GetAppConfigPath(char *path")
        end = source.index("int GetProgramDataPath(", start)
        functions = source[start:end]
        header = ROOT / "frontend/utility/AppIdentity.hpp"
        identity_include = (
            '#include "utility/AppIdentity.hpp"\n' if header.exists() else ""
        )
        harness = directory / "paths.cpp"
        harness.write_text(
            "#include <cstdio>\n#include <cstdlib>\n#include <cstring>\n#include <string>\n"
            + identity_include
            + '#define ALLOW_PORTABLE_MODE 1\n#define CONFIG_PATH "/portable/config"\n'
            + "bool portable_mode = false;\n"
            + "char *bstrdup(const char *s) { return strdup(s); }\n"
            + "int os_get_config_path(char *p, size_t n, const char *s) {\n"
            + 'return s && *s ? snprintf(p,n,"/user/config/%s",s) : snprintf(p,n,"/user/config"); }\n'
            + "char *os_get_config_path_ptr(const char *s) { char p[1024]; "
            + "os_get_config_path(p,sizeof(p),s); return bstrdup(p); }\n"
            + functions
            + "int main(int argc, char **argv) { portable_mode = argv[1][0] == '1'; "
            + "const char *name = argc > 2 ? argv[2] : nullptr; char p[1024]; "
            + "if (GetAppConfigPath(p,sizeof(p),name) <= 0) return 2; puts(p); "
            + "char *owned = GetAppConfigPathPtr(name); if (!owned) return 3; "
            + "puts(owned); free(owned); }\n"
        )
        cls.executables = {}
        for rust in (False, True):
            executable = directory / ("rust" if rust else "upstream")
            command = ["c++", "-std=c++17", "-I", str(ROOT / "frontend")]
            if rust:
                command.append("-DOBS_RUST_VERSION=1")
            subprocess.run(
                command + [str(harness), "-o", str(executable)],
                check=True,
                capture_output=True,
            )
            cls.executables[rust] = executable

    def paths(self, rust, portable, name):
        command = [str(self.executables[rust]), "1" if portable else "0"]
        if name is not None:
            command.append(name)
        return subprocess.check_output(command, text=True).splitlines()

    def test_rust_config_paths_are_isolated(self):
        for portable in (False, True):
            root = "/portable/config" if portable else "/user/config"
            for name in (
                "obs-studio/global.ini",
                "obs-studio/basic/profiles",
                "obs-studio\\updates\\updater.exe",
            ):
                with self.subTest(portable=portable, name=name):
                    expected = (
                        root + "/" + name.replace("obs-studio", "obs-studio-rust", 1)
                    )
                    self.assertEqual(
                        self.paths(True, portable, name), [expected, expected]
                    )

    def test_upstream_paths_stay_compatible(self):
        for portable in (False, True):
            root = "/portable/config" if portable else "/user/config"
            self.assertEqual(
                self.paths(False, portable, "obs-studio/global.ini"),
                [root + "/obs-studio/global.ini"] * 2,
            )

    def test_only_the_obs_directory_is_rewritten(self):
        for name in ("obs-studio-other/settings", "other/settings"):
            self.assertEqual(
                self.paths(True, False, name), ["/user/config/" + name] * 2
            )


class ConfigMigrationTest(unittest.TestCase):
    def test_first_run_migration_contract(self):
        with tempfile.TemporaryDirectory(prefix="obs-config-migration-") as scratch:
            directory = Path(scratch)
            executable = directory / "migration"
            subprocess.run(
                [
                    "c++",
                    "-std=c++17",
                    "-I",
                    str(ROOT / "frontend"),
                    str(ROOT / "frontend/tests/test_config_migration.cpp"),
                    "-o",
                    str(executable),
                ],
                check=True,
                capture_output=True,
            )
            for mode in (
                "first",
                "existing",
                "absent",
                "custom",
                "empty-custom",
                "failure",
            ):
                with self.subTest(mode=mode):
                    completed = subprocess.run(
                        [str(executable), str(directory / mode), mode],
                        capture_output=True,
                        text=True,
                    )
                    self.assertEqual(completed.returncode, 0, completed.stderr)
