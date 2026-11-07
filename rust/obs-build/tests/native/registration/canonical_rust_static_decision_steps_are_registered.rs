use super::*;

static CASES: std::sync::LazyLock<Vec<String>> =
    std::sync::LazyLock::new(|| rust_static_entries::cases("decision"));

#[test]
fn canonical_rust_static_decision_steps_are_registered() {
    let graph = rust_static_entries::assert_family("decision");
    for base in CASES.iter() {
        assert_decision_step_selection(&graph, &format!("{base}_rust"));
    }
}

fn assert_decision_step_selection(graph: &obs_build::validation::graph::Graph, name: &str) {
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        for available in [false, true] {
            let facts = [
                (os.into(), true),
                ("MSVC".into(), os == "OS_WINDOWS"),
                ("HAVE_LIBOBS_RUST_STATIC".into(), available),
            ]
            .into();
            let selected = graph.select(&facts).unwrap();
            assert_eq!(selected.targets.contains_key(name), available);
            assert_eq!(selected.tests.contains_key(name), available);
        }
    }
}

#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
fn checked(program: &str, arguments: &[&str]) -> std::process::Output {
    let result = std::process::Command::new(program)
        .args(arguments)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{program} {arguments:?}\n{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    result
}

#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
fn replay_cmake(directory: &Path, root: &Path, archive: &Path, selected: &serde_json::Value) {
    fs::write(directory.join("graph.json"), selected.to_string()).unwrap();
    let source = root.to_string_lossy().replace(char::from(92), "/");
    let archive = archive.to_string_lossy().replace(char::from(92), "/");
    fs::write(
        directory.join("CMakeLists.txt"),
        format!(
            r#"cmake_minimum_required(VERSION 3.28)
project(rust-static-decision-steps C)
set(MSVC FALSE)
enable_testing()
find_package(PkgConfig REQUIRED)
pkg_check_modules(CMOCKA REQUIRED IMPORTED_TARGET cmocka)
add_library(OBS::native-test-cmocka ALIAS PkgConfig::CMOCKA)
option(WITH_RUST "Import the real production archive" ON)
if(WITH_RUST)
  add_library(libobs_rust-static STATIC IMPORTED)
  set_target_properties(libobs_rust-static PROPERTIES IMPORTED_LOCATION "{archive}")
  if(NOT MSVC)
    target_link_libraries(libobs_rust-static INTERFACE m)
  endif()
endif()
include("{source}/test/cmocka/native-test-graph.cmake")
obs_native_test_graph("${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json" "{source}")
"#
        ),
    )
    .unwrap();
    let output = directory.join("cmake-output");
    let output = output.to_str().unwrap();
    checked("cmake", &["-S", directory.to_str().unwrap(), "-B", output]);
    checked(
        "cmake",
        &["--build", output, "--config", "Release", "--parallel", "2"],
    );
    checked(
        "ctest",
        &[
            "--test-dir",
            output,
            "-C",
            "Release",
            "--output-on-failure",
            "--no-tests=error",
        ],
    );
    for available in [false, true] {
        checked(
            "cmake",
            &[
                "-S",
                directory.to_str().unwrap(),
                "-B",
                output,
                if available {
                    "-DWITH_RUST=ON"
                } else {
                    "-DWITH_RUST=OFF"
                },
            ],
        );
        let inventory = checked("ctest", &["--test-dir", output, "--show-only=json-v1"]);
        let inventory: serde_json::Value = serde_json::from_slice(&inventory.stdout).unwrap();
        let tests = inventory["tests"].as_array().unwrap();
        assert_eq!(tests.len(), if available { CASES.len() } else { 0 });
        for test in tests {
            assert!(
                CASES
                    .iter()
                    .any(|base| test["name"] == format!("{base}_rust"))
            );
            assert!(
                !test["properties"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|p| p["name"] == "TIMEOUT")
            );
        }
    }
}

/// #1390: run unchanged archive-only characterizations through both real consumers.
#[test]
#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
fn canonical_rust_static_decision_steps_run_unchanged() {
    use obs_build::validation::graph::{Dependency, Graph};
    let root = dunce::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
    let archive = dunce::canonicalize(
        std::env::var_os("OBS_BUILD_RUST_ARCHIVE")
            .expect("set OBS_BUILD_RUST_ARCHIVE to the production staticlib"),
    )
    .unwrap();
    assert!(archive.is_file());
    let directory = tempfile::tempdir().unwrap();
    let raw: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap())
            .unwrap();
    let mut selected = serde_json::json!({"schema": 1, "targets": {}, "tests": {}});
    for base in CASES.iter() {
        for section in ["targets", "tests"] {
            let name = format!("{base}_rust");
            selected[section][&name] = raw[section][&name].clone();
        }
    }
    let graph = Graph::parse(&selected.to_string()).unwrap();
    let build = NativeBuild {
        host: obs_build::BUILD_HOST.into(),
        target: obs_build::BUILD_TARGET.into(),
        output: directory.path().join("cargo-output"),
        profile: Default::default(),
    };
    let interfaces = [
        (
            "m".into(),
            Dependency {
                link_options: vec!["-lm".into()],
                ..Default::default()
            },
        ),
        (
            "OBS::native-test-cmocka".into(),
            Dependency::package(&build.target, "cmocka").unwrap(),
        ),
        (
            "libobs_rust-static".into(),
            Dependency::rust_static(&build.target, &archive).unwrap(),
        ),
    ]
    .into();
    let facts = [
        ("HAVE_LIBOBS_RUST_STATIC".into(), true),
        ("MSVC".into(), false),
    ]
    .into();
    for target in graph.targets.values() {
        assert_eq!(
            target.dependency_artifacts(&facts, &interfaces).unwrap(),
            std::slice::from_ref(&archive)
        );
    }
    let artifacts = graph.compile(&build, &root, &facts, &interfaces).unwrap();
    assert_eq!(artifacts.len(), CASES.len());
    for (name, test) in &graph.tests {
        let result = test
            .command(&root, &build.output, &artifacts[name])
            .unwrap()
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{name}\n{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
    replay_cmake(directory.path(), &root, &archive, &selected);
}
