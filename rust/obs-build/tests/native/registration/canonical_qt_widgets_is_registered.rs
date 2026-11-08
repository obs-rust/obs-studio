use super::*;
use obs_build::validation::graph::Graph;
const CASES: [&str; 6] = [
    "test_camera_busy_dialog",
    "test_dock_state_restart",
    "test_fallback_warning_dialog",
    "test_source_label_destroy",
    "test_properties_dialog_refresh",
    "test_properties_undo_coalesce",
];
fn expected() -> serde_json::Value {
    serde_json::from_str(include_str!("qt_widgets_expected.json")).unwrap()
}
#[test]
fn canonical_qt_widgets_is_registered() {
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
fn canonical_qt_widgets_guards() {
    let graph = Graph::parse(&expected().to_string()).unwrap();
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        for widgets in [None, Some(false), Some(true)] {
            for testlib in [None, Some(false), Some(true)] {
                let mut facts =
                    std::collections::BTreeMap::from([("OS_LINUX".into(), os == "OS_LINUX")]);
                for (name, value) in [
                    ("HAVE_QT_WIDGETS_TEST_OWNER", widgets),
                    ("HAVE_QT_WIDGETS_TESTLIB_OWNER", testlib),
                ] {
                    if let Some(value) = value {
                        facts.insert(name.into(), value);
                    }
                }
                let selected = graph.select(&facts).unwrap();
                for name in CASES {
                    let enabled = os == "OS_LINUX"
                        && widgets == Some(true)
                        && (name == "test_source_label_destroy" || testlib == Some(true));
                    assert_eq!(
                        selected.targets.contains_key(name),
                        enabled,
                        "{os}/{widgets:?}/{testlib:?}/{name}"
                    );
                    assert_eq!(selected.tests.contains_key(name), enabled, "{name}");
                }
            }
        }
    }
}
#[test]
fn canonical_qt_widgets_cmake_without_qt() {
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
if(DECLARE_QT)
  add_library(Qt6::Widgets INTERFACE IMPORTED)
  add_library(Qt6::Test INTERFACE IMPORTED)
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
#[path = "qt_widgets_runtime.rs"]
mod runtime;
