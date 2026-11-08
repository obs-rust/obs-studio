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
    let count = match name {
        "test_alsa_device_lost" => 5,
        "test_alsa_reopen_destroy" => 2,
        "test_jack_input" => 3,
        "test_ffmpeg_mux_helper" => 6,
        _ => 1,
    };
    assert_eq!(
        text.matches(&format!("[  PASSED  ] {count} test(s)."))
            .count(),
        1,
        "{text}"
    );
    eprintln!("{text}");
}

/// Refs #1390: both consumers execute every headless ABI case headlessly.
#[test]
fn canonical_headless_audio_run_unchanged() {
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
            "OBS::caption".into(),
            Dependency {
                include_dirs: vec![root.join("deps/libcaption")],
                ..Default::default()
            },
        ),
        (
            "OBS::native-test-cmocka".into(),
            Dependency::package(&build.target, "cmocka").unwrap(),
        ),
    ]
    .into();
    for (name, package) in [
        ("avcodec", "libavcodec"),
        ("avformat", "libavformat"),
        ("avutil", "libavutil"),
        ("swscale", "libswscale"),
        ("swresample", "libswresample"),
    ] {
        interfaces.insert(
            format!("FFmpeg::{name}"),
            Dependency::package(&build.target, package).unwrap(),
        );
    }
    let facts: std::collections::BTreeMap<String, bool> = [
        ("OS_WINDOWS".into(), false),
        ("OS_LINUX".into(), true),
        ("HAVE_OBS_ALSA".into(), true),
    ]
    .into();
    interfaces.insert(
        "ALSA::ALSA".into(),
        Dependency::package(&build.target, "alsa").unwrap(),
    );
    let mut facts = facts;
    facts.insert(
        "C_COMPILER_CLANG".into(),
        std::env::var("CC").unwrap_or_default().contains("clang"),
    );
    let artifacts = graph.compile(&build, &root, &facts, &interfaces).unwrap();
    assert_eq!(artifacts.len(), CASES.len());
    // Require every unchanged cmocka case without skips.
    for name in CASES {
        eprintln!("Cargo {name}");
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
    fs::write(
        directory.join("CMakeLists.txt"),
        format!(
            r#"
cmake_minimum_required(VERSION 3.28)
project(headless-audio-contract C)
set(CMAKE_C_VISIBILITY_PRESET hidden)
enable_testing()
option(OS_LINUX "Evaluate Linux selection" ON)
option(HAVE_OBS_ALSA "Evaluate owner selection" ON)
option(OS_WINDOWS "Evaluate original Windows exclusion" OFF)
option(OS_MACOS "Evaluate original macOS selection" OFF)
{core}
find_package(PkgConfig REQUIRED)
pkg_check_modules(CMOCKA REQUIRED IMPORTED_TARGET cmocka)
add_library(OBS::native-test-cmocka ALIAS PkgConfig::CMOCKA)
add_library(OBS::w32-pthreads INTERFACE IMPORTED)
find_package(ALSA REQUIRED)
set(C_COMPILER_CLANG FALSE)
if(CMAKE_C_COMPILER_ID MATCHES "Clang")
  set(C_COMPILER_CLANG TRUE)
endif()
add_library(OBS::caption INTERFACE IMPORTED)
set_target_properties(OBS::caption PROPERTIES INTERFACE_INCLUDE_DIRECTORIES "{source}/deps/libcaption")
foreach(name IN ITEMS avcodec avformat avutil swscale swresample)
  pkg_check_modules(${{name}} REQUIRED IMPORTED_TARGET lib${{name}})
  add_library(FFmpeg::${{name}} ALIAS PkgConfig::${{name}})
endforeach()
include("{source}/test/cmocka/native-test-graph.cmake")
obs_native_test_graph("${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json" "{source}")
"#
        ),
    )
    .unwrap();
}

fn replay_cmake(directory: &Path) {
    for (mode, options, count) in [
        ("enabled", vec![], CASES.len()),
        (
            "windows",
            vec!["-DOS_WINDOWS=ON", "-DOS_LINUX=OFF", "-DHAVE_OBS_ALSA=OFF"],
            0,
        ),
        (
            "macos",
            vec!["-DOS_MACOS=ON", "-DOS_LINUX=OFF", "-DHAVE_OBS_ALSA=OFF"],
            0,
        ),
        ("no-owner", vec!["-DHAVE_OBS_ALSA=OFF"], 2),
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
