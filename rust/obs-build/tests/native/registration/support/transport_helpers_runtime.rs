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
        "test_rist_open_cleanup" => 3,
        "test_media_packet_eof_leak" => 2,
        _ => 4,
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
fn canonical_transport_helpers_run_unchanged() {
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
    let mut interfaces: std::collections::BTreeMap<String, Dependency> = [
        ("OBS::libobs".into(), core.dependency.clone()),
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
        ("avdevice", "libavdevice"),
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
    for (name, package) in [
        ("CURL::libcurl", "libcurl"),
        ("jansson::jansson", "jansson"),
        ("Librist::Librist", "librist"),
        ("Libsrt::Libsrt", "srt"),
    ] {
        interfaces.insert(
            name.into(),
            Dependency::package(&build.target, package).unwrap(),
        );
    }
    let updater = file_updater(&build, &root, &core.dependency);
    interfaces.insert("OBS::file-updater".into(), updater.clone());
    interfaces.insert(
        "OBS::ffmpeg-net-host".into(),
        Dependency {
            include_dirs: vec![root.join("plugins/obs-ffmpeg/hosts/posix")],
            ..Default::default()
        },
    );
    let mut facts = std::collections::BTreeMap::from([("OS_WINDOWS".into(), false)]);
    facts.extend(FACTS.map(|name| (name.into(), true)));
    interfaces.insert(
        "m".into(),
        Dependency {
            link_options: vec!["-lm".into()],
            ..Default::default()
        },
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
    write_cmake(directory.path(), &root, &core.cmake, &updater.artifacts[0]);
    replay_cmake(directory.path());
}

fn write_cmake(directory: &Path, root: &Path, core: &str, updater: &Path) {
    let source = root.to_string_lossy().replace('\\', "/");
    let updater = updater.to_string_lossy().replace('\\', "/");
    fs::write(
        directory.join("CMakeLists.txt"),
        format!(
            r#"
cmake_minimum_required(VERSION 3.28)
project(transport-helpers-contract C)
set(CMAKE_C_VISIBILITY_PRESET hidden)
enable_testing()
option(OS_WINDOWS "Evaluate Windows headers" OFF)
option(OS_MACOS "Evaluate macOS selection" OFF)
option(OS_LINUX "Evaluate Linux selection" ON)
option(HAVE_RTMPS_TEST_SDK "Original owner and jansson targets" ON)
option(HAVE_LIBRIST_TEST_SDK "Optional librist target" ON)
option(HAVE_SRT_TEST_SDK "Linux, new MPEGTS, FFmpeg owner and SRT" ON)
option(HAVE_MEDIA_PACKET_TEST_SDK "Original core and FFmpeg targets" ON)
option(HAVE_CURL_TEST_SDK "Optional curl target" ON)
if(NOT OS_LINUX)
  set(HAVE_SRT_TEST_SDK FALSE)
endif()
{core}
find_package(PkgConfig REQUIRED)
pkg_check_modules(CMOCKA REQUIRED IMPORTED_TARGET cmocka)
add_library(OBS::native-test-cmocka ALIAS PkgConfig::CMOCKA)
add_library(OBS::w32-pthreads INTERFACE IMPORTED)
foreach(pair IN ITEMS "CURL::libcurl|libcurl" "jansson::jansson|jansson" "Librist::Librist|librist" "Libsrt::Libsrt|srt")
  string(REPLACE "|" ";" fields "${{pair}}")
  list(GET fields 0 target)
  list(GET fields 1 package)
  pkg_check_modules(${{package}} REQUIRED IMPORTED_TARGET ${{package}})
  add_library(${{target}} ALIAS PkgConfig::${{package}})
endforeach()
add_library(OBS::file-updater STATIC IMPORTED)
set_target_properties(OBS::file-updater PROPERTIES IMPORTED_LOCATION "{updater}"
  INTERFACE_INCLUDE_DIRECTORIES "{source}/shared/file-updater" INTERFACE_LINK_LIBRARIES "CURL::libcurl;OBS::libobs")
add_library(OBS::ffmpeg-net-host INTERFACE IMPORTED)
set_target_properties(OBS::ffmpeg-net-host PROPERTIES INTERFACE_INCLUDE_DIRECTORIES "{source}/plugins/obs-ffmpeg/hosts/posix")
add_library(OBS::caption INTERFACE IMPORTED)
set_target_properties(OBS::caption PROPERTIES INTERFACE_INCLUDE_DIRECTORIES "{source}/deps/libcaption")
foreach(name IN ITEMS avcodec avdevice avformat avutil swscale swresample)
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
        ("windows", vec!["-DOS_WINDOWS=ON", "-DOS_LINUX=OFF"], 5),
        ("macos", vec!["-DOS_MACOS=ON", "-DOS_LINUX=OFF"], 5),
        (
            "no-sdks",
            vec![
                "-DHAVE_RTMPS_TEST_SDK=OFF",
                "-DHAVE_LIBRIST_TEST_SDK=OFF",
                "-DHAVE_SRT_TEST_SDK=OFF",
                "-DHAVE_MEDIA_PACKET_TEST_SDK=OFF",
                "-DHAVE_CURL_TEST_SDK=OFF",
            ],
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
            assert_eq!(
                command.len(),
                if name == "test_media_packet_eof_leak" {
                    2
                } else {
                    1
                }
            );
            assert_eq!(
                Path::new(command[0].as_str().unwrap()).file_name().unwrap(),
                name
            );
        }
    }
}

fn file_updater(build: &NativeBuild, root: &Path, core: &Dependency) -> Dependency {
    let curl = Dependency::package(&build.target, "libcurl").unwrap();
    let mut library = SharedLibrary {
        name: "transport-file-updater".into(),
        sources: vec![root.join("shared/file-updater/build/file-updater.core+.h.c")],
        include_dirs: core.include_dirs.clone(),
        ..Default::default()
    };
    library
        .include_dirs
        .extend([root.into(), root.join("shared/file-updater")]);
    library.include_dirs.extend(curl.include_dirs.clone());
    let archive = build.static_library(&library).unwrap();
    Dependency {
        artifacts: vec![archive],
        include_dirs: vec![root.join("shared/file-updater")],
        link_options: curl.link_options,
        ..Default::default()
    }
}
