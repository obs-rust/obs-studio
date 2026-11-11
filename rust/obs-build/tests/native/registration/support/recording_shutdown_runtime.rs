use super::*;
use obs_build::validation::graph::{Dependency, Graph};

fn checked(command: &mut std::process::Command) -> std::process::Output {
    let result = command
        .output()
        .unwrap_or_else(|error| panic!("{command:?}: {error}"));
    assert!(
        result.status.success(),
        "{command:?}\n{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    result
}

/// Smoke contract against the libobs built from this checkout; final-head OFF/ON parity is a rollup gate.
#[test]
fn canonical_recording_shutdown_runs_unchanged() {
    let root = dunce::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let raw: serde_json::Value = obs_build::validation::graph::Graph::read_value(
        &root.join("test/cmocka/build/test-graph.d"),
    )
    .unwrap();
    let mut selected = serde_json::json!({"schema": 1, "targets": {}, "tests": {}});
    for section in ["targets", "tests"] {
        for (name, _) in CASES {
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
            "Qt6::Core".into(),
            Dependency::package(&build.target, "Qt6Core").unwrap(),
        ),
        (
            "OBS::native-test-cmocka".into(),
            Dependency::package(&build.target, "cmocka").unwrap(),
        ),
    ]
    .into();
    let facts = [("OS_LINUX".into(), true), ("HAVE_QT6_CORE".into(), true)].into();
    let artifacts = graph.compile(&build, &root, &facts, &interfaces).unwrap();
    assert_eq!(artifacts.len(), CASES.len());
    for (name, _) in CASES {
        checked(
            &mut graph.tests[name]
                .command(&root, &build.output, &artifacts[name])
                .unwrap(),
        );
    }
    fs::write(directory.path().join("graph.json"), selected.to_string()).unwrap();
    write_cmake(directory.path(), &root, &core.cmake);
    let output = directory.path().join("cmake-output");
    checked(
        std::process::Command::new("cmake")
            .arg("-S")
            .arg(directory.path())
            .arg("-B")
            .arg(&output),
    );
    checked(
        std::process::Command::new("cmake")
            .arg("--build")
            .arg(&output)
            .args(["--parallel", "2"]),
    );
    check_inventory(&output, true);
    checked(
        std::process::Command::new("ctest")
            .arg("--test-dir")
            .arg(&output)
            .args(["--output-on-failure", "--no-tests=error"]),
    );
    for option in ["-DWITH_QT=OFF", "-DWITH_QT=ON", "-DOS_LINUX=FALSE"] {
        checked(
            std::process::Command::new("cmake")
                .arg("-S")
                .arg(directory.path())
                .arg("-B")
                .arg(&output)
                .arg(option),
        );
        check_inventory(&output, option == "-DWITH_QT=ON");
    }
}

fn write_cmake(directory: &Path, root: &Path, core: &str) {
    let source = root.to_string_lossy().replace('\\', "/");
    fs::write(
        directory.join("CMakeLists.txt"),
        format!(
            r#"cmake_minimum_required(VERSION 3.28)
project(recording-shutdown-contract C CXX)
enable_testing()
option(OS_LINUX "Select original Linux guard" ON)
option(WITH_QT "Import optional QtCore" ON)
find_package(PkgConfig REQUIRED)
pkg_check_modules(CMOCKA REQUIRED IMPORTED_TARGET cmocka)
{core}add_library(OBS::native-test-cmocka ALIAS PkgConfig::CMOCKA)
if(WITH_QT)
  find_package(Qt6 REQUIRED COMPONENTS Core)
endif()
set(HAVE_QT6_CORE FALSE)
if(TARGET Qt6::Core)
  set(HAVE_QT6_CORE TRUE)
endif()
include("{source}/test/cmocka/native-test-graph.cmake")
obs_native_test_graph("${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json" "{source}")
"#
        ),
    )
    .unwrap();
}

fn check_inventory(output: &Path, enabled: bool) {
    let result = checked(
        std::process::Command::new("ctest")
            .arg("--test-dir")
            .arg(output)
            .arg("--show-only=json-v1"),
    );
    let inventory: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    let tests = inventory["tests"].as_array().unwrap();
    assert_eq!(tests.len(), if enabled { CASES.len() } else { 0 });
    for test in tests {
        let (name, timeout) = CASES
            .iter()
            .find(|(name, _)| test["name"] == *name)
            .unwrap();
        assert_eq!(test["command"].as_array().unwrap().len(), 1);
        assert_eq!(
            Path::new(test["command"][0].as_str().unwrap())
                .file_name()
                .unwrap(),
            *name
        );
        let properties = test["properties"].as_array().unwrap();
        let actual = properties.iter().find(|p| p["name"] == "TIMEOUT").unwrap();
        assert_eq!(actual["value"].as_f64().unwrap(), f64::from(*timeout));
    }
}
