use super::*;
use obs_build::validation::graph::Dependency;
use std::path::PathBuf;
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

#[test]
fn canonical_private_capture_runs_unchanged() {
    assert!(std::env::var_os("DISPLAY").is_some_and(|v| !v.is_empty()));
    let root = dunce::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let raw: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap())
            .unwrap();
    let mut selected = expected();
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
    let mut core = built_core(&build, &root);
    core.dependency.compile_options.clear();
    core.cmake
        .push_str("set_property(TARGET OBS::libobs PROPERTY INTERFACE_COMPILE_OPTIONS \"\")\n");
    let core_path = core.dependency.artifacts[0].clone();
    let renderer =
        obs_build::graphics::build_opengl(&build, &root, &core_path, &Default::default()).unwrap();
    assert_binding(&renderer, &core_path);
    let glad = build_glad(directory.path(), &root);
    let interfaces = interfaces(core.dependency, &build, &root, &glad, &renderer);
    let facts = [
        ("OS_LINUX".into(), true),
        ("HAVE_PIPEWIRE_CAMERA_TEST_OWNER".into(), true),
        ("HAVE_X11_DISPLAY".into(), true),
        ("HAVE_V4L2_VMIC_TEST_OWNER".into(), true),
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
    fs::write(directory.path().join("graph.json"), selected.to_string()).unwrap();
    write_cmake(
        directory.path(),
        &root,
        &runtime,
        &core.cmake,
        &renderer,
        &glad,
    );
    replay_cmake(directory.path(), &runtime, &core_path);
    for name in CASES {
        assert_binding(&artifacts[name], &core_path);
        let result = checked(
            graph.tests[name]
                .command_in(&root, &build.output, &artifacts[name], &runtime)
                .unwrap()
                .env("CMOCKA_MESSAGE_OUTPUT", "STDOUT"),
        );
        assert_cases(&result, name);
        eprintln!(
            "Cargo {name}: {}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
}

fn build_glad(directory: &Path, root: &Path) -> PathBuf {
    let support = directory.join("support");
    fs::create_dir_all(&support).unwrap();
    fs::write(
        support.join("CMakeLists.txt"),
        format!(
            r#"cmake_minimum_required(VERSION 3.28)
project(camera-glad C)
add_library(camera-glad STATIC "{root}/deps/glad/src/glad.c")
target_include_directories(camera-glad PRIVATE "{root}/deps/glad/include")
"#,
            root = root.display()
        ),
    )
    .unwrap();
    checked(
        Command::new("cmake")
            .arg("-S")
            .arg(&support)
            .arg("-B")
            .arg(support.join("build"))
            .args(["-G", "Ninja"]),
    );
    checked(
        Command::new("cmake")
            .arg("--build")
            .arg(support.join("build")),
    );
    support.join("build/libcamera-glad.a")
}

fn write_cmake(
    directory: &Path,
    root: &Path,
    runtime: &Path,
    core: &str,
    renderer: &Path,
    glad: &Path,
) {
    fs::write(directory.join("CMakeLists.txt"), format!(r#"cmake_minimum_required(VERSION 3.28)
project(camera-daemon-contract C)
enable_testing()
option(OS_LINUX "Linux" ON)
option(OS_WINDOWS "Windows" OFF)
option(OS_MACOS "macOS" OFF)
option(WITH_OWNER "PipeWire plugin owner" ON)
set(CMAKE_MODULE_PATH "{root}/cmake/finders")
set(OBS_OUTPUT_DIR "{runtime}")
{core}
find_package(PkgConfig REQUIRED)
pkg_check_modules(CMOCKA REQUIRED IMPORTED_TARGET cmocka)
add_library(OBS::native-test-cmocka ALIAS PkgConfig::CMOCKA)
find_package(PipeWire 0.3.60 REQUIRED)
find_package(Gio REQUIRED)
add_link_options("LINKER:--no-copy-dt-needed-entries" "LINKER:--as-needed")
add_library(OBS::glad STATIC IMPORTED)
set_target_properties(OBS::glad PROPERTIES IMPORTED_LOCATION "{glad}" INTERFACE_INCLUDE_DIRECTORIES "{root}/deps/glad/include")
add_library(OBS::libobs-opengl MODULE IMPORTED)
set_property(TARGET OBS::libobs-opengl PROPERTY IMPORTED_LOCATION "{renderer}")
if(NOT DEFINED HAVE_V4L2_VMIC_TEST_OWNER)
  set(HAVE_V4L2_VMIC_TEST_OWNER ${{WITH_OWNER}})
endif()
if(WITH_OWNER)
  add_library(OBS::pipewire INTERFACE IMPORTED)
endif()
include("{root}/test/cmocka/native-test-graph.cmake")
obs_native_test_graph("${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json" "{root}")
"#, root=root.display(), runtime=runtime.display(), renderer=renderer.display(), glad=glad.display())).unwrap();
}

fn replay_cmake(directory: &Path, runtime: &Path, core: &Path) {
    for (mode, options, names) in [
        ("enabled", vec![], CASES.to_vec()),
        (
            "windows",
            vec![
                "-DOS_LINUX=OFF",
                "-DOS_WINDOWS=ON",
                "-DHAVE_V4L2_VMIC_TEST_OWNER=OFF",
            ],
            vec![],
        ),
        (
            "macos",
            vec![
                "-DOS_LINUX=OFF",
                "-DOS_MACOS=ON",
                "-DHAVE_V4L2_VMIC_TEST_OWNER=OFF",
            ],
            vec![],
        ),
        ("no-owner", vec!["-DWITH_OWNER=OFF"], vec![]),
        (
            "no-daemons",
            vec!["-DHAVE_PIPEWIRE_CAMERA_TEST_OWNER=OFF"],
            vec!["test_v4l2_vmic_server"],
        ),
        (
            "no-display",
            vec!["-DHAVE_X11_DISPLAY=OFF"],
            vec!["test_v4l2_vmic_server"],
        ),
    ] {
        let output = directory.join(mode);
        checked(
            Command::new("cmake")
                .arg("-S")
                .arg(directory)
                .arg("-B")
                .arg(&output)
                .args(["-G", "Ninja", "-DCMAKE_BUILD_TYPE=RelWithDebInfo"])
                .args(options),
        );
        if mode == "enabled" {
            checked(
                Command::new("cmake")
                    .arg("--build")
                    .arg(&output)
                    .args(["--parallel", "4"]),
            );
            for name in &names {
                assert_binding(&output.join(name), core);
                let result = checked(
                    Command::new("ctest")
                        .arg("--test-dir")
                        .arg(&output)
                        .args(["-V", "--no-tests=error", "-R"])
                        .arg(format!("^{name}$"))
                        .env("CMOCKA_MESSAGE_OUTPUT", "STDOUT"),
                );
                assert_cases(&result, name);
                assert!(
                    String::from_utf8_lossy(&result.stdout)
                        .contains("100% tests passed, 0 tests failed out of 1")
                );
                eprintln!("CMake {name}: {}", String::from_utf8_lossy(&result.stdout));
            }
        }
        check_inventory(&output, runtime, &names, mode == "enabled");
        eprintln!("Inventory {mode}: {}", names.len());
    }
}

fn assert_cases(output: &Output, name: &str) {
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
        "test_pipewire_camera_unplug" => 2,
        "test_pipewire_cursor_hdr" => 4,
        "test_v4l2_vmic_server" => 3,
        _ => panic!("unexpected fixture {name}"),
    };
    let actual: Vec<usize> = text
        .lines()
        .filter_map(|line| {
            line.split_once("[  PASSED  ] ")
                .map(|(_, summary)| summary.split_whitespace().next().unwrap().parse().unwrap())
        })
        .collect();
    assert_eq!(actual, [expected], "{text}");
}

fn assert_binding(executable: &Path, core: &Path) {
    let result = checked(Command::new("ldd").arg(executable));
    let text = String::from_utf8_lossy(&result.stdout);
    assert!(
        text.contains(core.to_str().unwrap()),
        "{} must load {}: {text}",
        executable.display(),
        core.display()
    );
    eprintln!(
        "Loader {}: {}\n{text}",
        executable.display(),
        core.display()
    );
}

fn check_inventory(output: &Path, runtime: &Path, names: &[&str], built: bool) {
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
        assert!(names.contains(&name));
        let record = &expected["tests"][name];
        let properties = test["properties"].as_array().unwrap();
        let property = |name: &str| {
            properties
                .iter()
                .find(|p| p["name"] == name)
                .map(|p| &p["value"])
        };
        assert_eq!(
            property("TIMEOUT").and_then(serde_json::Value::as_f64),
            record["timeout_seconds"].as_f64()
        );
        assert_eq!(
            property("RESOURCE_LOCK")
                .cloned()
                .unwrap_or_else(|| serde_json::json!([])),
            record
                .get("resource_locks")
                .cloned()
                .unwrap_or_else(|| serde_json::json!([]))
        );
        assert_eq!(
            property("SKIP_RETURN_CODE").and_then(serde_json::Value::as_f64),
            record["skip_return_code"].as_f64()
        );
        let mut environment: Vec<String> = record
            .get("environment")
            .and_then(serde_json::Value::as_object)
            .map(|values| {
                values
                    .iter()
                    .map(|(key, value)| format!("{key}={}", value.as_str().unwrap()))
                    .collect()
            })
            .unwrap_or_default();
        environment.sort();
        let mut actual: Vec<String> = property("ENVIRONMENT")
            .and_then(serde_json::Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .map(|v| v.as_str().unwrap().to_owned())
                    .collect()
            })
            .unwrap_or_default();
        actual.sort();
        assert_eq!(actual, environment, "{name}");
        if built {
            assert_eq!(
                normalized_command(&test["command"]),
                record["command"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| {
                        v.as_str()
                            .unwrap()
                            .replace(
                                "${source}",
                                &dunce::canonicalize(
                                    Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."),
                                )
                                .unwrap()
                                .to_string_lossy(),
                            )
                            .replace("${target}", &output.join(name).to_string_lossy())
                    })
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                dunce::canonicalize(property("WORKING_DIRECTORY").unwrap().as_str().unwrap())
                    .unwrap(),
                dunce::canonicalize(if record["working_directory"] == "${runtime}" {
                    runtime.join("RelWithDebInfo")
                } else {
                    output.to_path_buf()
                })
                .unwrap()
            );
        }
    }
}

fn normalized_command(command: &serde_json::Value) -> Vec<String> {
    let mut arguments: Vec<String> = command
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap().to_owned())
        .collect();
    if Path::new(&arguments[0]).file_name() == Some(std::ffi::OsStr::new("uv")) {
        let resolved = checked(Command::new("which").arg("uv"));
        assert_eq!(
            dunce::canonicalize(&arguments[0]).unwrap(),
            dunce::canonicalize(String::from_utf8_lossy(&resolved.stdout).trim()).unwrap()
        );
        arguments[0] = "uv".into();
    }
    arguments
}

fn interfaces(
    core: Dependency,
    build: &NativeBuild,
    root: &Path,
    glad: &Path,
    renderer: &Path,
) -> std::collections::BTreeMap<String, Dependency> {
    let mut interfaces = std::collections::BTreeMap::from([("OBS::libobs".into(), core)]);
    for (name, package) in [
        ("OBS::native-test-cmocka", "cmocka"),
        ("PipeWire::PipeWire", "libpipewire-0.3"),
        ("gio::gio", "gio-2.0"),
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
    interfaces.insert(
        "OBS::glad".into(),
        Dependency {
            artifacts: vec![glad.to_path_buf()],
            include_dirs: vec![root.join("deps/glad/include")],
            ..Default::default()
        },
    );
    interfaces.insert(
        "OBS::libobs-opengl".into(),
        Dependency {
            artifacts: vec![renderer.to_path_buf()],
            ..Default::default()
        },
    );
    interfaces
}
