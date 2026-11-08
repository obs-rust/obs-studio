use super::*;
use obs_build::validation::graph::Graph;
const CASES: [&str; 6] = [
    "test_fallback_warning_queue", "test_plugin_manager", "test_vst_editor_close",
    "test_pipewire_context_fail", "test_pulse_monitor_device", "test_pulse_two_sources",
];
fn expected() -> serde_json::Value {
    serde_json::from_str(include_str!("qt_daemons_expected.json")).unwrap()
}
#[test]
fn canonical_qt_daemons_is_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let raw: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap())
            .unwrap();
    let wanted = expected();
    for section in ["targets", "tests"] {
        for (name, record) in wanted[section].as_object().unwrap() {
            assert_eq!(raw[section][name], *record, "{section}/{name}");
        }
    }
    for name in CASES {
        let fragment =
            fs::read_to_string(root.join(format!("test/cmocka/tests.d/{name}.cmake"))).unwrap();
        assert!(fragment.contains("test-graph.json"));
        assert!(!fragment.contains("add_executable("), "{name}");
        assert!(!fragment.contains("add_test("), "{name}");
    }
}
#[test]
fn canonical_qt_daemons_guards() {
    let graph = Graph::parse(&expected().to_string()).unwrap();
    let owners = ["HAVE_QT_WIDGETS_TEST_OWNER", "HAVE_QT_WIDGETS_TESTLIB_OWNER", "HAVE_PLUGIN_MANAGER_TEST_OWNER", "HAVE_VST_EDITOR_TEST_OWNER", "HAVE_PRIVATE_DAEMON_TEST_OWNER"];
    for linux in [false, true] {
        for disabled in 0..=owners.len() {
            let mut facts = std::collections::BTreeMap::from([
                ("OS_LINUX".into(), linux), ("HAVE_OBS_PIPEWIRE".into(), true),
                ("ENABLE_PULSEAUDIO".into(), true), ("HAVE_OBS_PULSEAUDIO".into(), true), ("HAVE_X11_DISPLAY".into(), true),
            ]);
            for (index, name) in owners.iter().enumerate() {
                if index != disabled { facts.insert((*name).into(), true); }
            }
            let selected = graph.select(&facts).unwrap();
            for (name, record) in expected()["targets"].as_object().unwrap() {
                let enabled = linux && !record["requires"]["all"].as_array().unwrap().iter().any(|item| disabled < owners.len() && item == owners[disabled]);
                assert_eq!(selected.targets.contains_key(name.as_str()), enabled, "{linux}/{disabled}/{name}");
                assert_eq!(selected.tests.contains_key(name.as_str()), enabled, "{name}");
            }
        }
    }
}
#[test]
fn canonical_qt_daemons_cmake_without_qt() {
    let root = dunce::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("graph.json"), expected().to_string()).unwrap();
    fs::write(
        directory.path().join("CMakeLists.txt"),
        format!(
            r#"cmake_minimum_required(VERSION 3.28)
project(qt-disabled-contract NONE)
enable_testing()
set(OS_LINUX TRUE)
set(HAVE_OBS_PIPEWIRE FALSE)
set(HAVE_OBS_PULSEAUDIO FALSE)
set(ENABLE_PULSEAUDIO FALSE)
set(HAVE_PRIVATE_DAEMON_TEST_OWNER FALSE)
if(DECLARE_QT)
  add_library(Qt6::Widgets INTERFACE IMPORTED)
  add_library(Qt6::Test INTERFACE IMPORTED)
  add_library(Qt6::Svg INTERFACE IMPORTED)
  add_library(OBS::idian INTERFACE IMPORTED)
  add_library(nlohmann_json::nlohmann_json INTERFACE IMPORTED)
  add_library(test_vst_editor_close_module INTERFACE IMPORTED)
  add_library(test_vst_editor_close_plugin INTERFACE IMPORTED)
endif()
include("{root}/test/cmocka/native-test-graph.cmake")
obs_native_test_graph("${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json" "{root}")
"#,
            root = root.to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
    for (name, arguments) in [
        ("absent", vec![]),
        ("ui-off", vec!["-DDECLARE_QT=ON", "-DENABLE_UI=OFF"]),
        (
            "frontend-off",
            vec!["-DDECLARE_QT=ON", "-DENABLE_FRONTEND=OFF"],
        ),
    ] {
        let output = directory.path().join(name);
        let result = std::process::Command::new("cmake")
            .arg("-S")
            .arg(directory.path())
            .arg("-B")
            .arg(&output)
            .args(arguments)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let result = std::process::Command::new("ctest")
            .arg("--test-dir")
            .arg(output)
            .arg("--show-only=json-v1")
            .output()
            .unwrap();
        assert!(result.status.success());
        let inventory: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        assert!(inventory["tests"].as_array().unwrap().is_empty());
    }
}

#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
#[path = "qt_daemons_runtime.rs"]
mod runtime;
