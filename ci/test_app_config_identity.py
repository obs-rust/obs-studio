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


class InstalledAppIdentityTest(unittest.TestCase):
    def configure(self, rust):
        with tempfile.TemporaryDirectory(prefix="obs-installed-identity-") as scratch:
            directory = Path(scratch)
            # Execute the production defaults before CPack/dependency discovery;
            # this tests their actual install/RPATH decisions without a full build.
            defaults = (ROOT / "cmake/linux/defaults.cmake").read_text()
            defaults = defaults.split("include(cpackconfig)", 1)[0]
            (directory / "CMakeLists.txt").write_text(
                "cmake_minimum_required(VERSION 3.28)\nproject(identity LANGUAGES C)\n"
                'set(CMAKE_INSTALL_PREFIX "/usr")\nset(CMAKE_INSTALL_LIBDIR "lib")\n'
                + f"set(ENABLE_RUST_LIBOBS {'ON' if rust else 'OFF'})\n"
                + "set(ENABLE_RELOCATABLE ON)\n"
                + f'include("{ROOT}/cmake/common/app-identity.cmake")\n'
                + defaults
                + '\nfile(WRITE "${CMAKE_BINARY_DIR}/identity.txt" '
                '"${OBS_DESKTOP_ID}\\n${OBS_MACOS_BUNDLE_ID}\\n${OBS_MACOS_BUNDLE_NAME}\\n'
                "${OBS_UNIX_GUI_NAME}\\n${OBS_LIBRARY_DESTINATION}\\n"
                '${OBS_PLUGIN_DESTINATION}\\n${OBS_DATA_DESTINATION}\\n${OBS_EXECUTABLE_RPATH}\\n")\n'
                + f'configure_file("{ROOT}/frontend/cmake/linux/com.obsproject.Studio.desktop" '
                '"${CMAKE_BINARY_DIR}/app.desktop" @ONLY)\n'
            )
            build = directory / "build"
            subprocess.run(
                ["cmake", "-S", str(directory), "-B", str(build)],
                check=True,
                capture_output=True,
            )
            return (build / "identity.txt").read_text().splitlines(), (
                build / "app.desktop"
            ).read_text()

    def test_rust_installed_identity_and_private_libraries(self):
        identity, desktop = self.configure(True)
        self.assertEqual(
            identity,
            [
                "io.github.obs_rust.OBSStudioRust",
                "io.github.obs_rust.OBSStudioRust",
                "OBS-Studio-Rust",
                "obs-studio-rust-gui",
                "lib/obs-studio-rust",
                "lib/obs-studio-rust/obs-modules",
                "share/obs-studio-rust",
                "$ORIGIN/;$ORIGIN/../lib/obs-studio-rust",
            ],
        )
        self.assertIn("Name=OBS-Studio-Rust\n", desktop)
        self.assertIn("Exec=obs-studio-rust-gui\n", desktop)
        self.assertIn("Icon=io.github.obs_rust.OBSStudioRust\n", desktop)

    def test_upstream_installed_identity_stays_compatible(self):
        identity, desktop = self.configure(False)
        self.assertEqual(
            identity,
            [
                "com.obsproject.Studio",
                "com.obsproject.obs-studio",
                "OBS",
                "obs",
                "lib",
                "lib/obs-modules",
                "share/obs",
                "$ORIGIN/;$ORIGIN/../lib",
            ],
        )
        self.assertIn("Exec=obs\n", desktop)
        self.assertIn("Icon=com.obsproject.Studio\n", desktop)


class InstanceIdentityTest(unittest.TestCase):
    def test_linux_instance_check_ignores_the_other_application(self):
        with tempfile.TemporaryDirectory(prefix="obs-instance-identity-") as scratch:
            directory = Path(scratch)
            source = (ROOT / "frontend/utility/platform-x11.cpp").read_text()
            start = source.index("void CheckIfAlreadyRunning(bool &already_running)")
            functions = source[start : source.index("\n#endif", start)]
            harness = directory / "instance.cpp"
            harness.write_text(
                "#include <cstdio>\n#include <cstdlib>\n#include <cstring>\n#include <string>\n"
                "#include <iostream>\n#include <sys/socket.h>\n#include <sys/un.h>\n#include <unistd.h>\n"
                '#include "utility/AppIdentity.hpp"\n'
                "std::string observed; std::string fixture;\n"
                "int socket(int,int,int) {return 42;}\n"
                "int bind(int,const sockaddr *address,socklen_t) {"
                "observed=reinterpret_cast<const sockaddr_un *>(address)->sun_path+1; return 0;}\n"
                'FILE *fopen(const char *,const char *) {return fmemopen(fixture.data(),fixture.size(),"r");}\n'
                "constexpr int LOG_ERROR=1; void blog(int,const char *,...) {}\n"
                'struct FakeApp { std::string GetVersionString() {return "test";} };\n'
                "FakeApp *App() { static FakeApp app; return &app; }\n"
                + functions
                + "\nint main(int argc,char **argv) {fixture=argv[1]; bool running=true; "
                'CheckIfAlreadyRunning(running); std::cout<<observed<<"\\n"<<running<<"\\n"; }\n'
            )
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
                own = "/io/github/obs_rust/OBSStudioRust" if rust else "/com/obsproject"
                other = (
                    "/com/obsproject" if rust else "/io/github/obs_rust/OBSStudioRust"
                )
                fixture = f"entry @{own} 10 test\nentry @{other} 20 test\nentry @{other} 21 test\n"
                lines = subprocess.check_output(
                    [str(executable), fixture], text=True
                ).splitlines()
                with self.subTest(rust=rust):
                    self.assertTrue(lines[0].startswith(own + " "), lines[0])
                    self.assertEqual(
                        lines[1],
                        "0",
                        "other application's processes counted as our instances",
                    )


class IdentityBuildSelectionTest(unittest.TestCase):
    def test_build_switch_precedes_installed_identity_defaults(self):
        root = (ROOT / "CMakeLists.txt").read_text()
        option = root.find("option(ENABLE_RUST_LIBOBS")
        self.assertGreaterEqual(
            option, 0, "Rust switch must be declared at the top level"
        )
        self.assertLess(option, root.index("include(app-identity)"))
        self.assertLess(option, root.index("include(defaults)"))

    def test_release_uses_explicit_rust_build_selection(self):
        workflow = (ROOT / ".github/workflows/release-rust.yaml").read_text()
        build = workflow.split("- name: Build OBS-Studio-Rust", 1)[1].split(
            "- name:", 1
        )[0]
        self.assertIn("rustLibobs: 'ON'", build)
        self.assertNotIn("sed -i.bak", workflow)
        self.assertNotIn("update-index --assume-unchanged", workflow)

    def test_each_build_script_forwards_the_explicit_switch(self):
        posix = (ROOT / ".github/scripts/build-macos").read_text()
        before_platform = posix.split("case ${target}", 1)[0]
        self.assertIn(
            "-DENABLE_RUST_LIBOBS:BOOL=${ENABLE_RUST_LIBOBS}", before_platform
        )
        windows = (ROOT / ".github/scripts/Build-Windows.ps1").read_text()
        self.assertIn("-DENABLE_RUST_LIBOBS:BOOL=$env:ENABLE_RUST_LIBOBS", windows)
