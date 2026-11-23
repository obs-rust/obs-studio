use super::*;
use obs_build::validation::graph::Dependency;
use std::process::Command;

fn checked(command: &mut Command) -> std::process::Output {
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
fn canonical_plugin_sdk_headless_runs_unchanged() {
    let root = dunce::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let raw: serde_json::Value = obs_build::validation::graph::Graph::read_value(
        &root.join("test/cmocka/build/test-graph.d"),
    )
    .unwrap();
    let mut selected = serde_json::json!({"schema": 1, "targets": {}, "tests": {}});
    for section in ["targets", "tests"] {
        for name in CASES {
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
            "PipeWire::PipeWire".into(),
            Dependency::package(&build.target, "libpipewire-0.3").unwrap(),
        ),
        (
            "PulseAudio::PulseAudio".into(),
            Dependency::package(&build.target, "libpulse").unwrap(),
        ),
        (
            "OBS::native-test-cmocka".into(),
            Dependency::package(&build.target, "cmocka").unwrap(),
        ),
    ]
    .into();
    let facts = [
        "OS_LINUX",
        "HAVE_OBS_PIPEWIRE",
        "HAVE_OBS_PULSEAUDIO",
        "HAVE_PIPEWIRE_FRAME_LAYOUT_SDK",
        "HAVE_PULSE_DEFAULT_RETRY_SDK",
    ]
    .into_iter()
    .map(|name| (name.into(), true))
    .collect();
    let artifacts = graph.compile(&build, &root, &facts, &interfaces).unwrap();
    assert_eq!(artifacts.len(), CASES.len());
    for name in CASES {
        let output = checked(
            &mut graph.tests[name]
                .command(&root, &build.output, &artifacts[name])
                .unwrap(),
        );
        eprintln!("Cargo {name}: {}", String::from_utf8_lossy(&output.stdout));
    }
    fs::write(directory.path().join("graph.json"), selected.to_string()).unwrap();
    write_cmake(directory.path(), &root, &core.cmake);
    replay_cmake(directory.path());
}

fn write_cmake(directory: &Path, root: &Path, core: &str) {
    let source = root.to_string_lossy().replace('\\', "/");
    // Nix and cross SDK wrappers can restore role/target-specific paths.
    // Clear every such input inside this one CMake absence fixture only.
    let clear_pkg_paths: String = std::env::vars_os()
        .filter_map(|(name, _)| name.into_string().ok())
        .filter(|name| name.starts_with("PKG_CONFIG_PATH"))
        .filter(|name| {
            name.bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        })
        .map(|name| format!("  set(ENV{{{name}}} \"\")\n"))
        .collect();
    fs::write(
        directory.join("CMakeLists.txt"),
        format!(
            r#"cmake_minimum_required(VERSION 3.28)
project(plugin-sdk-headless-contract C)
set(CMAKE_C_VISIBILITY_PRESET hidden)
enable_testing()
option(OS_LINUX "Select original Linux guard" ON)
option(WITH_PLUGINS "Mark the owning plugins as built" ON)
option(WITHOUT_TEST_HEADERS "Search an empty pkg-config SDK directory" OFF)
find_package(PkgConfig REQUIRED)
{core}pkg_check_modules(CMOCKA REQUIRED IMPORTED_TARGET cmocka)
add_library(OBS::native-test-cmocka ALIAS PkgConfig::CMOCKA)
if(WITH_PLUGINS)
  foreach(plugin IN ITEMS pipewire pulseaudio)
    add_library(OBS::${{plugin}} INTERFACE IMPORTED)
  endforeach()
endif()
if(WITHOUT_TEST_HEADERS)
  set(PKG_CONFIG_USE_CMAKE_PREFIX_PATH FALSE)
  file(MAKE_DIRECTORY "${{CMAKE_CURRENT_BINARY_DIR}}/no-pkg-config")
  set(ENV{{PKG_CONFIG_PATH}} "")
{clear_pkg_paths}
  set(ENV{{PKG_CONFIG_LIBDIR}} "${{CMAKE_CURRENT_BINARY_DIR}}/no-pkg-config")
endif()
list(APPEND CMAKE_MODULE_PATH "{source}/cmake/finders")
include("{source}/test/cmocka/tests.d/test_pipewire_frame_layout.cmake")
include("{source}/test/cmocka/tests.d/test_pulse_default_retry.cmake")
set(HAVE_OBS_PIPEWIRE FALSE)
set(HAVE_OBS_PULSEAUDIO FALSE)
if(TARGET OBS::pipewire)
  set(HAVE_OBS_PIPEWIRE TRUE)
endif()
if(TARGET OBS::pulseaudio)
  set(HAVE_OBS_PULSEAUDIO TRUE)
endif()
include("{source}/test/cmocka/native-test-graph.cmake")
obs_native_test_graph("${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json" "{source}")
"#
        ),
    )
    .unwrap();
}

fn replay_cmake(directory: &Path) {
    for (mode, options, expected) in [
        ("enabled", vec![], CASES.to_vec()),
        ("non-linux", vec!["-DOS_LINUX=OFF"], vec![]),
        ("no-owner", vec!["-DWITH_PLUGINS=OFF"], vec![]),
        (
            "no-pulse",
            vec!["-DCMAKE_DISABLE_FIND_PACKAGE_PulseAudio=TRUE"],
            vec![CASES[0]],
        ),
        (
            "no-pipewire",
            vec!["-DCMAKE_DISABLE_FIND_PACKAGE_PipeWire=TRUE"],
            vec![CASES[1]],
        ),
        (
            "no-headers",
            vec!["-DWITHOUT_TEST_HEADERS=ON"],
            vec![CASES[1]],
        ),
    ] {
        let output = directory.join(mode);
        checked(
            Command::new("cmake")
                .arg("-DCMAKE_BUILD_TYPE=RelWithDebInfo")
                .arg("-S")
                .arg(directory)
                .arg("-B")
                .arg(&output)
                .args(options),
        );
        if mode == "enabled" {
            checked(
                Command::new("cmake")
                    .arg("--build")
                    .arg(&output)
                    .args(["--parallel", "2"]),
            );
            let result = checked(Command::new("ctest").arg("--test-dir").arg(&output).args([
                "--output-on-failure",
                "--no-tests=error",
                "-V",
            ]));
            eprintln!("CMake: {}", String::from_utf8_lossy(&result.stdout));
        }
        check_inventory(&output, &expected, mode == "enabled");
    }
}

fn check_inventory(output: &Path, expected: &[&str], built: bool) {
    let result = checked(
        Command::new("ctest")
            .arg("--test-dir")
            .arg(output)
            .arg("--show-only=json-v1"),
    );
    let inventory: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    let tests = inventory["tests"].as_array().unwrap();
    assert_eq!(tests.len(), expected.len(), "{}", output.display());
    for test in tests {
        let name = test["name"].as_str().unwrap();
        assert!(expected.contains(&name));
        if built {
            assert_eq!(test["command"].as_array().unwrap().len(), 1);
            assert_eq!(
                Path::new(test["command"][0].as_str().unwrap())
                    .file_name()
                    .unwrap(),
                name
            );
        }
        let timeout = test["properties"]
            .as_array()
            .unwrap()
            .iter()
            .find(|property| property["name"] == "TIMEOUT");
        if name == CASES[1] {
            assert_eq!(timeout.unwrap()["value"], 10.0);
        } else {
            assert!(timeout.is_none());
        }
    }
}
