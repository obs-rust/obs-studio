use super::*;
use obs_build::validation::graph::Dependency;
use std::process::Command;

fn checked(command: &mut Command) -> std::process::Output {
    let output = command
        .output()
        .unwrap_or_else(|e| panic!("{command:?}: {e}"));
    assert!(
        output.status.success(),
        "{command:?}\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}
fn assert_cases(output: &std::process::Output, name: &str) {
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !text.lines().any(|line| line.trim_start().starts_with('[')
            && line.to_ascii_lowercase().contains("skipped")),
        "{text}"
    );
    let count = match name {
        "test_wayland_hotkeys" | "test_lua_graphics_lock" => 8,
        "test_service_state" => 1,
        "test_lua_graphics_lock_bounded" => 5,
        "test_vcam_audio_config" | "test_virtual_cam_shutdown" => 0,
        _ => panic!("unknown fixture"),
    };
    if count > 0 {
        assert_eq!(
            text.matches(&format!("[  PASSED  ] {count} test(s)."))
                .count(),
            1,
            "{text}"
        );
    }
    eprintln!("{name}: {text}");
}
#[test]
fn canonical_headless_characterizations_run_unchanged() {
    let root = dunce::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let raw: serde_json::Value = obs_build::validation::graph::Graph::read_value(
        &root.join("test/cmocka/build/test-graph.d"),
    )
    .unwrap();
    let mut selected = serde_json::json!({"schema":1,"targets":{},"tests":{}});
    for name in CASES {
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
    let caption = obs_build::libobs::build_caption(&build, &root).unwrap();
    let interfaces = [
        ("OBS::libobs".into(), core.dependency),
        (
            "OBS::native-test-cmocka".into(),
            Dependency::package(&build.target, "cmocka").unwrap(),
        ),
        (
            "OBS::caption".into(),
            Dependency {
                artifacts: vec![caption],
                include_dirs: vec![root.join("deps/libcaption")],
                ..Default::default()
            },
        ),
        (
            "Threads::Threads".into(),
            Dependency {
                link_options: vec!["-pthread".into()],
                ..Default::default()
            },
        ),
        (
            "OBS::headless-characterization-headers".into(),
            obs_build::validation::headless::prepare_headers(&build, &root).unwrap(),
        ),
    ]
    .into();
    let facts = [
        ("OS_LINUX".into(), true),
        ("OS_FREEBSD".into(), false),
        ("OS_WINDOWS".into(), false),
        ("HAVE_FRONTEND_PLAIN_TEST_OWNER".into(), true),
    ]
    .into();
    // NativeProfile defaults to RelWithDebInfo, which defines NDEBUG.
    // These unchanged frontend fixtures use assert(), so compile test executables
    // in Debug while retaining the selected core's normal optimized build.
    let fixture_build = build.with_profile(obs_build::NativeProfile::Debug);
    let artifacts = graph
        .compile(&fixture_build, &root, &facts, &interfaces)
        .unwrap();
    assert_eq!(artifacts.len(), 6);
    for (name, test) in &graph.tests {
        assert_cases(
            &checked(
                &mut test
                    .command(&root, &build.output, &artifacts[&test.target])
                    .unwrap(),
            ),
            name,
        );
    }
    fs::write(directory.path().join("graph.json"), selected.to_string()).unwrap();
    write_cmake(directory.path(), &root, &core.cmake);
    replay_cmake(directory.path());
}
fn write_cmake(directory: &Path, root: &Path, core: &str) {
    let source = root.to_string_lossy().replace('\\', "/");
    fs::write(directory.join("CMakeLists.txt"), format!(r#"cmake_minimum_required(VERSION 3.28)
project(headless-characterizations-contract C CXX)
enable_testing()
option(OS_LINUX "Linux selection" ON)
option(OS_FREEBSD "FreeBSD selection" OFF)
option(OS_WINDOWS "Windows selection" OFF)
option(OS_MACOS "macOS selection" OFF)
option(ENABLE_FRONTEND "Frontend owner" ON)
option(ENABLE_UNIT_TESTS "Unit tests" ON)
{core}
find_package(Threads REQUIRED)
find_package(PkgConfig REQUIRED)
pkg_check_modules(CMOCKA REQUIRED IMPORTED_TARGET cmocka)
add_library(OBS::native-test-cmocka ALIAS PkgConfig::CMOCKA)
add_library(OBS::w32-pthreads INTERFACE IMPORTED)
add_library(OBS::caption INTERFACE IMPORTED)
set_target_properties(OBS::caption PROPERTIES INTERFACE_INCLUDE_DIRECTORIES "{source}/deps/libcaption")
foreach(name IN ITEMS test_service_state test_lua_graphics_lock test_lua_graphics_lock_bounded test_frontend_plain_helpers)
 include("{source}/test/cmocka/tests.d/${{name}}.cmake")
endforeach()
include("{source}/test/cmocka/native-test-graph.cmake")
obs_native_test_graph("${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json" "{source}")
"#)).unwrap();
}
fn replay_cmake(directory: &Path) {
    for (mode, options, count) in [
        ("enabled", vec![], 6),
        ("windows", vec!["-DOS_LINUX=OFF", "-DOS_WINDOWS=ON"], 5),
        ("macos", vec!["-DOS_LINUX=OFF", "-DOS_MACOS=ON"], 5),
        ("freebsd", vec!["-DOS_LINUX=OFF", "-DOS_FREEBSD=ON"], 6),
        ("no-frontend", vec!["-DENABLE_FRONTEND=OFF"], 4),
        ("no-tests", vec!["-DENABLE_UNIT_TESTS=OFF"], 4),
    ] {
        let output = directory.join(mode);
        // Debug keeps the original header-helper assert() checks enabled.
        checked(
            Command::new("cmake")
                .arg("-S")
                .arg(directory)
                .arg("-B")
                .arg(&output)
                .arg("-DCMAKE_BUILD_TYPE=Debug")
                .args(options),
        );
        if mode == "enabled" {
            checked(
                Command::new("cmake")
                    .arg("--build")
                    .arg(&output)
                    .args(["--parallel", "2"]),
            );
            for name in CASES {
                let result = checked(
                    Command::new("ctest")
                        .arg("--test-dir")
                        .arg(&output)
                        .args([
                            "--output-on-failure",
                            "--no-tests=error",
                            "-V",
                            "-R",
                            &format!("^{name}$"),
                        ])
                        .env("CMOCKA_MESSAGE_OUTPUT", "STDOUT"),
                );
                assert_cases(&result, name);
            }
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
        for test in tests {
            assert!(CASES.contains(&test["name"].as_str().unwrap()));
            assert!(
                !test["properties"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|p| p["name"] == "SKIP_RETURN_CODE")
            );
        }
    }
}
