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
fn canonical_v4l2_capture_sdk_runs_unchanged() {
    let root = dunce::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let raw: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap())
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
    let mut interfaces: std::collections::BTreeMap<String, Dependency> = [
        ("OBS::libobs".into(), core.dependency),
        (
            "OBS::native-test-cmocka".into(),
            Dependency::package(&build.target, "cmocka").unwrap(),
        ),
    ]
    .into();
    for (name, package) in [
        ("FFmpeg::avcodec", "libavcodec"),
        ("FFmpeg::avformat", "libavformat"),
        ("FFmpeg::avutil", "libavutil"),
    ] {
        interfaces.insert(
            name.into(),
            Dependency::package(&build.target, package).unwrap(),
        );
    }
    let facts = [
        ("OS_LINUX".into(), true),
        ("OS_FREEBSD".into(), false),
        ("OS_OPENBSD".into(), false),
        ("HAVE_OBS_V4L2".into(), true),
        ("HAVE_V4L2_CAPTURE_SDK".into(), true),
    ]
    .into();
    let artifacts = graph.compile(&build, &root, &facts, &interfaces).unwrap();
    assert_eq!(artifacts.len(), CASES.len());
    for name in CASES {
        let result = checked(
            &mut graph.tests[name]
                .command(&root, &build.output, &artifacts[name])
                .unwrap(),
        );
        eprintln!("Cargo {name}: {}", String::from_utf8_lossy(&result.stdout));
    }
    fs::write(directory.path().join("graph.json"), selected.to_string()).unwrap();
    write_cmake(directory.path(), &root, &core.cmake);
    replay_cmake(directory.path());
}

fn write_cmake(directory: &Path, root: &Path, core: &str) {
    let source = root.to_string_lossy().replace('\\', "/");
    fs::write(
        directory.join("CMakeLists.txt"),
        format!(
            r#"cmake_minimum_required(VERSION 3.28)
project(v4l2-capture-sdk-contract C)
set(CMAKE_C_VISIBILITY_PRESET hidden)
enable_testing()
option(OS_LINUX "Original Linux guard" ON)
option(OS_FREEBSD "Original FreeBSD guard" OFF)
option(OS_OPENBSD "Original OpenBSD guard" OFF)
option(WITH_PLUGIN "Mark the owning plugin as built" ON)
option(WITHOUT_PKG_CONFIG "Disable optional pkg-config discovery after importing core" OFF)
find_package(PkgConfig REQUIRED)
pkg_check_modules(CMOCKA REQUIRED IMPORTED_TARGET cmocka)
{core}add_library(OBS::native-test-cmocka ALIAS PkgConfig::CMOCKA)
if(WITH_PLUGIN)
  add_library(OBS::v4l2 INTERFACE IMPORTED)
endif()
if(WITHOUT_PKG_CONFIG)
  set(CMAKE_DISABLE_FIND_PACKAGE_PkgConfig TRUE)
  set(PKG_CONFIG_FOUND FALSE)
endif()
list(APPEND CMAKE_MODULE_PATH "{source}/cmake/finders")
include("{source}/test/cmocka/tests.d/test_v4l2_controls.cmake")
include("{source}/test/cmocka/tests.d/test_v4l2_corrupt_frame.cmake")
include("{source}/test/cmocka/tests.d/test_v4l2_dqbuf_error.cmake")
include("{source}/test/cmocka/tests.d/test_v4l2_framerate.cmake")
set(HAVE_OBS_V4L2 FALSE)
if(TARGET OBS::v4l2)
  set(HAVE_OBS_V4L2 TRUE)
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
        ("freebsd", vec!["-DOS_LINUX=OFF", "-DOS_FREEBSD=ON"], vec![]),
        ("no-owner", vec!["-DWITH_PLUGIN=OFF"], vec![]),
        (
            "no-ffmpeg",
            vec!["-DCMAKE_DISABLE_FIND_PACKAGE_FFmpeg=TRUE"],
            vec![],
        ),
        ("no-sdk", vec!["-DWITHOUT_PKG_CONFIG=ON"], vec![]),
    ] {
        let output = directory.join(mode);
        checked(
            Command::new("cmake")
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
        assert_eq!(
            timeout.unwrap()["value"],
            if name == "test_v4l2_dqbuf_error" {
                90.0
            } else {
                60.0
            }
        );
    }
}
