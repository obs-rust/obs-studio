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
    // Native diagnostics are separate from cmocka case records.
    // Reject cmocka skip records; the exact passed count also forbids hidden cases.
    assert!(
        !text.lines().any(|line| line.trim_start().starts_with('[')
            && line.to_ascii_lowercase().contains("skipped")),
        "{text}"
    );
    let count = match name {
        "test_media_format_change" => 5,
        "test_media_loop_start_time" | "test_media_network_timeout" => 4,
        "test_media_ts_start_time" => 8,
        "test_media_seek_keyframe" => 7,
        "test_media_alpha_network" => 6,
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
fn canonical_media_clips_run_unchanged() {
    let root = dunce::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let raw: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap())
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
        ("FFmpeg::swscale", "libswscale"),
        ("FFmpeg::swresample", "libswresample"),
    ] {
        interfaces.insert(
            name.into(),
            Dependency::package(&build.target, package).unwrap(),
        );
    }
    interfaces.insert(
        "m".into(),
        Dependency {
            link_options: vec!["-lm".into()],
            ..Default::default()
        },
    );
    add_media(&build, &root, &mut interfaces);
    let facts = [
        ("OS_LINUX".into(), true),
        ("OS_FREEBSD".into(), false),
        ("OS_OPENBSD".into(), false),
        ("MSVC".into(), false),
        ("HAVE_MEDIA_PLAYBACK_TEST_OWNER".into(), true),
        ("OS_WINDOWS".into(), false),
    ]
    .into();
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
fn add_media(
    build: &NativeBuild,
    root: &Path,
    interfaces: &mut std::collections::BTreeMap<String, Dependency>,
) {
    let mut library = SharedLibrary {
        name: "headless-media-playback".into(),
        sources: ["cache", "core"]
            .iter()
            .map(|group| {
                root.join(format!(
                    "shared/media-playback/build/media-playback.{group}+.h.c"
                ))
            })
            .collect(),
        include_dirs: vec![
            root.into(),
            root.join("libobs"),
            root.join("shared/media-playback"),
        ],
        ..Default::default()
    };
    library
        .include_dirs
        .extend(interfaces["OBS::libobs"].include_dirs.clone());
    for (group, header) in [
        ("cache", "media-playback-cache.pch.h"),
        ("core", "media-playback.pch.h"),
    ] {
        library
            .precompiled_headers
            .push(obs_build::PrecompiledHeader {
                header: root.join("shared/media-playback/build").join(header),
                sources: vec![root.join(format!(
                    "shared/media-playback/build/media-playback.{group}+.h.c"
                ))],
            });
    }
    for package in [
        "simde",
        "libavcodec",
        "libavdevice",
        "libavutil",
        "libavformat",
    ] {
        let dependency = Dependency::package(&build.target, package).unwrap();
        library.include_dirs.extend(dependency.include_dirs);
        library.compile_options.extend(dependency.compile_options);
        library.link_options.extend(dependency.link_options);
    }
    let artifact = build.static_library(&library).unwrap();
    interfaces.insert(
        "OBS::media-playback".into(),
        Dependency {
            artifacts: std::iter::once(artifact)
                .chain(interfaces["OBS::libobs"].artifacts.clone())
                .collect(),
            include_dirs: library.include_dirs,
            compile_options: library.compile_options,
            link_options: library.link_options,
            ..Default::default()
        },
    );
}
fn write_cmake(directory: &Path, root: &Path, core: &str) {
    let source = root.to_string_lossy().replace('\\', "/");
    fs::write(directory.join("CMakeLists.txt"),format!(r#"cmake_minimum_required(VERSION 3.28)
project(media-clips-contract C)
set(CMAKE_C_VISIBILITY_PRESET hidden)
enable_testing()
option(OS_LINUX "Linux selection" ON)
option(OS_WINDOWS "Windows selection" OFF)
option(OS_MACOS "macOS selection" OFF)
option(OS_FREEBSD "FreeBSD selection" OFF)
option(OS_OPENBSD "OpenBSD selection" OFF)
option(WITH_MEDIA "Media playback owner" ON)
{core}
find_package(PkgConfig REQUIRED)
pkg_check_modules(CMOCKA REQUIRED IMPORTED_TARGET cmocka)
add_library(OBS::native-test-cmocka ALIAS PkgConfig::CMOCKA)
list(APPEND CMAKE_MODULE_PATH "{source}/cmake/finders")
if(WITH_MEDIA)
 find_package(FFmpeg 8.0 REQUIRED COMPONENTS avcodec avdevice avutil avformat swscale swresample)
 add_subdirectory("{source}/shared/media-playback" media-playback)
 target_include_directories(media-playback INTERFACE "{source}" "{source}/libobs")
endif()
foreach(name IN ITEMS test_media_format_change test_media_loop_start_time test_media_ts_start_time test_media_seek_keyframe test_media_alpha_network test_media_network_timeout)
 include("{source}/test/cmocka/tests.d/${{name}}.cmake")
endforeach()
include("{source}/test/cmocka/native-test-graph.cmake")
obs_native_test_graph("${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json" "{source}")
"#)).unwrap();
}
fn replay_cmake(directory: &Path) {
    for (mode, options, count) in [
        ("enabled", vec![], 6),
        ("windows", vec!["-DOS_LINUX=OFF", "-DOS_WINDOWS=ON"], 6),
        ("macos", vec!["-DOS_LINUX=OFF", "-DOS_MACOS=ON"], 6),
        ("no-media", vec!["-DWITH_MEDIA=OFF"], 0),
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
