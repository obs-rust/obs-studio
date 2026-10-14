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
    def test_windows_identity_readback_is_recorded_after_logging_starts(self):
        platform = (ROOT / "frontend/utility/platform-windows.cpp").read_text()
        start = platform.index("bool InitializeAppIdentity()")
        if "static bool AppIdentityMatches()" in platform:
            start = platform.rfind(
                "#ifdef OBS_RUST_VERSION",
                0,
                platform.index("static bool AppIdentityMatches()"),
            )
        functions = platform[
            start : platform.index("extern bool portable_mode;", start)
        ]
        app = (ROOT / "frontend/OBSApp.cpp").read_text()
        initialization = app.split("void OBSApp::AppInit()", 1)[1]
        initialization = initialization[
            initialization.index("#ifdef _WIN32") : initialization.index(
                "\n\tif (!MigrateRustConfiguration"
            )
        ]
        main = (ROOT / "frontend/obs-main.cpp").read_text()
        logging = main[
            main.rindex("\t\tif (!created_log) {") : main.index(
                "\t\tprogram.checkForUncleanShutdown();"
            )
        ]
        with tempfile.TemporaryDirectory(prefix="obs-windows-identity-log-") as scratch:
            directory = Path(scratch)
            source = directory / "identity.cpp"
            source.write_text(
                '#include <cstdio>\n#include <cstdlib>\n#include <cwchar>\n#include "utility/AppIdentity.hpp"\n'
                "#define SUCCEEDED(x) ((x)>=0)\n#define FAILED(x) ((x)<0)\n"
                "using PWSTR=wchar_t*; using HRESULT=int; constexpr int LOG_INFO=1;\n"
                "int logged=0, sets=0, gets=0; bool logging_open=false;\n"
                "int SetCurrentProcessExplicitAppUserModelID(const wchar_t*) { ++sets; return 0; }\n"
                "int GetCurrentProcessExplicitAppUserModelID(PWSTR *s) { ++gets; *s=wcsdup(OBS_WINDOWS_APP_ID); return 0; }\n"
                "void CoTaskMemFree(void *p) { free(p); }\n"
                "void blog(int,const char*,...) { if(logging_open) ++logged; }\n"
                "void create_log_file(int&) { logging_open=true; }\n"
                + functions
                + "int main() { bool created_log=false; int logFile=0;\n"
                + initialization
                + "\n"
                + logging
                + '\nprintf("%d %d %d\\n",logged,sets,gets); }\n'
            )
            executable = directory / "identity"
            compiled = subprocess.run(
                [
                    "c++",
                    "-std=c++17",
                    "-D_WIN32",
                    "-DOBS_RUST_VERSION=1",
                    "-I",
                    str(ROOT / "frontend"),
                    str(source),
                    "-o",
                    str(executable),
                ],
                capture_output=True,
                text=True,
            )
            self.assertEqual(compiled.returncode, 0, compiled.stderr)
            output = subprocess.check_output([str(executable)], text=True).split()
            self.assertEqual(
                output[:2],
                ["1", "1"],
                "exactly one verified identity log after logger opens",
            )
            self.assertGreaterEqual(
                int(output[2]),
                2,
                "read actual process identity again after logger opens",
            )

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

    def test_settings_folder_uses_selected_application_identity(self):
        source = (ROOT / "frontend/widgets/OBSBasic_MainControls.cpp").read_text()
        body = source.split(
            "void OBSBasic::on_actionShowSettingsFolder_triggered()", 1
        )[1].split("\n}", 1)[0]
        expression = body.split("const std::string userConfigPath = ", 1)[1].split(
            ";", 1
        )[0]
        with tempfile.TemporaryDirectory(prefix="obs-settings-folder-") as scratch:
            directory = Path(scratch)
            harness = directory / "settings.cpp"
            harness.write_text(
                "#include <filesystem>\n#include <iostream>\n#include <string>\n"
                '#include "utility/AppIdentity.hpp"\n'
                "struct FakeApp { std::filesystem::path userConfigLocation; };\n"
                'FakeApp *App() { static FakeApp app{"/chosen/storage"}; return &app; }\n'
                "int main() { std::cout << (" + expression + "); }\n"
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
                self.assertEqual(
                    subprocess.check_output([str(executable)], text=True),
                    "/chosen/storage/" + ("obs-studio-rust" if rust else "obs-studio"),
                )

    def test_default_migration_root_has_no_redundant_separator(self):
        source = (ROOT / "frontend/OBSApp.cpp").read_text()
        migration = source.split("bool OBSApp::MigrateRustConfiguration()", 1)[1].split(
            "namespace OBS", 1
        )[0]
        expression = migration.split("const auto base = ", 1)[1].split(";", 1)[0]
        with tempfile.TemporaryDirectory(
            prefix="obs-default-migration-root-"
        ) as scratch:
            directory = Path(scratch)
            harness = directory / "root.cpp"
            harness.write_text(
                '#include <filesystem>\n#include <iostream>\n#include "utility/ConfigMigration.hpp"\n'
                "struct Root { const char *value; const char *Get() { return value; } };\n"
                "int main(int argc,char **argv) { namespace fs=std::filesystem; Root configRoot{argv[1]}; "
                "const auto base=" + expression + "; std::cout << base.u8string(); }\n"
            )
            executable = directory / "root"
            subprocess.run(
                [
                    "c++",
                    "-std=c++17",
                    "-I",
                    str(ROOT / "frontend"),
                    str(harness),
                    "-o",
                    str(executable),
                ],
                check=True,
                capture_output=True,
            )
            for value, expected in (
                ("/chosen/config/", "/chosen/config"),
                ("/chosen/config", "/chosen/config"),
                ("/", "/"),
            ):
                with self.subTest(value=value):
                    self.assertEqual(
                        subprocess.check_output([str(executable), value], text=True),
                        expected,
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
                "locations",
                "locations-existing",
                "locations-failure",
            ):
                with self.subTest(mode=mode):
                    completed = subprocess.run(
                        [str(executable), str(directory / mode), mode],
                        capture_output=True,
                        text=True,
                    )
                    self.assertEqual(completed.returncode, 0, completed.stderr)


class InstalledAppIdentityTest(unittest.TestCase):
    def test_failed_snapshot_comparison_does_not_report_verified_isolation(self):
        source = (ROOT / "build-aux/appimage/test-appimage.sh").read_text()
        start = source.index(
            'if [ "$CONFIG_NAME" = obs-studio-rust ]; then\n    source_snapshot'
        )
        block = source[start : source.index('\nLOG="', start)]
        with tempfile.TemporaryDirectory(prefix="obs-snapshot-report-") as scratch:
            directory = Path(scratch)
            root = directory / "obs-studio-rust"
            (root / "basic/profiles/Test").mkdir(parents=True)
            (root / "basic/profiles/Test/basic.ini").touch()
            (root / "basic/scenes").mkdir(parents=True)
            (root / "basic/scenes/Test.json").touch()
            (root / "plugin_manager").mkdir()
            (root / "plugin_manager/migration-proof.txt").write_text(
                "preserved plugin settings\n"
            )
            (root / "global.ini").touch()
            (directory / "upstream-before.txt").write_text("original\n")
            script = (
                'XDG_CONFIG_HOME="$1"\nOUT="$1"\nCONFIG_NAME=obs-studio-rust\nSCENARIO=saved-profile-existing\n'
                'FAILED=()\nfail() { FAILED+=("$*"); }\nsource_snapshot() { printf "changed\\n"; }\n'
                + block
                + '\nprintf "failure_count=%s\\n" "${#FAILED[@]}"\n'
            )
            output = subprocess.check_output(
                ["bash", "-c", script, "test", str(directory)], text=True
            )
            self.assertIn("failure_count=1", output)
            self.assertNotIn("Rust config isolation verified", output)

    def test_non_debian_installs_the_snapshot_comparison_tool(self):
        source = (ROOT / "build-aux/appimage/test-appimage.sh").read_text()
        block = source.split('case "${ID_LIKE:-} ${ID}" in', 1)[1]
        block = (
            'case "${ID_LIKE:-} ${ID}" in' + block.split("\nesac", 1)[0] + "\nesac\n"
        )
        with tempfile.TemporaryDirectory(prefix="obs-package-tools-") as scratch:
            for distro in ("fedora", "arch"):
                requested = Path(scratch) / distro
                script = (
                    'ID="$1"\nID_LIKE=""\nREQUESTS="$2"\n'
                    'dnf() { printf "%s\\n" "$@" >>"$REQUESTS"; }\n'
                    'pacman() { printf "%s\\n" "$@" >>"$REQUESTS"; }\n' + block
                )
                subprocess.run(
                    ["bash", "-c", script, "test", distro, str(requested)], check=True
                )
                with self.subTest(distro=distro):
                    self.assertIn("diffutils", requested.read_text().splitlines())

    def test_appimage_identity_keeps_the_ubuntu_pipewire_exception(self):
        source = (ROOT / "build-aux/appimage/test-appimage.sh").read_text()
        block = source.split('PACKAGE_DATA="$APPDIR/usr/share/obs"', 1)[1]
        block = (
            'PACKAGE_DATA="$APPDIR/usr/share/obs"'
            + block.split("# The C and C++ runtime", 1)[0]
        )
        predicate = next(
            line.strip()
            for line in source.splitlines()
            if 'if [ "$ID" = ubuntu ]' in line
        )
        with tempfile.TemporaryDirectory(prefix="obs-appimage-distro-") as scratch:
            directory = Path(scratch)
            (directory / "usr/bin").mkdir(parents=True)
            (directory / "usr/bin/obs-studio-rust-gui").touch()
            (directory / "usr/share/metainfo").mkdir(parents=True)
            (
                directory
                / "usr/share/metainfo/io.github.obs_rust.OBSStudioRust.metainfo.xml"
            ).touch()
            (directory / "io.github.obs_rust.OBSStudioRust.desktop").write_text(
                "Exec=obs-studio-rust-gui\nIcon=io.github.obs_rust.OBSStudioRust\nStartupWMClass=obs-studio-rust-gui\n"
            )
            script = (
                'APPDIR="$1"\nID="$2"\nVERSION_ID="$3"\nVERSION_OUT="OBS-Studio-Rust 0.1.0"\n'
                'FAILED=()\nfail() { FAILED+=("$*"); }\nfinish() { exit 7; }\n'
                + block
                + '\nprintf "%s\\n" "$ID"\n'
                + predicate
                + '\nprintf "allowed\\n"\nelse\nprintf "denied\\n"\nfi\n'
            )
            for distro, version, result in (
                ("ubuntu", "22.04", "allowed"),
                ("ubuntu", "24.04", "denied"),
                ("debian", "13", "denied"),
                ("arch", "rolling", "denied"),
            ):
                with self.subTest(distro=distro, version=version):
                    output = subprocess.check_output(
                        ["bash", "-c", script, "test", str(directory), distro, version],
                        text=True,
                    )
                    self.assertEqual(output.splitlines(), [distro, result])

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
        self.assertIn("StartupWMClass=obs-studio-rust-gui\n", desktop)

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
        self.assertIn("StartupWMClass=obs\n", desktop)

    def test_debian_dependency_scan_knows_the_private_library_directory(self):
        for rust in (False, True):
            with (
                self.subTest(rust=rust),
                tempfile.TemporaryDirectory(prefix="obs-private-deps-") as scratch,
            ):
                directory = Path(scratch)
                defaults = (
                    (ROOT / "cmake/linux/defaults.cmake")
                    .read_text()
                    .split("include(cpackconfig)", 1)[0]
                )
                packaging = (
                    (ROOT / "cmake/linux/cpackconfig.cmake")
                    .read_text()
                    .split("include(CPack)", 1)[0]
                    .replace("include_guard(GLOBAL)", "")
                )
                (directory / "CMakeLists.txt").write_text(
                    "cmake_minimum_required(VERSION 3.28)\nproject(private_deps LANGUAGES C)\n"
                    'set(OS_LINUX TRUE)\nset(CMAKE_INSTALL_PREFIX "/usr")\nset(CMAKE_INSTALL_LIBDIR "lib")\n'
                    "set(OBS_VERSION 33.0.0)\nset(OBS_VERSION_MAJOR 33)\nset(OBS_VERSION_MINOR 0)\nset(OBS_VERSION_PATCH 0)\n"
                    + f"set(ENABLE_RUST_LIBOBS {'ON' if rust else 'OFF'})\n"
                    + f'list(APPEND CMAKE_MODULE_PATH "{ROOT}/cmake/common")\n'
                    + f'include("{ROOT}/cmake/common/app-identity.cmake")\n'
                    + defaults
                    + "\n"
                    + packaging
                    + '\nfile(WRITE "${CMAKE_BINARY_DIR}/deps.txt" "${CPACK_DEBIAN_PACKAGE_SHLIBDEPS}\\n${CPACK_DEBIAN_PACKAGE_SHLIBDEPS_PRIVATE_DIRS}\\n")\n'
                )
                build = directory / "build"
                subprocess.run(
                    ["cmake", "-S", str(directory), "-B", str(build)],
                    check=True,
                    capture_output=True,
                )
                enabled, paths = (build / "deps.txt").read_text().splitlines()
                self.assertEqual(
                    enabled, "TRUE", "dependency checking must stay enabled"
                )
                self.assertEqual(paths, "./usr/lib/obs-studio-rust" if rust else "")


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

    def test_release_installs_uv_before_package_identity_checks(self):
        workflow = (ROOT / ".github/workflows/release-rust.yaml").read_text()
        build = workflow.split("\n  build:", 1)[1].split("\n  appimage:", 1)[0]
        self.assertIn("astral-sh/setup-uv@", build)
        self.assertLess(
            build.index("astral-sh/setup-uv@"),
            build.index("- name: Collect and verify"),
        )

    def test_each_build_script_forwards_the_explicit_switch(self):
        posix = (ROOT / ".github/scripts/build-macos").read_text()
        before_platform = posix.split("case ${target}", 1)[0]
        self.assertIn(
            "-DENABLE_RUST_LIBOBS:BOOL=${ENABLE_RUST_LIBOBS}", before_platform
        )
        windows = (ROOT / ".github/scripts/Build-Windows.ps1").read_text()
        self.assertIn("-DENABLE_RUST_LIBOBS:BOOL=$env:ENABLE_RUST_LIBOBS", windows)


class StartupOrderTest(unittest.TestCase):
    def test_rust_import_precedes_crash_handler_directory_creation(self):
        source = (ROOT / "frontend/OBSApp.cpp").read_text()
        constructor = source.split("OBSApp::OBSApp(", 1)[1].split(
            "OBSApp::~OBSApp()", 1
        )[0]
        constructor = constructor.split("void OBSApp::InitCrashHandler()", 1)[0]
        with tempfile.TemporaryDirectory(prefix="obs-startup-order-") as scratch:
            fragment = Path(scratch) / "constructor.cpp"
            fragment.write_text(constructor)
            rust = subprocess.check_output(
                ["c++", "-E", "-P", "-DOBS_RUST_VERSION=1", str(fragment)], text=True
            )
            upstream = subprocess.check_output(
                ["c++", "-E", "-P", str(fragment)], text=True
            )
        self.assertNotIn(
            "CrashHandler",
            rust,
            "crash handler would create the Rust directory before first-run import",
        )
        self.assertIn("CrashHandler", upstream, "C-only constructor behavior changed")
        startup = source.split("void OBSApp::AppInit()", 1)[1].split(
            "void OBSApp::", 1
        )[0]
        self.assertLess(
            startup.index("MigrateRustConfiguration"), startup.index("InitCrashHandler")
        )
