use super::*;
use obs_build::validation::graph::Dependency;
use std::process::Command;
fn checked(command: &mut Command) -> std::process::Output {
    let result = command.output().unwrap();
    assert!(
        result.status.success(),
        "{command:?}\n{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    result
}
fn cases(output: &std::process::Output, name: &str) {
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
    .replace("\r\n", "\n");
    let count = 5;
    assert!(
        text.contains(&format!("[  PASSED  ] {count} test(s).")),
        "{name}: {text}"
    );
    assert!(!text.contains("[  SKIPPED ]"), "{name}: {text}");
    assert!(!text.contains("[  FAILED  ]"), "{name}: {text}");
}
fn unavailable_owners(directory: &Path, root: &str) {
    let absent = directory.join("absent-sources");
    fs::create_dir_all(&absent).unwrap();
    for (mode, source_root, qt, expected_names) in [
        ("no-source", absent.as_path(), "ON", vec![]),
        ("no-qt", Path::new(root), "OFF", vec!["test_ws_image_mime"]),
    ] {
        let output = directory.join(mode);
        checked(
            Command::new("cmake")
                .arg("-S")
                .arg(directory)
                .arg("-B")
                .arg(&output)
                .args(["-G", "Ninja"])
                .arg(format!("-DWITH_QT={qt}"))
                .arg(format!(
                    "-DGRAPH_ROOT={}",
                    source_root.to_string_lossy().replace('\\', "/")
                )),
        );
        let inventory = checked(
            Command::new("ctest")
                .arg("--test-dir")
                .arg(&output)
                .arg("--show-only=json-v1"),
        );
        let inventory: serde_json::Value = serde_json::from_slice(&inventory.stdout).unwrap();
        let names: Vec<_> = inventory["tests"]
            .as_array()
            .unwrap()
            .iter()
            .map(|test| test["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, expected_names);
    }
}

#[test]
fn canonical_websocket_helpers_run_unchanged() {
    let root = dunce::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let build = NativeBuild {
        host: obs_build::BUILD_HOST.into(),
        target: obs_build::BUILD_TARGET.into(),
        output: directory.path().join("native"),
        profile: Default::default(),
    };
    let interfaces = std::collections::BTreeMap::from([
        (
            "OBS::native-test-cmocka".into(),
            Dependency::package(&build.target, "cmocka").unwrap(),
        ),
        (
            "Qt6::Core".into(),
            Dependency::package(&build.target, "Qt6Core").unwrap(),
        ),
    ]);
    let facts = std::collections::BTreeMap::from([
        ("HAVE_WS_IMAGE_MIME_TEST_OWNER".into(), true),
        ("HAVE_WS_SESSION_DURATION_TEST_OWNER".into(), true),
    ]);
    let graph = Graph::parse(&expected().to_string()).unwrap();
    let artifacts = graph.compile(&build, &root, &facts, &interfaces).unwrap();
    assert_eq!(artifacts.len(), CASES.len());
    for name in CASES {
        let test = graph.tests[name].resolve_runtime(&interfaces).unwrap();
        cases(
            &checked(
                &mut test
                    .command(&root, &build.output, &artifacts[name])
                    .unwrap(),
            ),
            name,
        );
    }
    fs::write(directory.path().join("graph.json"), expected().to_string()).unwrap();
    let root = root.to_string_lossy().replace('\\', "/");
    fs::write(
        directory.path().join("CMakeLists.txt"),
        format!(
            r#"cmake_minimum_required(VERSION 3.28)
project(websocket-helper-contract C CXX)
enable_testing()
find_package(PkgConfig REQUIRED)
pkg_check_modules(CMOCKA REQUIRED IMPORTED_TARGET cmocka)
add_library(OBS::native-test-cmocka ALIAS PkgConfig::CMOCKA)
option(WITH_QT "Optional Qt owner" ON)
if(WITH_QT)
  find_package(Qt6 QUIET COMPONENTS Core)
endif()
set(GRAPH_ROOT "{root}" CACHE PATH "Fixture source root")
include("{root}/test/cmocka/native-test-graph.cmake")
obs_native_test_graph("${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json" "${{GRAPH_ROOT}}")
"#
        ),
    )
    .unwrap();
    let output = directory.path().join("cmake");
    checked(
        Command::new("cmake")
            .arg("-S")
            .arg(directory.path())
            .arg("-B")
            .arg(&output)
            .args(["-G", "Ninja"]),
    );
    checked(
        Command::new("cmake")
            .arg("--build")
            .arg(&output)
            .args(["--parallel", "4"]),
    );
    for name in CASES {
        cases(
            &checked(
                Command::new("ctest")
                    .arg("--test-dir")
                    .arg(&output)
                    .args(["-V", "--no-tests=error", "-R"])
                    .arg(format!("^{name}$")),
            ),
            name,
        );
    }
    unavailable_owners(directory.path(), &root);
}
