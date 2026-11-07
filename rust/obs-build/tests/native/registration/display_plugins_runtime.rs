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

fn assert_no_skips(output: &std::process::Output) {
    assert!(
        ![output.stdout.as_slice(), output.stderr.as_slice()]
            .iter()
            .any(
                |stream| super::super::canonical_display_startup_is_registered::has_skipped_cases(
                    &String::from_utf8_lossy(stream)
                )
            ),
        "skipped native cases: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn canonical_display_plugins_runs_unchanged() {
    assert!(
        std::env::var_os("DISPLAY").is_some_and(|value| !value.is_empty()),
        "run this display contract under Xvfb or declare an X server"
    );
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
    let core_path = core.dependency.artifacts[0].clone();
    let renderer = obs_build::graphics::build_opengl(
        &build,
        &root,
        &core.dependency.artifacts[0],
        &Default::default(),
    )
    .unwrap();
    assert_core_binding(&renderer, &core_path);
    let (filters, services) = modules(&build, &root, &core_path);
    let interfaces = interfaces(
        &build,
        core.dependency,
        renderer.clone(),
        filters.clone(),
        services.clone(),
    );
    let facts = [
        ("OS_LINUX".into(), true),
        ("OS_WINDOWS".into(), false),
        ("MSVC".into(), false),
        ("HAVE_STARTUP_X11".into(), true),
        ("HAVE_OBS_FILTERS".into(), true),
        ("HAVE_RTMP_SERVICES_MODULE".into(), true),
        ("HAVE_QT6_WIDGETS".into(), true),
        ("HAVE_OBS_OPENGL".into(), true),
        ("HAVE_X11_DISPLAY".into(), true),
    ]
    .into();
    let runtime = directory.path().join("runtime");
    for data in ["share/obs", "RelWithDebInfo/share/obs"] {
        fs::create_dir_all(runtime.join(data)).unwrap();
        checked(
            Command::new("cp")
                .args(["-R"])
                .arg(root.join("libobs/data"))
                .arg(runtime.join(data).join("libobs")),
        );
    }
    let artifacts = graph.compile(&build, &root, &facts, &interfaces).unwrap();
    assert_eq!(artifacts.len(), CASES.len());
    for name in CASES {
        assert_core_binding(&artifacts[name], &core_path);
        let result = checked(
            graph.tests[name]
                .command_in(&root, &build.output, &artifacts[name], &runtime)
                .unwrap()
                .env("CMOCKA_MESSAGE_OUTPUT", "STDOUT"),
        );
        assert_cases(&result, expected_cases(name));
        eprintln!("Cargo {name}: {}", String::from_utf8_lossy(&result.stdout));
    }
    fs::write(directory.path().join("graph.json"), selected.to_string()).unwrap();
    write_cmake(
        directory.path(),
        &root,
        &runtime,
        &core.cmake,
        &renderer,
        &filters,
        &services,
    );
    replay_cmake(directory.path(), &core_path);
}

fn modules(
    build: &NativeBuild,
    root: &Path,
    core: &Path,
) -> (std::path::PathBuf, std::path::PathBuf) {
    let filters = obs_build::plugins::build_filters(
        build,
        root,
        core,
        &obs_build::plugins::FiltersOptions {
            speexdsp: false,
            rnnoise: false,
        },
    )
    .unwrap();
    assert_core_binding(&filters.library, core);
    let services = obs_build::plugins::build_source(
        build,
        root,
        core,
        obs_build::plugins::SourceModule::RtmpServices { updates: false },
    )
    .unwrap();
    assert_core_binding(&services.library, core);
    (filters.library, services.library)
}

fn interfaces(
    build: &NativeBuild,
    core: Dependency,
    renderer: std::path::PathBuf,
    filters: std::path::PathBuf,
    services: std::path::PathBuf,
) -> std::collections::BTreeMap<String, Dependency> {
    let mut interfaces: std::collections::BTreeMap<String, Dependency> = [
        ("OBS::libobs".into(), core),
        (
            "OBS::native-test-cmocka".into(),
            Dependency::package(&build.target, "cmocka").unwrap(),
        ),
    ]
    .into();
    interfaces.insert(
        "OBS::libobs-opengl".into(),
        Dependency {
            artifacts: vec![renderer],
            ..Default::default()
        },
    );
    interfaces.insert(
        "m".into(),
        Dependency {
            link_options: vec!["-lm".into()],
            ..Default::default()
        },
    );
    interfaces.insert(
        "ZLIB::ZLIB".into(),
        Dependency::package(&build.target, "zlib").unwrap(),
    );

    for (name, package) in [("Qt6::Widgets", "Qt6Widgets"), ("X11::X11", "x11")] {
        interfaces.insert(
            name.into(),
            Dependency::package(&build.target, package).unwrap(),
        );
    }
    interfaces.insert(
        "obs-filters".into(),
        Dependency {
            artifacts: vec![filters],
            ..Default::default()
        },
    );
    interfaces.insert(
        "rtmp-services".into(),
        Dependency {
            artifacts: vec![services],
            ..Default::default()
        },
    );
    for name in ["OBS::caption", "Uthash::Uthash"] {
        interfaces.insert(name.into(), Dependency::default());
    }
    interfaces.insert(
        "dl".into(),
        Dependency {
            link_options: vec!["-ldl".into()],
            ..Default::default()
        },
    );
    interfaces
}

fn write_cmake(
    directory: &Path,
    root: &Path,
    runtime: &Path,
    core: &str,
    renderer: &Path,
    filters: &Path,
    services: &Path,
) {
    let source = root.to_string_lossy().replace('\\', "/");
    fs::write(
        directory.join("CMakeLists.txt"),
        format!(
            r#"cmake_minimum_required(VERSION 3.28)
project(display-plugins-contract C CXX)
set(CMAKE_C_VISIBILITY_PRESET hidden)
enable_testing()
option(OS_LINUX "Original Linux guard" ON)
option(OS_WINDOWS "Windows dependency guard" OFF)
option(OS_MACOS "macOS fixture guard" OFF)
option(HAVE_OBS_OPENGL "Original renderer guard" ON)
option(DECLARE_XVFB_RUNNER "Root CMake declares its runtime Xvfb runner" ON)
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
add_library(OBS::native-test-cmocka ALIAS PkgConfig::CMOCKA)
option(DECLARE_FILTERS "Original built filter module" ON)
set(HAVE_OBS_FILTERS FALSE)
if(DECLARE_FILTERS)
  add_library(obs-filters MODULE IMPORTED)
  set_property(TARGET obs-filters PROPERTY IMPORTED_LOCATION "{filters}")
  set(HAVE_OBS_FILTERS TRUE)
endif()
option(DECLARE_SERVICES "Original built RTMP services module" ON)
set(HAVE_RTMP_SERVICES_MODULE FALSE)
if(DECLARE_SERVICES)
  add_library(rtmp-services MODULE IMPORTED)
  set_property(TARGET rtmp-services PROPERTY IMPORTED_LOCATION "{services}")
  set(HAVE_RTMP_SERVICES_MODULE TRUE)
endif()
option(DECLARE_QT "Original Qt Widgets headers" ON)
set(HAVE_QT6_WIDGETS FALSE)
if(DECLARE_QT)
  find_package(Qt6 REQUIRED COMPONENTS Widgets)
  set(HAVE_QT6_WIDGETS TRUE)
endif()
option(DECLARE_X11 "Declare the original X11 dependency" ON)
if(DECLARE_X11)
  find_package(X11 REQUIRED)
endif()
add_library(OBS::caption INTERFACE IMPORTED)
add_library(Uthash::Uthash INTERFACE IMPORTED)
if(OS_WINDOWS)
  add_library(OBS::w32-pthreads INTERFACE IMPORTED)
endif()
if(HAVE_OBS_OPENGL)
  add_library(OBS::libobs-opengl MODULE IMPORTED)
  set_property(TARGET OBS::libobs-opengl PROPERTY IMPORTED_LOCATION "{renderer}")
endif()
include("{source}/test/cmocka/native-test-graph.cmake")
obs_native_test_graph("${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json" "{source}")
"#,
            runtime = runtime.display(),
            renderer = renderer.display(),
            filters = filters.display(),
            services = services.display()
        ),
    )
    .unwrap();
}

fn replay_cmake(directory: &Path, core: &Path) {
    for (mode, options, expected) in [
        ("enabled", vec![], CASES.to_vec()),
        ("windows", vec!["-DOS_LINUX=OFF", "-DOS_WINDOWS=ON"], vec![]),
        ("macos", vec!["-DOS_LINUX=OFF", "-DOS_MACOS=ON"], vec![]),
        ("no-display", vec!["-DDECLARE_XVFB_RUNNER=OFF"], vec![]),
        ("declared-runner", vec![], CASES.to_vec()),
        (
            "no-renderer",
            vec!["-DHAVE_OBS_OPENGL=OFF"],
            vec!["test_rtmps_upgrade", "test_x11_volume_keys"],
        ),
        (
            "no-x11",
            vec!["-DDECLARE_X11=OFF"],
            CASES
                .into_iter()
                .filter(|name| *name != "test_x11_volume_keys")
                .collect(),
        ),
        (
            "no-filters",
            vec!["-DDECLARE_FILTERS=OFF"],
            CASES
                .into_iter()
                .filter(|name| {
                    !matches!(
                        *name,
                        "test_color_correction_16f" | "test_scroll_filter_wide_source"
                    )
                })
                .collect(),
        ),
        (
            "no-services",
            vec!["-DDECLARE_SERVICES=OFF"],
            CASES
                .into_iter()
                .filter(|name| *name != "test_rtmps_upgrade")
                .collect(),
        ),
        (
            "no-qt",
            vec!["-DDECLARE_QT=OFF"],
            CASES
                .into_iter()
                .filter(|name| *name != "test_frontend_screenshot_srgb")
                .collect(),
        ),
    ] {
        let output = directory.join(mode);
        let mut configure = Command::new("cmake");
        configure
            .arg("-S")
            .arg(directory)
            .arg("-B")
            .arg(&output)
            .arg("-DCMAKE_BUILD_TYPE=RelWithDebInfo")
            .args(options);
        if mode == "no-display" || mode == "declared-runner" {
            configure.env_remove("DISPLAY");
        }
        checked(&mut configure);
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
            for name in CASES {
                let result = checked(
                    Command::new("ctest")
                        .arg("--test-dir")
                        .arg(&output)
                        .args(["--output-on-failure", "--no-tests=error", "-V", "-R"])
                        .arg(format!("^{name}$"))
                        .env("CMOCKA_MESSAGE_OUTPUT", "STDOUT"),
                );
                assert_cases(&result, expected_cases(name));
                assert!(
                    String::from_utf8_lossy(&result.stdout)
                        .contains("100% tests passed, 0 tests failed out of 1")
                );
                eprintln!("CMake {name}: {}", String::from_utf8_lossy(&result.stdout));
            }
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
            let command = test["command"].as_array().unwrap();
            assert_eq!(command.len(), 1);
            assert_eq!(
                Path::new(
                    test["command"]
                        .as_array()
                        .unwrap()
                        .last()
                        .unwrap()
                        .as_str()
                        .unwrap()
                )
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
        let expected = super::expected();
        let record = &expected["tests"][name];
        assert_eq!(
            timeout.map(|p| p["value"].as_f64().unwrap()),
            record["timeout_seconds"].as_f64()
        );
        let locks = test["properties"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["name"] == "RESOURCE_LOCK");
        let expected_locks = record
            .get("resource_locks")
            .and_then(|value| value.as_array());
        assert_eq!(
            locks.map(|property| property["value"].as_array().unwrap()),
            expected_locks
        );
        let cwd = test["properties"]
            .as_array()
            .unwrap()
            .iter()
            .find(|property| property["name"] == "WORKING_DIRECTORY")
            .unwrap();
        let expected_cwd = if record.get("working_directory").is_some() {
            output.parent().unwrap().join("runtime/RelWithDebInfo")
        } else {
            output.to_path_buf()
        };
        if built {
            assert_eq!(
                dunce::canonicalize(cwd["value"].as_str().unwrap()).unwrap(),
                dunce::canonicalize(expected_cwd).unwrap()
            );
        }
    }
}

fn expected_cases(name: &str) -> Vec<usize> {
    match name {
        "test_color_correction_16f" => vec![3],
        "test_frontend_screenshot_srgb" => vec![7],
        "test_scroll_filter_wide_source" => vec![6],
        "test_rtmps_upgrade" => vec![9],
        "test_slideshow_manual_end" => vec![6],
        "test_x11_volume_keys" => vec![1],
        _ => panic!("unexpected fixture {name}"),
    }
}

fn assert_cases(output: &std::process::Output, counts: Vec<usize>) {
    assert_no_skips(output);
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !text.contains("only its name is checked"),
        "incomplete X11 key checks: {text}"
    );
    let actual: Vec<usize> = text
        .lines()
        .filter_map(|line| {
            line.split_once("[  PASSED  ] ")
                .map(|(_, summary)| summary.split_whitespace().next().unwrap().parse().unwrap())
        })
        .collect();
    assert_eq!(actual, counts, "{text}");
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
