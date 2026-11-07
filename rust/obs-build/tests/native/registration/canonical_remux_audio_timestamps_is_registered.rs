use super::*;

#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
const NAMES: [&str; 1] = ["test_remux_audio_timestamps"];

#[test]
fn canonical_remux_audio_timestamps_is_registered() {
    use obs_build::validation::graph::Graph;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let raw: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap())
            .unwrap();
    let name = "test_remux_audio_timestamps";
    assert_eq!(
        raw["targets"][name],
        serde_json::json!({
            "sources": ["test/cmocka/test_remux_audio_timestamps.c"],
            "libraries": ["OBS::libobs", "FFmpeg::avcodec", "FFmpeg::avformat", "FFmpeg::avutil", "OBS::native-test-cmocka", {"name": "m", "requires": {"not": "OS_WINDOWS"}}]
        })
    );
    assert_eq!(
        raw["tests"][name],
        serde_json::json!({
            "target": name, "timeout_seconds": 60
        })
    );
    let fragment =
        fs::read_to_string(root.join("test/cmocka/tests.d/test_remux_audio_timestamps.cmake"))
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
        let facts = [("OS_WINDOWS".into(), os == "OS_WINDOWS")].into();
        let selected = graph.select(&facts).unwrap();
        assert!(selected.targets.contains_key(name));
        assert!(selected.tests.contains_key(name));
    }
}

// This smoke contract requires installed real libobs/cmocka packages. Native
// Windows/macOS execution remains the Tier 2 host runner's responsibility.
#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
#[test]
fn canonical_remux_audio_timestamps_runs_unchanged() {
    use obs_build::validation::graph::{Dependency, Graph};
    let root = dunce::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let raw: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap())
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
    let mut core = Dependency::package(&build.target, "libobs").unwrap();
    // Some installed libobs.pc files name include/ instead of include/obs/.
    let nested: Vec<_> = core
        .include_dirs
        .iter()
        .map(|path| path.join("obs"))
        .filter(|path| path.is_dir())
        .collect();
    core.include_dirs.extend(nested);
    let mut interfaces: std::collections::BTreeMap<String, Dependency> = [
        ("OBS::libobs".into(), core),
        (
            "OBS::native-test-cmocka".into(),
            Dependency::package(&build.target, "cmocka").unwrap(),
        ),
    ]
    .into();
    for component in ["avcodec", "avformat", "avutil"] {
        interfaces.insert(
            format!("FFmpeg::{component}"),
            Dependency::package(&build.target, &format!("lib{component}")).unwrap(),
        );
    }
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
    replay_cmake_helpers(directory.path(), &root);
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
fn replay_cmake_helpers(directory: &Path, root: &Path) {
    use std::process::Command;
    let source = root.to_string_lossy().replace('\\', "/");
    fs::write(
        directory.join("CMakeLists.txt"),
        format!(
            r#"cmake_minimum_required(VERSION 3.28)
project(remux-audio-timestamps-contract C)
enable_testing()
set(OS_LINUX TRUE)
set(OS_WINDOWS FALSE)
set(OS_FREEBSD FALSE)
set(OS_OPENBSD FALSE)
find_package(PkgConfig REQUIRED)
pkg_check_modules(CORE REQUIRED IMPORTED_TARGET libobs)
pkg_check_modules(CMOCKA REQUIRED IMPORTED_TARGET cmocka)
add_library(OBS::libobs ALIAS PkgConfig::CORE)
foreach(dir IN LISTS CORE_INCLUDE_DIRS)
  if(IS_DIRECTORY "${{dir}}/obs")
    target_include_directories(PkgConfig::CORE INTERFACE "${{dir}}/obs")
  endif()
endforeach()
add_library(OBS::native-test-cmocka ALIAS PkgConfig::CMOCKA)
foreach(component avcodec avformat avutil)
  pkg_check_modules(${{component}} REQUIRED IMPORTED_TARGET lib${{component}})
  add_library(FFmpeg::${{component}} ALIAS PkgConfig::${{component}})
endforeach()
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
        assert_eq!(timeout["value"], 60.0);
        assert_eq!(test["command"].as_array().unwrap().len(), 1);
    }
}
