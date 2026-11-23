use super::*;
use obs_build::validation::graph::{Dependency, Graph};
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
fn canonical_display_outputs_runs_unchanged() {
    assert!(std::env::var_os("DISPLAY").is_some_and(|value| !value.is_empty()));
    let root = dunce::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let raw: serde_json::Value = obs_build::validation::graph::Graph::read_value(
        &root.join("test/cmocka/build/test-graph.d"),
    )
    .unwrap();
    let mut selected = expected();
    for section in ["targets", "tests"] {
        for (name, record) in selected[section].as_object_mut().unwrap() {
            *record = raw[section][name].clone();
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
    let core_path = core.dependency.artifacts[0].clone();
    let renderer =
        obs_build::graphics::build_opengl(&build, &root, &core_path, &Default::default()).unwrap();
    assert_core_binding(&renderer, &core_path);
    let interfaces = interfaces(&build, core.dependency, renderer.clone());
    let facts = [
        ("OS_LINUX".into(), true),
        ("OS_WINDOWS".into(), false),
        ("OS_MACOS".into(), false),
        ("ENABLE_PULSEAUDIO".into(), true),
        ("HAVE_SLIDESHOW_TEST_OWNER".into(), true),
        ("HAVE_OBS_OPENGL".into(), true),
        ("HAVE_X11_DISPLAY".into(), true),
    ]
    .into();
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
    let artifacts = graph.compile(&build, &root, &facts, &interfaces).unwrap();
    assert_eq!(artifacts.len(), CASES.len());
    for name in CASES {
        assert_core_binding(&artifacts[name], &core_path);
    }
    for (name, spec) in &graph.tests {
        let result = checked(
            spec.command_in(&root, &build.output, &artifacts[&spec.target], &runtime)
                .unwrap()
                .env("CMOCKA_MESSAGE_OUTPUT", "STDOUT"),
        );
        assert_cases(&result, &spec.target);
        eprintln!(
            "Cargo {name}: {}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
    fs::write(directory.path().join("graph.json"), selected.to_string()).unwrap();
    write_cmake(directory.path(), &root, &runtime, &core.cmake, &renderer);
    replay_cmake(directory.path(), &core_path);
}

fn interfaces(
    build: &NativeBuild,
    core: Dependency,
    renderer: std::path::PathBuf,
) -> std::collections::BTreeMap<String, Dependency> {
    let mut interfaces = std::collections::BTreeMap::from([
        ("OBS::libobs".into(), core),
        (
            "OBS::libobs-opengl".into(),
            Dependency {
                artifacts: vec![renderer],
                ..Default::default()
            },
        ),
        ("OBS::caption".into(), Dependency::default()),
        ("Uthash::Uthash".into(), Dependency::default()),
    ]);
    for (name, package) in [
        ("OBS::native-test-cmocka", "cmocka"),
        ("FFmpeg::avcodec", "libavcodec"),
        ("FFmpeg::avformat", "libavformat"),
        ("FFmpeg::avutil", "libavutil"),
        ("FFmpeg::swscale", "libswscale"),
        ("FFmpeg::swresample", "libswresample"),
        ("PulseAudio::PulseAudio", "libpulse"),
    ] {
        interfaces.insert(
            name.into(),
            Dependency::package(&build.target, package).unwrap(),
        );
    }
    interfaces
}

fn write_cmake(directory: &Path, root: &Path, runtime: &Path, core: &str, renderer: &Path) {
    let source = root.to_string_lossy().replace('\\', "/");
    fs::write(directory.join("CMakeLists.txt"), format!(r#"cmake_minimum_required(VERSION 3.28)
project(display-outputs-contract C CXX)
set(CMAKE_C_VISIBILITY_PRESET hidden)
enable_testing()
option(OS_LINUX "Original Linux guard" ON)
option(OS_WINDOWS "Windows dependency guard" OFF)
option(OS_MACOS "macOS source guard" OFF)
option(HAVE_OBS_OPENGL "Original renderer guard" ON)
option(ENABLE_PULSEAUDIO "Original PulseAudio guard" ON)
option(DECLARE_SLIDESHOW "Original slideshow owner guard" ON)
option(DECLARE_XVFB_RUNNER "Root CMake declares its Xvfb runner" ON)
if(DECLARE_XVFB_RUNNER)
  find_program(OBS_XVFB_RUN_REAL xvfb-run REQUIRED)
  configure_file("{source}/test/cmocka/xvfb-run-or-display.sh.in" "${{CMAKE_CURRENT_BINARY_DIR}}/xvfb-run-or-display.sh" @ONLY)
  set(XVFB_RUN "${{CMAKE_CURRENT_BINARY_DIR}}/xvfb-run-or-display.sh")
  file(CHMOD "${{XVFB_RUN}}" PERMISSIONS OWNER_READ OWNER_WRITE OWNER_EXECUTE GROUP_READ GROUP_EXECUTE WORLD_READ WORLD_EXECUTE)
endif()
set(OBS_OUTPUT_DIR "{runtime}")
find_package(PkgConfig REQUIRED)
pkg_check_modules(CMOCKA REQUIRED IMPORTED_TARGET cmocka)
{core}
# built_core deliberately hides consumer symbols for ordinary probes. The real
# core target has no such interface policy; these interposition fixtures need
# their original default visibility, selected by the graph's compiler option.
set_property(TARGET OBS::libobs PROPERTY INTERFACE_COMPILE_OPTIONS "")
add_library(OBS::native-test-cmocka ALIAS PkgConfig::CMOCKA)
foreach(component avcodec avformat avutil swscale swresample)
  pkg_check_modules(${{component}} REQUIRED IMPORTED_TARGET lib${{component}})
  add_library(FFmpeg::${{component}} ALIAS PkgConfig::${{component}})
endforeach()
pkg_check_modules(PULSE REQUIRED IMPORTED_TARGET libpulse)
add_library(PulseAudio::PulseAudio ALIAS PkgConfig::PULSE)
add_library(OBS::caption INTERFACE IMPORTED)
add_library(Uthash::Uthash INTERFACE IMPORTED)
if(OS_WINDOWS)
  add_library(OBS::w32-pthreads INTERFACE IMPORTED)
endif()
if(DECLARE_SLIDESHOW)
  add_library(image-source INTERFACE IMPORTED)
  add_library(obs-transitions INTERFACE IMPORTED)
endif()
include("{source}/test/cmocka/tests.d/test_slideshow_empty.cmake")
if(HAVE_OBS_OPENGL)
  add_library(OBS::libobs-opengl MODULE IMPORTED)
  set_property(TARGET OBS::libobs-opengl PROPERTY IMPORTED_LOCATION "{renderer}")
endif()
include("{source}/test/cmocka/native-test-graph.cmake")
obs_native_test_graph("${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json" "{source}")
"#, runtime=runtime.display(), renderer=renderer.display())).unwrap();
}

fn replay_cmake(directory: &Path, core: &Path) {
    let all: Vec<String> = expected()["tests"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    for (mode, options, expected_names) in [
        ("enabled", vec![], all.clone()),
        (
            "windows",
            vec!["-DOS_LINUX=OFF", "-DOS_WINDOWS=ON"],
            vec!["test_slideshow_empty".into()],
        ),
        (
            "macos",
            vec!["-DOS_LINUX=OFF", "-DOS_MACOS=ON"],
            vec!["test_slideshow_empty".into()],
        ),
        ("no-display", vec!["-DDECLARE_XVFB_RUNNER=OFF"], vec![]),
        ("declared-runner", vec![], all.clone()),
        (
            "no-renderer",
            vec!["-DHAVE_OBS_OPENGL=OFF"],
            all.iter()
                .filter(|name| !name.starts_with("test_ffmpeg_"))
                .cloned()
                .collect(),
        ),
        (
            "no-pulse",
            vec!["-DENABLE_PULSEAUDIO=OFF"],
            all.iter()
                .filter(|name| !name.starts_with("test_pulse_"))
                .cloned()
                .collect(),
        ),
        (
            "no-owners",
            vec!["-DDECLARE_SLIDESHOW=OFF"],
            all.iter()
                .filter(|name| name.as_str() != "test_slideshow_empty")
                .cloned()
                .collect(),
        ),
    ] {
        let output = directory.join(mode);
        let mut command = Command::new("cmake");
        command
            .arg("-S")
            .arg(directory)
            .arg("-B")
            .arg(&output)
            .arg("-DCMAKE_BUILD_TYPE=RelWithDebInfo")
            .args(options);
        if mode == "no-display" || mode == "declared-runner" {
            command.env_remove("DISPLAY");
        }
        checked(&mut command);
        if mode == "enabled" {
            checked(
                Command::new("cmake")
                    .arg("--build")
                    .arg(&output)
                    .args(["--parallel", "2"]),
            );
            for name in CASES {
                assert_core_binding(&output.join(name), core);
            }
            for name in &all {
                let result = checked(
                    Command::new("ctest")
                        .arg("--test-dir")
                        .arg(&output)
                        .args(["--output-on-failure", "--no-tests=error", "-V", "-R"])
                        .arg(format!("^{name}$"))
                        .env("CMOCKA_MESSAGE_OUTPUT", "STDOUT"),
                );
                let fixture = expected()["tests"][name]["target"]
                    .as_str()
                    .unwrap()
                    .to_owned();
                assert_cases(&result, &fixture);
                assert!(
                    String::from_utf8_lossy(&result.stdout)
                        .contains("100% tests passed, 0 tests failed out of 1")
                );
                eprintln!("CMake {name}: {}", String::from_utf8_lossy(&result.stdout));
            }
        }
        check_inventory(&output, &expected_names, mode == "enabled");
    }
}

fn check_inventory(output: &Path, names: &[String], built: bool) {
    let result = checked(
        Command::new("ctest")
            .arg("--test-dir")
            .arg(output)
            .arg("--show-only=json-v1"),
    );
    let inventory: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    let tests = inventory["tests"].as_array().unwrap();
    assert_eq!(tests.len(), names.len(), "{}", output.display());
    let expected = expected();
    for test in tests {
        let name = test["name"].as_str().unwrap();
        assert!(names.iter().any(|expected| expected == name));
        let record = &expected["tests"][name];
        let properties = test["properties"].as_array().unwrap();
        let property = |name: &str| {
            properties
                .iter()
                .find(|p| p["name"] == name)
                .map(|p| &p["value"])
        };
        assert_eq!(
            property("TIMEOUT").unwrap().as_f64(),
            record["timeout_seconds"].as_f64()
        );
        assert_eq!(
            property("SKIP_RETURN_CODE").and_then(serde_json::Value::as_f64),
            record["skip_return_code"].as_f64()
        );
        if built {
            let command = test["command"].as_array().unwrap();
            let target = record["target"].as_str().unwrap();
            let executable = output.join(target).to_string_lossy().into_owned();
            let index = command
                .iter()
                .position(|value| value.as_str() == Some(&executable))
                .unwrap();
            if target == "test_pulse_monitor_reset" {
                assert_eq!(command[index + 1], record["command"][1]);
            }
            if record.get("optional_prefixes").is_some() {
                assert!(index > 0);
            } else {
                assert_eq!(index, 0);
            }
            let cwd = property("WORKING_DIRECTORY").unwrap().as_str().unwrap();
            let wanted = if record.get("working_directory").is_some() {
                output.parent().unwrap().join("runtime/RelWithDebInfo")
            } else {
                output.to_path_buf()
            };
            assert_eq!(
                dunce::canonicalize(cwd).unwrap(),
                dunce::canonicalize(wanted).unwrap()
            );
        }
    }
}

fn assert_cases(output: &std::process::Output, name: &str) {
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !super::super::canonical_display_startup_is_registered::has_skipped_cases(&text),
        "{text}"
    );
    let expected = match name {
        "test_ffmpeg_mux_unwritable_path" | "test_ffmpeg_output_stop" => vec![2],
        "test_ffmpeg_output_alpha" => vec![3],
        "test_ffmpeg_output_drain" => vec![4],
        "test_pulse_monitor_reset" => vec![1],
        "test_slideshow_empty" => vec![1; 12],
        _ => panic!("unexpected fixture {name}"),
    };
    let actual: Vec<usize> = text
        .lines()
        .filter_map(|line| {
            line.split_once("[  PASSED  ] ")
                .map(|(_, summary)| summary.split_whitespace().next().unwrap().parse().unwrap())
        })
        .collect();
    assert_eq!(actual, expected, "{text}");
}

fn assert_core_binding(executable: &Path, core: &Path) {
    let output = checked(Command::new("ldd").arg(executable));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(core.to_str().unwrap()),
        "{} must load {}: {stdout}",
        executable.display(),
        core.display()
    );
    eprintln!("Loader {}: {}", executable.display(), core.display());
}
