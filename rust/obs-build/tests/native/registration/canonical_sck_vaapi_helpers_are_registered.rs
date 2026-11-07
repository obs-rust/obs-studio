use super::*;

const HELPERS: [(&str, &str, &str); 3] = [
    ("test_sck_stall", "mac-capture", "mac-sck-stall"),
    ("test_sck_stop_wait", "mac-capture", "mac-sck-stop"),
    (
        "test_vaapi_probe_policy",
        "obs-ffmpeg",
        "vaapi-probe-policy",
    ),
];

#[test]
fn canonical_sck_vaapi_helpers_are_registered() {
    use obs_build::validation::graph::Graph;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let raw: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap())
            .unwrap();
    for (name, component, helper) in HELPERS {
        assert_helper_record(&raw, name, component, helper);
        let fragment =
            fs::read_to_string(root.join(format!("test/cmocka/tests.d/{name}.cmake"))).unwrap();
        assert!(!fragment.contains("add_executable("));
        assert!(!fragment.contains("add_test("));
    }
    let mut graph = Graph::parse(&raw.to_string()).unwrap();
    graph
        .targets
        .retain(|name, _| HELPERS.iter().any(|entry| entry.0 == name));
    graph
        .tests
        .retain(|name, _| HELPERS.iter().any(|entry| entry.0 == name));
    for os in [
        "OS_LINUX",
        "OS_MACOS",
        "OS_WINDOWS",
        "OS_FREEBSD",
        "OS_OPENBSD",
    ] {
        let facts = [
            "OS_LINUX",
            "OS_MACOS",
            "OS_WINDOWS",
            "OS_FREEBSD",
            "OS_OPENBSD",
        ]
        .map(|fact| (fact.into(), fact == os))
        .into();
        let selected = graph.select(&facts).unwrap();
        for (name, _, _) in HELPERS {
            let enabled = name != "test_vaapi_probe_policy"
                || ["OS_LINUX", "OS_FREEBSD", "OS_OPENBSD"].contains(&os);
            assert_eq!(selected.targets.contains_key(name), enabled, "{name}/{os}");
            assert_eq!(selected.tests.contains_key(name), enabled, "{name}/{os}");
        }
    }
}

fn assert_helper_record(raw: &serde_json::Value, name: &str, component: &str, helper: &str) {
    let pch = helper.strip_prefix("mac-").unwrap_or(helper);
    let mut target = serde_json::json!({
        "sources": [format!("test/cmocka/{name}.c"), format!("plugins/{component}/{helper}.c.h")],
        "includes": [format!("plugins/{component}")],
        "watch_inputs": [format!("plugins/{component}/{helper}.h"), format!("plugins/{component}/build/{component}-{pch}.pch.h")],
        "libraries": ["OBS::libobs", "OBS::native-test-cmocka"]
    });
    let mut test = serde_json::json!({"target": name});
    if name == "test_sck_stop_wait" {
        target["libraries"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"name": "OBS::w32-pthreads", "requires": "OS_WINDOWS"}));
    }
    if name == "test_vaapi_probe_policy" {
        let guard = serde_json::json!({"any": ["OS_LINUX", "OS_FREEBSD", "OS_OPENBSD"]});
        target["requires"] = guard.clone();
        test["requires"] = guard;
    }
    assert_eq!(
        raw["targets"][name], target,
        "{name} exact compilation contract"
    );
    assert_eq!(raw["tests"][name], test, "{name} exact execution contract");
}

// This smoke contract requires installed real libobs/cmocka packages. Native
// Windows/macOS execution remains the Tier 2 host runner's responsibility.
#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
#[test]
fn canonical_sck_vaapi_helpers_run_unchanged() {
    use obs_build::validation::graph::{Dependency, Graph};
    let root = dunce::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let raw: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap())
            .unwrap();
    let mut selected = serde_json::json!({"schema": 1, "targets": {}, "tests": {}});
    for (name, _, _) in HELPERS {
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
    let interfaces = [
        ("OBS::libobs".into(), core),
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
    assert_eq!(artifacts.len(), HELPERS.len());
    for (name, test) in graph.select(&facts).unwrap().tests {
        let output = test
            .command(&root, &build.output, &artifacts[&test.target])
            .unwrap()
            .output()
            .unwrap();
        assert_success(&output, name);
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
project(sck-vaapi-contract C)
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
    assert_eq!(tests.len(), HELPERS.len());
    for (name, _, _) in HELPERS {
        let test = tests.iter().find(|test| test["name"] == name).unwrap();
        assert!(
            !test["properties"]
                .as_array()
                .unwrap()
                .iter()
                .any(|property| property["name"] == "TIMEOUT")
        );
        assert_eq!(test["command"].as_array().unwrap().len(), 1);
    }
}
