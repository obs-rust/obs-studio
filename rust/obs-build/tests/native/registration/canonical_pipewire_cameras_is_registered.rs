use super::*;
use obs_build::validation::graph::Graph;

const CASES: [&str; 6] = [
    "test_pipewire_camera_control_refresh",
    "test_pipewire_camera_format_dedup",
    "test_pipewire_camera_nv12",
    "test_pipewire_camera_renegotiate",
    "test_pipewire_camera_resolution_restart",
    "test_pipewire_camera_unload",
];

#[test]
fn canonical_pipewire_cameras_is_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let graph = Graph::load(&root.join("test/cmocka/build/test-graph.d")).unwrap();
    let raw: serde_json::Value = obs_build::validation::graph::Graph::read_value(
        &root.join("test/cmocka/build/test-graph.d"),
    )
    .unwrap();
    let wanted = expected();
    for section in ["targets", "tests"] {
        for name in CASES {
            assert_eq!(
                raw[section][name], wanted[section][name],
                "{section}/{name}"
            );
        }
    }
    for name in CASES {
        assert!(graph.targets.contains_key(name), "missing target {name}");
        assert!(graph.tests.contains_key(name), "missing test {name}");
        let fragment =
            fs::read_to_string(root.join(format!("test/cmocka/tests.d/{name}.cmake"))).unwrap();
        assert!(fragment.contains("test-graph.d"), "{name}");
        assert!(!fragment.contains("add_executable("), "{name}");
        assert!(!fragment.contains("add_test("), "{name}");
    }
}

#[test]
fn canonical_pipewire_cameras_optional_owner() {
    let graph = Graph::parse(r#"{"schema":1,"targets":{"camera":{"sources":["test.c"],"requires":"HAVE_PIPEWIRE_CAMERA_TEST_OWNER"}},"tests":{"camera":{"target":"camera","requires":"HAVE_PIPEWIRE_CAMERA_TEST_OWNER"}}}"#).unwrap();
    assert!(
        graph
            .select(&Default::default())
            .unwrap()
            .targets
            .is_empty()
    );
}

fn expected() -> serde_json::Value {
    serde_json::from_str(include_str!("pipewire_cameras_expected.json")).unwrap()
}

#[test]
fn canonical_pipewire_cameras_guards() {
    let graph = Graph::parse(&expected().to_string()).unwrap();
    for linux in [false, true] {
        for owner in [None, Some(false), Some(true)] {
            for display in [None, Some(false), Some(true)] {
                let mut facts = std::collections::BTreeMap::from([("OS_LINUX".into(), linux)]);
                if let Some(value) = owner {
                    facts.insert("HAVE_PIPEWIRE_CAMERA_TEST_OWNER".into(), value);
                }
                if let Some(value) = display {
                    facts.insert("HAVE_X11_DISPLAY".into(), value);
                }
                let selected = graph.select(&facts).unwrap();
                let count = if linux && owner == Some(true) && display == Some(true) {
                    6
                } else {
                    0
                };
                assert_eq!(selected.targets.len(), count);
                assert_eq!(selected.tests.len(), count);
            }
        }
    }
}

#[test]
fn canonical_pipewire_cameras_cmake_unavailable() {
    let root = dunce::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("graph.json"), expected().to_string()).unwrap();
    fs::write(
        directory.path().join("CMakeLists.txt"),
        format!(
            r#"cmake_minimum_required(VERSION 3.28)
project(camera-unavailable NONE)
enable_testing()
option(OS_LINUX "Linux" ON)
set(HAVE_X11_DISPLAY TRUE)
include("{root}/test/cmocka/native-test-graph.cmake")
obs_native_test_graph("${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json" "{root}")
"#,
            root = root.to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
    for (name, options) in [
        ("no-owner", vec![]),
        ("no-daemons", vec!["-DHAVE_PIPEWIRE_CAMERA_TEST_OWNER=OFF"]),
        ("windows", vec!["-DOS_LINUX=OFF", "-DOS_WINDOWS=ON"]),
        ("macos", vec!["-DOS_LINUX=OFF", "-DOS_MACOS=ON"]),
    ] {
        let output = directory.path().join(name);
        let result = std::process::Command::new("cmake")
            .arg("-S")
            .arg(directory.path())
            .arg("-B")
            .arg(&output)
            .args(options)
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
#[path = "support/pipewire_cameras_runtime.rs"]
mod runtime;
