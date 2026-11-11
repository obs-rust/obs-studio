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
fn canonical_wayland_sdk_runs_unchanged() {
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
    let caption = obs_build::libobs::build_caption(&build, &root).unwrap();
    let interfaces = [
        ("OBS::libobs".into(), core.dependency),
        (
            "OBS::caption".into(),
            Dependency {
                artifacts: vec![caption.clone()],
                include_dirs: vec![root.join("deps/libcaption")],
                ..Default::default()
            },
        ),
        (
            "Wayland::Client".into(),
            Dependency::package(&build.target, "wayland-client").unwrap(),
        ),
        (
            "Wayland::Server".into(),
            Dependency::package(&build.target, "wayland-server").unwrap(),
        ),
        (
            "xkbcommon::xkbcommon".into(),
            Dependency::package(&build.target, "xkbcommon").unwrap(),
        ),
        (
            "OBS::native-test-cmocka".into(),
            Dependency::package(&build.target, "cmocka").unwrap(),
        ),
    ]
    .into();
    let facts = [
        ("OS_LINUX".into(), true),
        ("OS_FREEBSD".into(), false),
        ("ENABLE_WAYLAND".into(), true),
        ("HAVE_WAYLAND_CLIENT_TEST_SDK".into(), true),
        ("HAVE_WAYLAND_SERVER_TEST_SDK".into(), true),
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
    write_cmake(directory.path(), &root, &core.cmake, &caption);
    replay_cmake(directory.path());
}

fn write_cmake(directory: &Path, root: &Path, core: &str, caption: &Path) {
    let source = root.to_string_lossy().replace('\\', "/");
    let caption = caption.to_string_lossy().replace('\\', "/");
    fs::write(directory.join("CMakeLists.txt"), format!(r#"cmake_minimum_required(VERSION 3.28)
project(wayland-sdk-contract C)
set(CMAKE_C_VISIBILITY_PRESET hidden)
enable_testing()
option(OS_LINUX "Linux guard" ON)
option(OS_FREEBSD "FreeBSD guard" OFF)
option(ENABLE_WAYLAND "Original feature guard" ON)
find_package(PkgConfig REQUIRED)
pkg_check_modules(CMOCKA REQUIRED IMPORTED_TARGET cmocka)
{core}add_library(OBS::native-test-cmocka ALIAS PkgConfig::CMOCKA)
add_library(OBS::caption STATIC IMPORTED)
set_target_properties(OBS::caption PROPERTIES IMPORTED_LOCATION "{caption}" INTERFACE_INCLUDE_DIRECTORIES "{source}/deps/libcaption")
find_package(ECM REQUIRED NO_MODULE)
list(APPEND CMAKE_MODULE_PATH "{source}/cmake/finders" "${{ECM_FIND_MODULE_DIR}}")
include("{source}/test/cmocka/tests.d/test_wayland_keymap.cmake")
include("{source}/test/cmocka/tests.d/test_wayland_volume_keys.cmake")
include("{source}/test/cmocka/tests.d/test_wayland_multiseat.cmake")
include("{source}/test/cmocka/native-test-graph.cmake")
obs_native_test_graph("${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json" "{source}")
"#)).unwrap();
}

fn replay_cmake(directory: &Path) {
    for (mode, options, expected) in [
        ("enabled", vec![], 3),
        ("non-linux", vec!["-DOS_LINUX=OFF"], 0),
        ("freebsd", vec!["-DOS_LINUX=OFF", "-DOS_FREEBSD=ON"], 3),
        ("feature-off", vec!["-DENABLE_WAYLAND=OFF"], 0),
        (
            "no-wayland",
            vec!["-DCMAKE_DISABLE_FIND_PACKAGE_Wayland=TRUE"],
            0,
        ),
        (
            "no-xkbcommon",
            vec!["-DCMAKE_DISABLE_FIND_PACKAGE_Xkbcommon=TRUE"],
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
        check_inventory(&output, expected, mode == "enabled");
    }
}

fn check_inventory(output: &Path, expected: usize, built: bool) {
    let result = checked(
        Command::new("ctest")
            .arg("--test-dir")
            .arg(output)
            .arg("--show-only=json-v1"),
    );
    let inventory: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    let tests = inventory["tests"].as_array().unwrap();
    assert_eq!(tests.len(), expected, "{}", output.display());
    for test in tests {
        let name = test["name"].as_str().unwrap();
        assert!(CASES.contains(&name));
        if built {
            assert_eq!(
                test["command"],
                serde_json::json!([output.join(name).to_string_lossy()])
            );
        }
        assert!(
            !test["properties"]
                .as_array()
                .unwrap()
                .iter()
                .any(|p| p["name"] == "TIMEOUT")
        );
    }
}
