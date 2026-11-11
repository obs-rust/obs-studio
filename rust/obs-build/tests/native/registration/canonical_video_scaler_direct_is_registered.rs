use super::*;

#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
const NAMES: [&str; 1] = ["test_video_scaler_direct"];

#[test]
fn canonical_video_scaler_direct_is_registered() {
    use obs_build::validation::graph::Graph;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let raw: serde_json::Value = obs_build::validation::graph::Graph::read_value(
        &root.join("test/cmocka/build/test-graph.d"),
    )
    .unwrap();
    let name = "test_video_scaler_direct";
    assert_eq!(
        raw["targets"][name],
        serde_json::json!({
            "sources": ["test/cmocka/test_video_scaler_direct.c"],
            "libraries": ["OBS::libobs", "OBS::native-test-cmocka"],
            "requires": "OS_LINUX"
        })
    );
    assert_eq!(
        raw["tests"][name],
        serde_json::json!({
            "target": name, "requires": "OS_LINUX", "timeout_seconds": 120
        })
    );
    let fragment =
        fs::read_to_string(root.join("test/cmocka/tests.d/test_video_scaler_direct.cmake"))
            .unwrap();
    assert!(!fragment.contains("add_executable("));
    assert!(!fragment.contains("add_test("));
    let mut graph = Graph::parse(&raw.to_string()).unwrap();
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in [
        "OS_LINUX",
        "OS_MACOS",
        "OS_WINDOWS",
        "OS_FREEBSD",
        "OS_OPENBSD",
    ] {
        let facts = [("OS_LINUX".into(), os == "OS_LINUX")].into();
        let selected = graph.select(&facts).unwrap();
        assert_eq!(selected.targets.contains_key(name), os == "OS_LINUX");
        assert_eq!(selected.tests.contains_key(name), os == "OS_LINUX");
    }
}

// This smoke contract builds libobs from this checkout (OBS_GRAPH_LIBOBS). Native
// Windows/macOS execution remains the Tier 2 host runner's responsibility.
#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
#[test]
fn canonical_video_scaler_direct_runs_unchanged() {
    use obs_build::validation::graph::{Dependency, Graph};
    let root = dunce::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let raw: serde_json::Value = obs_build::validation::graph::Graph::read_value(
        &root.join("test/cmocka/build/test-graph.d"),
    )
    .unwrap();
    let mut selected = serde_json::json!({"schema": 1, "targets": {}, "tests": {}});
    for name in NAMES {
        for section in ["targets", "tests"] {
            selected[section][name] = raw[section][name].clone();
        }
    }
    let graph = Graph::parse(&selected.to_string()).unwrap();
    let build = NativeBuild {
        host: obs_build::BUILD_HOST.into(),
        target: obs_build::BUILD_TARGET.into(),
        output: directory.path().join("cargo-output"),
        profile: Default::default(),
    };
    let core = built_core(&build, &root);
    let interfaces = [
        ("OBS::libobs".into(), core.dependency),
        (
            "OBS::native-test-cmocka".into(),
            Dependency::package(&build.target, "cmocka").unwrap(),
        ),
    ]
    .into();
    let facts = [
        ("OS_LINUX".into(), true),
        ("OS_FREEBSD".into(), false),
        ("OS_OPENBSD".into(), false),
        ("OS_WINDOWS".into(), false),
    ]
    .into();
    let artifacts = graph.compile(&build, &root, &facts, &interfaces).unwrap();
    assert_eq!(artifacts.len(), NAMES.len());
    for (name, test) in graph.select(&facts).unwrap().tests {
        let output = test
            .command(&root, &build.output, &artifacts[&test.target])
            .unwrap()
            .output()
            .unwrap();
        assert_success(&output, name);
        eprintln!("{}", String::from_utf8_lossy(&output.stdout));
    }
    fs::write(directory.path().join("graph.json"), selected.to_string()).unwrap();
    replay_cmake_helpers(directory.path(), &root, &core.cmake);
}

#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
fn assert_success(output: &std::process::Output, context: &str) {
    assert!(
        output.status.success(),
        "{context}\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
fn replay_cmake_helpers(directory: &Path, root: &Path, core: &str) {
    use std::process::Command;
    let source = root.to_string_lossy().replace('\\', "/");
    fs::write(
        directory.join("CMakeLists.txt"),
        format!(
            r#"cmake_minimum_required(VERSION 3.28)
project(video-scaler-direct-contract C)
enable_testing()
set(OS_LINUX TRUE)
set(OS_WINDOWS FALSE)
set(OS_FREEBSD FALSE)
set(OS_OPENBSD FALSE)
find_package(PkgConfig REQUIRED)
pkg_check_modules(CMOCKA REQUIRED IMPORTED_TARGET cmocka)
{core}add_library(OBS::native-test-cmocka ALIAS PkgConfig::CMOCKA)
add_custom_target(cmocka-tests)
include("{source}/test/cmocka/native-test-graph.cmake")
obs_native_test_graph("${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json" "{source}")
"#
        ),
    )
    .unwrap();
    let build = directory.join("cmake-output");
    for (program, arguments) in [
        (
            "cmake",
            vec![
                "-S",
                directory.to_str().unwrap(),
                "-B",
                build.to_str().unwrap(),
            ],
        ),
        (
            "cmake",
            vec!["--build", build.to_str().unwrap(), "--parallel", "2"],
        ),
        (
            "ctest",
            vec![
                "--test-dir",
                build.to_str().unwrap(),
                "--output-on-failure",
                "--no-tests=error",
            ],
        ),
    ] {
        assert_success(
            &Command::new(program).args(arguments).output().unwrap(),
            program,
        );
    }
    let output = Command::new("ctest")
        .args(["--test-dir", build.to_str().unwrap(), "--show-only=json-v1"])
        .output()
        .unwrap();
    assert_success(&output, "CTest inventory");
    let inventory: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let tests = inventory["tests"].as_array().unwrap();
    assert_eq!(tests.len(), NAMES.len());
    for name in NAMES {
        let test = tests.iter().find(|test| test["name"] == name).unwrap();
        let timeout = test["properties"]
            .as_array()
            .unwrap()
            .iter()
            .find(|property| property["name"] == "TIMEOUT")
            .unwrap();
        assert_eq!(timeout["value"], 120.0);
        assert_eq!(test["command"].as_array().unwrap().len(), 1);
    }
}
