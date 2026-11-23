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

fn assert_cases(output: &std::process::Output, name: &str) {
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!text.to_ascii_lowercase().contains("skipped"), "{text}");
    if name == "test_windows_monitor_identity" {
        assert!(
            text.contains("Windows monitor identity: 16 checks, 0 failures"),
            "{text}"
        );
        eprintln!("{name}: {text}");
        return;
    }
    let count = match name {
        "test_qsv_update" => 3,
        "test_qsv_encode_bounds" => 8,
        "test_v4l2_device_identity" => 11,
        "test_vcam_format" | "test_v4l2_input_caps" => 4,
        _ => panic!("unknown fixture"),
    };
    assert_eq!(
        text.matches(&format!("[  PASSED  ] {count} test(s)."))
            .count(),
        1,
        "{text}"
    );
    eprintln!("{name}: {text}");
}
#[test]
fn canonical_headless_devices_run_unchanged() {
    let root = dunce::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let raw: serde_json::Value = obs_build::validation::graph::Graph::read_value(
        &root.join("test/cmocka/build/test-graph.d"),
    )
    .unwrap();
    let mut selected = serde_json::json!({"schema":1,"targets":{},"tests":{}});
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
    let mut interfaces: std::collections::BTreeMap<String, Dependency> =
        [("OBS::libobs".into(), core.dependency)].into();
    for (name, package) in [
        ("OBS::native-test-cmocka", "cmocka"),
        ("FFmpeg::avcodec", "libavcodec"),
        ("FFmpeg::avformat", "libavformat"),
        ("FFmpeg::avutil", "libavutil"),
        ("VPL::VPL", "vpl"),
        ("Libv4l2::Libv4l2", "libv4l2"),
    ] {
        interfaces.insert(
            name.into(),
            Dependency::package(&build.target, package).unwrap(),
        );
    }
    let facts = ["OS_LINUX", "HAVE_OBS_V4L2", "HAVE_QSV_TEST_SDK"]
        .into_iter()
        .map(|n| (n.into(), true))
        .chain([("OS_FREEBSD".into(), false)])
        .collect();
    let artifacts = graph.compile(&build, &root, &facts, &interfaces).unwrap();
    assert_eq!(artifacts.len(), 6);
    for name in CASES {
        assert_cases(
            &checked(
                &mut graph.tests[name]
                    .command(&root, &build.output, &artifacts[name])
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
    fs::write(directory.join("CMakeLists.txt"),format!(r#"cmake_minimum_required(VERSION 3.28)
project(headless-devices-contract C CXX)
set(CMAKE_C_VISIBILITY_PRESET hidden)
enable_testing()
option(OS_LINUX "Linux selection" ON)
option(WITH_OWNERS "Owning plugins" ON)
option(WITH_VPL "VPL SDK" ON)
{core}
find_package(PkgConfig REQUIRED)
pkg_check_modules(CMOCKA REQUIRED IMPORTED_TARGET cmocka)
add_library(OBS::native-test-cmocka ALIAS PkgConfig::CMOCKA)
list(APPEND CMAKE_MODULE_PATH "{source}/cmake/finders")
find_package(FFmpeg REQUIRED COMPONENTS avcodec avformat avutil)
if(WITH_OWNERS)
 add_library(OBS::v4l2 INTERFACE IMPORTED)
endif()
if(NOT WITH_VPL)
 set(CMAKE_DISABLE_FIND_PACKAGE_VPL TRUE)
endif()
set(HAVE_OBS_V4L2 ${{WITH_OWNERS}})
foreach(name IN ITEMS test_qsv_update test_qsv_encode_bounds test_v4l2_device_identity test_vcam_format test_v4l2_input_caps test_windows_monitor_identity)
 include("{source}/test/cmocka/tests.d/${{name}}.cmake")
endforeach()
include("{source}/test/cmocka/native-test-graph.cmake")
obs_native_test_graph("${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json" "{source}")
"#)).unwrap();
}
fn replay_cmake(directory: &Path) {
    for (mode, options, count) in [
        ("enabled", vec![], 6),
        (
            "windows",
            vec!["-DOS_LINUX=OFF", "-DOS_WINDOWS=ON", "-DWITH_OWNERS=OFF"],
            1,
        ),
        (
            "macos",
            vec!["-DOS_LINUX=OFF", "-DOS_MACOS=ON", "-DWITH_OWNERS=OFF"],
            1,
        ),
        ("no-owner", vec!["-DWITH_OWNERS=OFF"], 3),
        ("no-vpl", vec!["-DWITH_VPL=OFF"], 4),
    ] {
        let output = directory.join(mode);
        checked(
            Command::new("cmake")
                .arg("-S")
                .arg(directory)
                .arg("-B")
                .arg(&output)
                .arg("-DCMAKE_BUILD_TYPE=RelWithDebInfo")
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
        check_inventory(&output, count, mode == "enabled");
    }
}

fn check_inventory(output: &Path, count: usize, built: bool) {
    let result = checked(
        Command::new("ctest")
            .arg("--test-dir")
            .arg(output)
            .arg("--show-only=json-v1"),
    );
    let inventory: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    let tests = inventory["tests"].as_array().unwrap();
    assert_eq!(tests.len(), count, "{}", output.display());
    for test in tests {
        let name = test["name"].as_str().unwrap();
        assert!(CASES.contains(&name));
        let properties = test["properties"].as_array().unwrap();
        assert!(!properties.iter().any(|p| p["name"] == "SKIP_RETURN_CODE"));
        if built {
            let command = test["command"].as_array().unwrap();
            assert_eq!(command.len(), 1);
            assert_eq!(
                Path::new(command[0].as_str().unwrap()).file_name().unwrap(),
                name
            );
        }
    }
}
