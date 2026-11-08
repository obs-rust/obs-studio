use super::*;
use obs_build::validation::graph::{Dependency, Graph};
use std::process::{Command, Output};

fn checked(command: &mut Command) -> Output {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{command:?}\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

#[test]
fn canonical_qt_daemons_runs_unchanged() {
    assert!(std::env::var_os("DISPLAY").is_some_and(|v| !v.is_empty()));
    let root = dunce::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let raw: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap())
            .unwrap();
    let mut selected = expected();
    for section in ["targets", "tests"] {
        for (name, record) in selected[section].as_object_mut().unwrap() {
            *record = raw[section][name].clone();
        }
    }
    let mut graph = Graph::parse(&selected.to_string()).unwrap();
    for target in graph.targets.values_mut() {
        target.sanitizers = None;
    }
    let build = NativeBuild {
        host: obs_build::BUILD_HOST.into(),
        target: obs_build::BUILD_TARGET.into(),
        output: directory.path().join("cargo-output"),
        profile: Default::default(),
    };
    let mut core = built_core(&build, &root);
    core.dependency.compile_options.clear();
    core.cmake
        .push_str("set_property(TARGET OBS::libobs PROPERTY INTERFACE_COMPILE_OPTIONS \"\")\n");
    let core_path = core.dependency.artifacts[0].clone();
    let support = build_owners(directory.path(), &root, &core.cmake);
    assert_bindings(
        &support.join("test_vst_editor_close_module.so"),
        &core_path,
        "vst-module",
    );
    let mut interfaces = interfaces(&build, &root, &support, core.dependency);
    // The original standalone frontend-glue configuration has no Rust owner.
    // Full callers must supply this interface with the owner's flags and archive.
    interfaces.insert(
        "OBS::fallback-warning-test-frontend".into(),
        Dependency::default(),
    );
    let facts = [
        ("OS_LINUX".into(), true),
        ("HAVE_QT_WIDGETS_TEST_OWNER".into(), true),
        ("HAVE_QT_WIDGETS_TESTLIB_OWNER".into(), true),
        ("HAVE_PLUGIN_MANAGER_TEST_OWNER".into(), true),
        ("HAVE_VST_EDITOR_TEST_OWNER".into(), true),
        ("HAVE_PRIVATE_DAEMON_TEST_OWNER".into(), true),
        ("HAVE_OBS_PIPEWIRE".into(), true),
        ("HAVE_OBS_PULSEAUDIO".into(), true),
        ("ENABLE_PULSEAUDIO".into(), true),
        ("HAVE_X11_DISPLAY".into(), true),
    ]
    .into();
    let artifacts = graph.compile(&build, &root, &facts, &interfaces).unwrap();
    assert_eq!(artifacts.len(), CASES.len());
    for (name, spec) in &graph.tests {
        assert_bindings(&artifacts[&spec.target], &core_path, &spec.target);
        let result = checked(
            spec.command(&root, &build.output, &artifacts[&spec.target])
                .unwrap()
                .env("CMOCKA_MESSAGE_OUTPUT", "STDOUT"),
        );
        assert_cases(&result, name);
        eprintln!(
            "Cargo {name}: {}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
    fs::write(directory.path().join("graph.json"), selected.to_string()).unwrap();
    write_cmake(directory.path(), &root, &support, &core.cmake);
    replay_cmake(directory.path(), &core_path);
}

fn build_owners(directory: &Path, root: &Path, core: &str) -> std::path::PathBuf {
    let source = directory.join("owners-src");
    let output = directory.join("owners");
    fs::create_dir_all(&source).unwrap();
    let vst =
        fs::read_to_string(root.join("test/cmocka/tests.d/test_vst_editor_close.cmake")).unwrap();
    fs::write(
        source.join("vst-owners.cmake"),
        vst.replace("${CMAKE_SOURCE_DIR}", &root.to_string_lossy()),
    )
    .unwrap();
    fs::write(
        source.join("CMakeLists.txt"),
        format!(
            r#"cmake_minimum_required(VERSION 3.28)
project(qt-daemon-owners C CXX)
set(CMAKE_CXX_STANDARD 17)
set(OS_LINUX TRUE)
set(CMAKE_MODULE_PATH "{root}/cmake/finders")
include("{root}/cmake/common/unity.cmake")
include("{root}/cmake/common/pch.cmake")
include("{root}/cmake/common/groups.cmake")
find_package(Qt6 6.10 REQUIRED COMPONENTS Widgets Test Svg)
find_package(nlohmann_json 3.11 REQUIRED)
{core}
add_subdirectory("{root}/shared/qt/idian" "${{CMAKE_BINARY_DIR}}/shared/qt/idian")
include("${{CMAKE_CURRENT_SOURCE_DIR}}/vst-owners.cmake")
add_library(daemon-glad STATIC "{root}/deps/glad/src/glad.c")
target_include_directories(daemon-glad PRIVATE "{root}/deps/glad/include")
"#,
            root = root.display()
        ),
    )
    .unwrap();
    checked(
        Command::new("cmake")
            .arg("-S")
            .arg(source)
            .arg("-B")
            .arg(&output)
            .args(["-G", "Ninja", "-DCMAKE_BUILD_TYPE=RelWithDebInfo"]),
    );
    checked(Command::new("cmake").arg("--build").arg(&output).args([
        "--target",
        "idian",
        "test_vst_editor_close_module",
        "test_vst_editor_close_plugin",
        "daemon-glad",
        "--parallel",
        "4",
    ]));
    output
}

fn interfaces(
    build: &NativeBuild,
    root: &Path,
    support: &Path,
    core: Dependency,
) -> std::collections::BTreeMap<String, Dependency> {
    let mut result = std::collections::BTreeMap::from([("OBS::libobs".into(), core)]);
    for (name, package) in [
        ("Qt6::Widgets", "Qt6Widgets"),
        ("Qt6::Test", "Qt6Test"),
        ("Qt6::Svg", "Qt6Svg"),
        ("OBS::native-test-cmocka", "cmocka"),
        ("PulseAudio::PulseAudio", "libpulse"),
        ("PipeWire::PipeWire", "libpipewire-0.3"),
        ("gio::gio", "gio-2.0"),
    ] {
        result.insert(
            name.into(),
            Dependency::package(&build.target, package).unwrap(),
        );
    }
    let mut idian = result["Qt6::Svg"].clone();
    idian
        .artifacts
        .push(support.join("shared/qt/idian/libidian.a"));
    idian
        .include_dirs
        .push(root.join("shared/qt/idian/include"));
    result.insert("OBS::idian".into(), idian);
    result.insert("nlohmann_json::nlohmann_json".into(), Dependency::default());
    result.insert(
        "OBS::glad".into(),
        Dependency {
            artifacts: vec![support.join("libdaemon-glad.a")],
            include_dirs: vec![root.join("deps/glad/include")],
            ..Default::default()
        },
    );
    for name in [
        "test_vst_editor_close_module",
        "test_vst_editor_close_plugin",
    ] {
        result.insert(
            name.into(),
            Dependency {
                artifacts: vec![support.join(format!("{name}.so"))],
                ..Default::default()
            },
        );
    }
    result
}

fn write_cmake(directory: &Path, root: &Path, support: &Path, core: &str) {
    fs::write(directory.join("CMakeLists.txt"), format!(r#"cmake_minimum_required(VERSION 3.28)
project(qt-daemon-contract C CXX)
set(CMAKE_CXX_STANDARD 17)
enable_testing()
option(OS_LINUX "Linux" ON)
option(OS_WINDOWS "Windows" OFF)
option(OS_MACOS "macOS" OFF)
option(WITH_QT "Qt SDK" ON)
option(WITH_OWNERS "Plugin owners" ON)
option(WITH_DAEMONS "Private daemon tools" ON)
option(ENABLE_UI "UI" ON)
option(ENABLE_FRONTEND "Frontend" ON)
set(HAVE_OBS_PIPEWIRE TRUE)
set(HAVE_OBS_PULSEAUDIO TRUE)
set(ENABLE_PULSEAUDIO TRUE)
set(HAVE_PRIVATE_DAEMON_TEST_OWNER ${{WITH_DAEMONS}})
{core}
find_package(PkgConfig REQUIRED)
pkg_check_modules(CMOCKA REQUIRED IMPORTED_TARGET cmocka)
add_library(OBS::native-test-cmocka ALIAS PkgConfig::CMOCKA)
foreach(pair IN ITEMS "PipeWire::PipeWire=libpipewire-0.3" "PulseAudio::PulseAudio=libpulse")
  string(REPLACE "=" ";" fields "${{pair}}")
  list(GET fields 0 name)
  list(GET fields 1 package)
  string(MAKE_C_IDENTIFIER "${{package}}" prefix)
  pkg_check_modules(${{prefix}} REQUIRED IMPORTED_TARGET "${{package}}")
  add_library(${{name}} ALIAS PkgConfig::${{prefix}})
endforeach()
# Use the real plugin interface: a pkg-config gio alias also links GLib and
# GObject directly, concealing missing fixture dependencies under as-needed.
list(PREPEND CMAKE_MODULE_PATH "{root}/cmake/finders")
find_package(Gio REQUIRED)
add_link_options("LINKER:--no-copy-dt-needed-entries" "LINKER:--as-needed")
add_library(daemon-glad STATIC IMPORTED)
set_target_properties(daemon-glad PROPERTIES IMPORTED_LOCATION "{support}/libdaemon-glad.a" INTERFACE_INCLUDE_DIRECTORIES "{root}/deps/glad/include")
add_library(OBS::glad ALIAS daemon-glad)
if(WITH_QT)
  find_package(Qt6 6.10 REQUIRED COMPONENTS Widgets Test Svg)
  add_library(OBS::fallback-warning-test-frontend INTERFACE IMPORTED)
  if(WITH_OWNERS)
    find_package(nlohmann_json 3.11 REQUIRED)
    add_library(OBS::idian STATIC IMPORTED)
    set_target_properties(OBS::idian PROPERTIES IMPORTED_LOCATION "{support}/shared/qt/idian/libidian.a" INTERFACE_INCLUDE_DIRECTORIES "{root}/shared/qt/idian/include" INTERFACE_LINK_LIBRARIES "Qt6::Widgets;Qt6::Svg")
    foreach(name test_vst_editor_close_module test_vst_editor_close_plugin)
      add_library(${{name}} MODULE IMPORTED)
      set_property(TARGET ${{name}} PROPERTY IMPORTED_LOCATION "{support}/${{name}}.so")
    endforeach()
  endif()
endif()
function(obs_test_sanitize)
endfunction()
include("{root}/test/cmocka/native-test-graph.cmake")
obs_native_test_graph("${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json" "{root}")
"#,root=root.display(),support=support.display())).unwrap();
}

fn replay_cmake(directory: &Path, core: &Path) {
    let daemons = vec![
        "test_pipewire_context_fail",
        "test_pulse_monitor_device",
        "test_pulse_two_sources",
    ];
    for (mode, options, names) in [
        ("enabled", vec![], CASES.to_vec()),
        ("windows", vec!["-DOS_LINUX=OFF", "-DOS_WINDOWS=ON"], vec![]),
        ("macos", vec!["-DOS_LINUX=OFF", "-DOS_MACOS=ON"], vec![]),
        ("no-qt", vec!["-DWITH_QT=OFF"], daemons.clone()),
        ("ui-off", vec!["-DENABLE_UI=OFF"], daemons.clone()),
        ("frontend-off", vec!["-DENABLE_FRONTEND=OFF"], daemons),
        (
            "no-owners",
            vec!["-DWITH_OWNERS=OFF"],
            vec![
                "test_fallback_warning_queue",
                "test_pipewire_context_fail",
                "test_pulse_monitor_device",
                "test_pulse_two_sources",
            ],
        ),
        (
            "no-daemons",
            vec!["-DWITH_DAEMONS=OFF"],
            vec![
                "test_fallback_warning_queue",
                "test_plugin_manager",
                "test_vst_editor_close",
            ],
        ),
    ] {
        let output = directory.join(mode);
        checked(
            Command::new("cmake")
                .arg("-S")
                .arg(directory)
                .arg("-B")
                .arg(&output)
                .args(["-G", "Ninja", "-DCMAKE_BUILD_TYPE=RelWithDebInfo"])
                .args(options),
        );
        if mode == "enabled" {
            checked(
                Command::new("cmake")
                    .arg("--build")
                    .arg(&output)
                    .args(["--parallel", "4"]),
            );
            for name in &names {
                assert_bindings(&output.join(name), core, name);
                let result = checked(
                    Command::new("ctest")
                        .arg("--test-dir")
                        .arg(&output)
                        .args(["-V", "--no-tests=error", "-R"])
                        .arg(format!("^{name}$"))
                        .env("CMOCKA_MESSAGE_OUTPUT", "STDOUT"),
                );
                assert_cases(&result, name);
                assert!(
                    String::from_utf8_lossy(&result.stdout)
                        .contains("100% tests passed, 0 tests failed out of 1")
                );
                eprintln!("CMake {name}: {}", String::from_utf8_lossy(&result.stdout));
            }
        }
        check_inventory(&output, &names, mode == "enabled");
        eprintln!("Inventory {mode}: {}", names.len());
    }
}

fn check_inventory(output: &Path, names: &[&str], built: bool) {
    let result = checked(
        Command::new("ctest")
            .arg("--test-dir")
            .arg(output)
            .arg("--show-only=json-v1"),
    );
    let inventory: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    let tests = inventory["tests"].as_array().unwrap();
    assert_eq!(tests.len(), names.len(), "{}", output.display());
    let expected = expected();
    for test in tests {
        let name = test["name"].as_str().unwrap();
        assert!(names.contains(&name));
        let record = &expected["tests"][name];
        let properties = test["properties"].as_array().unwrap();
        let property = |name: &str| {
            properties
                .iter()
                .find(|p| p["name"] == name)
                .map(|p| &p["value"])
        };
        assert_eq!(
            property("TIMEOUT").and_then(serde_json::Value::as_f64),
            record["timeout_seconds"].as_f64()
        );
        assert_eq!(
            property("RESOURCE_LOCK")
                .cloned()
                .unwrap_or_else(|| serde_json::json!([])),
            record
                .get("resource_locks")
                .cloned()
                .unwrap_or_else(|| serde_json::json!([]))
        );
        assert_eq!(
            property("SKIP_RETURN_CODE").and_then(serde_json::Value::as_f64),
            record["skip_return_code"].as_f64()
        );
        let mut environment: Vec<String> = record
            .get("environment")
            .and_then(serde_json::Value::as_object)
            .map(|values| {
                values
                    .iter()
                    .map(|(key, value)| format!("{key}={}", value.as_str().unwrap()))
                    .collect()
            })
            .unwrap_or_default();
        environment.sort();
        let mut actual: Vec<String> = property("ENVIRONMENT")
            .and_then(serde_json::Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .map(|v| v.as_str().unwrap().to_owned())
                    .collect()
            })
            .unwrap_or_default();
        actual.sort();
        assert_eq!(actual, environment, "{name}");
        if built {
            assert_eq!(
                normalized_command(&test["command"]),
                record["command"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| {
                        v.as_str()
                            .unwrap()
                            .replace(
                                "${source}",
                                &dunce::canonicalize(
                                    Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."),
                                )
                                .unwrap()
                                .to_string_lossy(),
                            )
                            .replace("${target}", &output.join(name).to_string_lossy())
                    })
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                dunce::canonicalize(property("WORKING_DIRECTORY").unwrap().as_str().unwrap())
                    .unwrap(),
                dunce::canonicalize(output).unwrap()
            );
        }
    }
}

fn normalized_command(command: &serde_json::Value) -> Vec<String> {
    let mut arguments: Vec<String> = command
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap().to_owned())
        .collect();
    if Path::new(&arguments[0]).file_name() == Some(std::ffi::OsStr::new("uv")) {
        let resolved = checked(Command::new("which").arg("uv"));
        assert_eq!(
            dunce::canonicalize(&arguments[0]).unwrap(),
            dunce::canonicalize(String::from_utf8_lossy(&resolved.stdout).trim()).unwrap()
        );
        arguments[0] = "uv".into();
    }
    arguments
}

fn assert_cases(output: &Output, name: &str) {
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !super::super::canonical_display_startup_is_registered::has_skipped_cases(&text),
        "{text}"
    );
    let expected = match name {
        "test_fallback_warning_queue" => 10,
        "test_plugin_manager" => 8,
        "test_vst_editor_close" => 4,
        "test_pipewire_context_fail" => 3,
        "test_pulse_monitor_device" => 3,
        "test_pulse_two_sources" => 2,
        _ => panic!("unexpected fixture {name}"),
    };
    let actual: Vec<usize> = text
        .lines()
        .filter_map(|line| {
            line.split_once("[  PASSED  ] ")
                .map(|(_, summary)| summary.split_whitespace().next().unwrap().parse().unwrap())
        })
        .collect();
    assert_eq!(actual, [expected], "{text}");
}

fn assert_bindings(executable: &Path, core: &Path, name: &str) {
    let output = checked(Command::new("ldd").arg(executable));
    let text = String::from_utf8_lossy(&output.stdout);
    if matches!(name, "test_fallback_warning_queue" | "test_plugin_manager") {
        assert!(
            !text.contains("libobs.so"),
            "standalone dialog must remain core-free: {text}"
        );
    } else {
        assert!(
            text.contains(core.to_str().unwrap()),
            "{} must load {}: {text}",
            executable.display(),
            core.display()
        );
    }
    if !matches!(
        name,
        "test_fallback_warning_queue"
            | "test_plugin_manager"
            | "test_vst_editor_close"
            | "vst-module"
    ) {
        eprintln!("Loader {name}: {}\n{text}", core.display());
        return;
    }
    let selected = checked(Command::new("pkg-config").args(["--variable=libdir", "Qt6Widgets"]));
    let expected = dunce::canonicalize(
        Path::new(String::from_utf8_lossy(&selected.stdout).trim()).join("libQt6Widgets.so.6"),
    )
    .unwrap();
    let actual = text
        .lines()
        .find(|line| line.contains("libQt6Widgets.so.6 =>"))
        .unwrap()
        .split_whitespace()
        .nth(2)
        .unwrap();
    assert_eq!(dunce::canonicalize(actual).unwrap(), expected);
    eprintln!("Loader {name}: {}\n{text}", core.display());
}
