use super::*;
use obs_build::validation::graph::{Dependency, Graph};
use std::process::{Command, Output};

fn checked(command: &mut Command) -> Output {
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("{command:?}: {error}"));
    assert!(
        output.status.success(),
        "{command:?}\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

#[test]
fn canonical_qt_widgets_runs_unchanged() {
    assert!(std::env::var_os("DISPLAY").is_some_and(|value| !value.is_empty()));
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
    // Like build_c_tests(per_target_sanitizers=false), this is the ordinary
    // Tier 2 replay. The graph retains the original sanitizer annotations.
    for target in graph.targets.values_mut() {
        target.sanitizers = None;
    }
    let build = NativeBuild {
        host: obs_build::BUILD_HOST.into(),
        target: obs_build::BUILD_TARGET.into(),
        output: directory.path().join("cargo-output"),
        profile: Default::default(),
    };
    let core = built_core(&build, &root);
    let core_path = core.dependency.artifacts[0].clone();
    let mut interfaces =
        std::collections::BTreeMap::from([("OBS::libobs".into(), core.dependency)]);
    for (name, package) in [
        ("Qt6::Widgets", "Qt6Widgets"),
        ("Qt6::Test", "Qt6Test"),
        ("OBS::native-test-cmocka", "cmocka"),
    ] {
        interfaces.insert(
            name.into(),
            Dependency::package(&build.target, package).unwrap(),
        );
    }
    let facts = [
        ("OS_LINUX".into(), true),
        ("HAVE_QT_WIDGETS_TEST_OWNER".into(), true),
        ("HAVE_QT_WIDGETS_TESTLIB_OWNER".into(), true),
    ]
    .into();
    let artifacts = graph.compile(&build, &root, &facts, &interfaces).unwrap();
    assert_eq!(artifacts.len(), CASES.len());
    for (name, spec) in &graph.tests {
        assert_bindings(&artifacts[name], &core_path, name);
        let result = checked(
            spec.command(&root, &build.output, &artifacts[name])
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
    write_cmake(directory.path(), &root, &core.cmake);
    replay_cmake(directory.path(), &core_path);
}

fn write_cmake(directory: &Path, root: &Path, core: &str) {
    fs::write(
        directory.join("CMakeLists.txt"),
        format!(
            r#"cmake_minimum_required(VERSION 3.28)
project(qt-widgets-contract C CXX)
set(CMAKE_CXX_STANDARD 17)
enable_testing()
option(OS_LINUX "Original Linux guard" ON)
option(OS_WINDOWS "Windows configuration" OFF)
option(OS_MACOS "macOS configuration" OFF)
option(WITH_QT "Resolve image Qt" ON)
option(WITH_QT_TEST "Original optional Qt Test" ON)
option(ENABLE_UI "UI enabled" ON)
option(ENABLE_FRONTEND "Frontend enabled" ON)
if(WITH_QT)
  find_package(Qt6 6.10 REQUIRED COMPONENTS Widgets)
  if(WITH_QT_TEST)
    find_package(Qt6 6.10 REQUIRED COMPONENTS Test)
  endif()
endif()
find_package(PkgConfig REQUIRED)
pkg_check_modules(CMOCKA REQUIRED IMPORTED_TARGET cmocka)
add_library(OBS::native-test-cmocka ALIAS PkgConfig::CMOCKA)
{core}
# Disable per-target instrumentation for this ordinary Tier 2 comparison.
function(obs_test_sanitize)
endfunction()
include("{root}/test/cmocka/native-test-graph.cmake")
obs_native_test_graph("${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json" "{root}")
"#,
            root = root.to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
}

fn replay_cmake(directory: &Path, core: &Path) {
    for (mode, options, names) in [
        ("enabled", vec![], CASES.to_vec()),
        ("windows", vec!["-DOS_LINUX=OFF", "-DOS_WINDOWS=ON"], vec![]),
        ("macos", vec!["-DOS_LINUX=OFF", "-DOS_MACOS=ON"], vec![]),
        ("no-qt", vec!["-DWITH_QT=OFF"], vec![]),
        (
            "no-qt-test",
            vec!["-DWITH_QT_TEST=OFF"],
            vec!["test_source_label_destroy"],
        ),
        ("ui-off", vec!["-DENABLE_UI=OFF"], vec![]),
        ("frontend-off", vec!["-DENABLE_FRONTEND=OFF"], vec![]),
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
                        .args(["-V", "--output-on-failure", "--no-tests=error", "-R"])
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
                test["command"],
                serde_json::json!([output.join(name).to_str().unwrap()])
            );
            assert_eq!(
                dunce::canonicalize(property("WORKING_DIRECTORY").unwrap().as_str().unwrap())
                    .unwrap(),
                dunce::canonicalize(output).unwrap()
            );
        }
    }
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
        "test_camera_busy_dialog" => 11,
        "test_dock_state_restart" => 3,
        "test_fallback_warning_dialog" => 9,
        "test_source_label_destroy" => 3,
        "test_properties_undo_coalesce" => 4,
        "test_properties_dialog_refresh" => 6,
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
    if matches!(
        name,
        "test_camera_busy_dialog" | "test_fallback_warning_dialog"
    ) {
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
