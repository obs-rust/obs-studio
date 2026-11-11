use super::*;
use obs_build::validation::graph::Dependency;
use std::process::{Command, Output};

fn checked(command: &mut Command) -> Output {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{command:?}\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn assert_cases(output: &Output, name: &str) {
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
    .replace("\r\n", "\n");
    let counts: &[usize] = match name {
        "test_crash_log_upload" => &[1],
        "test_rtmps_fallback_warning" => &[8],
        "test_preview_audio_sources" => &[5],
        "test_source_select_list" => &[3],
        "test_output_filename_reserve" => &[12, 1],
        _ => panic!("unknown fixture {name}"),
    };
    for count in counts {
        assert!(
            text.contains(&format!("[  PASSED  ] {count} test(s).")),
            "{name}: {text}"
        );
    }
    assert!(!text.contains("[  SKIPPED ]"), "{name}: {text}");
    assert!(!text.contains("[  FAILED  ]"), "{name}: {text}");
}

fn binding(binary: &Path, core: &Path) {
    let output = checked(Command::new("ldd").arg(binary));
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(!text.contains("not found"), "{text}");
    let line = text
        .lines()
        .find(|line| line.trim_start().starts_with("libobs.so"));
    let Some(line) = line else {
        assert_eq!(binary.file_name().unwrap(), "test_rtmps_fallback_warning");
        eprintln!(
            "Stand-in fixture {}: no libobs DSO retained",
            binary.display()
        );
        return;
    };
    let loaded = line
        .split_once("=>")
        .unwrap()
        .1
        .split_whitespace()
        .next()
        .unwrap();
    assert_eq!(
        dunce::canonicalize(loaded).unwrap(),
        dunce::canonicalize(core).unwrap(),
        "{text}"
    );
    eprintln!("Selected core {}: {}", binary.display(), core.display());
}

#[test]
fn canonical_qt_core_filename_run_unchanged() {
    let root = dunce::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let build = NativeBuild {
        host: obs_build::BUILD_HOST.into(),
        target: obs_build::BUILD_TARGET.into(),
        output: directory.path().join("cargo-output"),
        profile: Default::default(),
    };
    let core = built_core(&build, &root);
    let core_path = core.dependency.artifacts[0].clone();
    let renderer =
        obs_build::graphics::build_opengl(&build, &root, &core_path, &Default::default()).unwrap();
    let runtime = directory.path().join("runtime");
    for data in ["share/obs", "RelWithDebInfo/share/obs"] {
        fs::create_dir_all(runtime.join(data)).unwrap();
        checked(
            Command::new("cp")
                .arg("-R")
                .arg(root.join("libobs/data"))
                .arg(runtime.join(data).join("libobs")),
        );
    }
    let interfaces = interfaces(&build, core.dependency, &renderer);
    let facts = std::collections::BTreeMap::from([
        ("UNIX".into(), true),
        ("OS_LINUX".into(), true),
        ("OS_FREEBSD".into(), false),
        ("OS_WINDOWS".into(), false),
        ("OS_MACOS".into(), false),
        ("HAVE_QT_CORE_TEST_OWNER".into(), true),
        ("HAVE_CRASH_LOG_TEST_OWNER".into(), true),
        ("HAVE_OBS_OPENGL".into(), true),
    ]);
    let mut graph = expected();
    for target in graph.targets.values_mut() {
        target.link_options.extend([
            "-Wl,--as-needed".into(),
            "-Wl,--no-copy-dt-needed-entries".into(),
        ]);
    }
    let artifacts = graph.compile(&build, &root, &facts, &interfaces).unwrap();
    assert_eq!(artifacts.len(), 5);
    let raw: serde_json::Value =
        serde_json::from_str(include_str!("../qt_core_filename_expected.json")).unwrap();
    fs::write(directory.path().join("graph.json"), raw.to_string()).unwrap();
    write_cmake(directory.path(), &root, &core.cmake, &renderer, &runtime);
    let output = directory.path().join("cmake-output");
    checked(
        Command::new("cmake")
            .arg("-S")
            .arg(directory.path())
            .arg("-B")
            .arg(&output)
            .args(["-G", "Ninja", "-DCMAKE_BUILD_TYPE=RelWithDebInfo"]),
    );
    checked(
        Command::new("cmake")
            .arg("--build")
            .arg(&output)
            .args(["--parallel", "4"]),
    );
    for name in graph.tests.keys() {
        binding(&output.join(name), &core_path);
        let result = checked(
            Command::new("ctest")
                .arg("--test-dir")
                .arg(&output)
                .args(["-V", "--no-tests=error", "--repeat", "until-fail:5", "-R"])
                .arg(format!("^{name}$"))
                .env("CMOCKA_MESSAGE_OUTPUT", "STDOUT"),
        );
        assert_cases(&result, name);
        eprintln!("CMake {name}: {}", String::from_utf8_lossy(&result.stdout));
    }
    for (name, artifact) in artifacts {
        binding(&artifact, &core_path);
        for iteration in 1..=5 {
            let output = checked(
                graph.tests[&name]
                    .resolve_runtime(&interfaces)
                    .unwrap()
                    .command_in(&root, &build.output, &artifact, &runtime)
                    .unwrap()
                    .env("CMOCKA_MESSAGE_OUTPUT", "STDOUT"),
            );
            assert_cases(&output, &name);
            eprintln!(
                "Cargo {name} iteration {iteration}: {}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}

fn interfaces(
    build: &NativeBuild,
    core: Dependency,
    renderer: &Path,
) -> std::collections::BTreeMap<String, Dependency> {
    let mut interfaces = std::collections::BTreeMap::from([
        ("OBS::libobs".into(), core),
        ("nlohmann_json::nlohmann_json".into(), Dependency::default()),
        (
            "OBS::output-filename-mux-test".into(),
            Dependency {
                definitions: vec![
                    ("TEST_FFMPEG_MUX".into(), None),
                    (
                        "GRAPHICS_MODULE_PATH".into(),
                        Some(serde_json::to_string(&renderer.to_str().unwrap()).unwrap()),
                    ),
                ],
                ..Default::default()
            },
        ),
    ]);
    for (name, package) in [
        ("OBS::native-test-cmocka", "cmocka"),
        ("Qt6::Core", "Qt6Core"),
        ("FFmpeg::avformat", "libavformat"),
        ("FFmpeg::avutil", "libavutil"),
    ] {
        interfaces.insert(
            name.into(),
            Dependency::package(&build.target, package).unwrap(),
        );
    }
    interfaces
}

fn write_cmake(directory: &Path, root: &Path, core: &str, renderer: &Path, runtime: &Path) {
    let cmake = format!(
        r#"cmake_minimum_required(VERSION 3.28)
project(qt-core-filename-contract C CXX)
set(CMAKE_CXX_STANDARD 17)
enable_testing()
set(OS_LINUX ON)
set(OS_FREEBSD OFF)
set(OS_WINDOWS OFF)
set(OS_MACOS OFF)
set(OBS_OUTPUT_DIR "{runtime}")
set(CMAKE_MODULE_PATH "{root}/cmake/finders")
{core}
find_package(PkgConfig REQUIRED)
pkg_check_modules(CMOCKA REQUIRED IMPORTED_TARGET cmocka)
add_library(OBS::native-test-cmocka ALIAS PkgConfig::CMOCKA)
find_package(Qt6 REQUIRED COMPONENTS Core)
find_package(nlohmann_json 3.11 REQUIRED)
find_package(FFmpeg REQUIRED COMPONENTS avformat avutil)
add_library(OBS::libobs-opengl MODULE IMPORTED)
set_target_properties(OBS::libobs-opengl PROPERTIES IMPORTED_LOCATION "{renderer}")
set(HAVE_OBS_OPENGL ON)
include("{root}/test/cmocka/native-test-graph.cmake")
add_link_options("LINKER:--as-needed" "LINKER:--no-copy-dt-needed-entries")
obs_native_test_graph("${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json" "{root}")
"#,
        root = root.to_string_lossy().replace('\\', "/"),
        renderer = renderer.to_string_lossy().replace('\\', "/"),
        runtime = runtime.to_string_lossy().replace('\\', "/")
    );
    fs::write(directory.join("CMakeLists.txt"), cmake).unwrap();
}
