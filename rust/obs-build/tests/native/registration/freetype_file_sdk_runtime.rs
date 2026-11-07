use super::*;
use obs_build::validation::graph::Dependency;
use std::process::Command;

fn checked(command: &mut Command) -> std::process::Output {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{command:?}\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

#[test]
fn canonical_freetype_file_sdk_runs_unchanged() {
    let root = dunce::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let raw: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap())
            .unwrap();
    let selected = serde_json::json!({"schema": 1, "targets": {NAME: raw["targets"][NAME]}, "tests": {NAME: raw["tests"][NAME]}});
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
            "Freetype::Freetype".into(),
            Dependency::package(&build.target, "freetype2").unwrap(),
        ),
        (
            "Fontconfig::Fontconfig".into(),
            Dependency::package(&build.target, "fontconfig").unwrap(),
        ),
        (
            "OBS::native-test-cmocka".into(),
            Dependency::package(&build.target, "cmocka").unwrap(),
        ),
    ]
    .into();
    let facts = [
        ("OS_LINUX".into(), true),
        ("HAVE_OBS_TEXT_FREETYPE2".into(), true),
        ("HAVE_FREETYPE_FILE_SDK".into(), true),
    ]
    .into();
    let artifacts = graph.compile(&build, &root, &facts, &interfaces).unwrap();
    assert_eq!(artifacts.len(), 1);
    let result = checked(
        &mut graph.tests[NAME]
            .command(&root, &build.output, &artifacts[NAME])
            .unwrap(),
    );
    eprintln!("Cargo: {}", String::from_utf8_lossy(&result.stdout));
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
project(freetype-file-sdk-contract C)
set(CMAKE_C_VISIBILITY_PRESET hidden)
enable_testing()
option(OS_LINUX "Original Linux guard" ON)
option(WITH_PLUGIN "Mark the owning plugin as built" ON)
find_package(PkgConfig REQUIRED)
pkg_check_modules(CMOCKA REQUIRED IMPORTED_TARGET cmocka)
{core}add_library(OBS::native-test-cmocka ALIAS PkgConfig::CMOCKA)
if(WITH_PLUGIN)
  add_library(OBS::text-freetype2 INTERFACE IMPORTED)
endif()
include("{source}/test/cmocka/tests.d/test_text_aa_file.cmake")
include("{source}/test/cmocka/native-test-graph.cmake")
obs_native_test_graph("${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json" "{source}")
"#
        ),
    )
    .unwrap();
}

fn replay_cmake(directory: &Path) {
    for (mode, options, count) in [
        ("enabled", vec![], 1),
        ("non-linux", vec!["-DOS_LINUX=OFF"], 0),
        ("no-owner", vec!["-DWITH_PLUGIN=OFF"], 0),
        (
            "no-freetype",
            vec!["-DCMAKE_DISABLE_FIND_PACKAGE_Freetype=TRUE"],
            0,
        ),
        (
            "no-fontconfig",
            vec!["-DCMAKE_DISABLE_FIND_PACKAGE_Fontconfig=TRUE"],
            0,
        ),
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
        if count == 1 {
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
        let result = checked(
            Command::new("ctest")
                .arg("--test-dir")
                .arg(&output)
                .arg("--show-only=json-v1"),
        );
        let inventory: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        let tests = inventory["tests"].as_array().unwrap();
        assert_eq!(tests.len(), count, "{mode}");
        if let Some(test) = tests.first() {
            assert_eq!(test["name"], NAME);
            assert_eq!(test["command"].as_array().unwrap().len(), 1);
            assert_eq!(
                Path::new(test["command"][0].as_str().unwrap())
                    .file_name()
                    .unwrap(),
                NAME
            );
            assert!(
                !test["properties"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|property| property["name"] == "TIMEOUT")
            );
        }
    }
}
