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
            .any(|stream| String::from_utf8_lossy(stream)
                .to_ascii_lowercase()
                .contains("skipped")),
        "skipped native cases: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn canonical_display_startup_runs_unchanged() {
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
    let renderer = obs_build::graphics::build_opengl(
        &build,
        &root,
        &core.dependency.artifacts[0],
        &Default::default(),
    )
    .unwrap();
    let mut interfaces: std::collections::BTreeMap<String, Dependency> = [
        ("OBS::libobs".into(), core.dependency),
        (
            "OBS::native-test-cmocka".into(),
            Dependency::package(&build.target, "cmocka").unwrap(),
        ),
    ]
    .into();
    interfaces.insert(
        "OBS::libobs-opengl".into(),
        Dependency {
            artifacts: vec![renderer.clone()],
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
    interfaces.insert("OBS::caption".into(), Dependency::default());

    let facts = [
        ("OS_LINUX".into(), true),
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
    write_cmake(directory.path(), &root, &runtime, &core.cmake, &renderer);
    replay_cmake(directory.path());
}

fn write_cmake(directory: &Path, root: &Path, runtime: &Path, core: &str, renderer: &Path) {
    let source = root.to_string_lossy().replace('\\', "/");
    fs::write(
        directory.join("CMakeLists.txt"),
        format!(
            r#"cmake_minimum_required(VERSION 3.28)
project(display-startup-contract C)
set(CMAKE_C_VISIBILITY_PRESET hidden)
enable_testing()
option(OS_LINUX "Original Linux guard" ON)
option(HAVE_OBS_OPENGL "Original renderer guard" ON)
set(OBS_OUTPUT_DIR "{runtime}")
find_package(PkgConfig REQUIRED)
pkg_check_modules(CMOCKA REQUIRED IMPORTED_TARGET cmocka)
{core}
add_library(OBS::native-test-cmocka ALIAS PkgConfig::CMOCKA)
add_library(OBS::caption INTERFACE IMPORTED)
if(HAVE_OBS_OPENGL)
  add_library(OBS::libobs-opengl MODULE IMPORTED)
  set_property(TARGET OBS::libobs-opengl PROPERTY IMPORTED_LOCATION "{renderer}")
endif()
include("{source}/test/cmocka/native-test-graph.cmake")
obs_native_test_graph("${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json" "{source}")
"#,
            runtime = runtime.display(),
            renderer = renderer.display()
        ),
    )
    .unwrap();
}

fn replay_cmake(directory: &Path) {
    for (mode, options, expected) in [
        ("enabled", vec![], CASES.to_vec()),
        (
            "windows",
            vec!["-DOS_LINUX=OFF", "-DOS_WINDOWS=ON"],
            vec!["test_startup_alloc_stable"],
        ),
        (
            "macos",
            vec!["-DOS_LINUX=OFF", "-DOS_MACOS=ON"],
            vec!["test_startup_alloc_stable"],
        ),
        ("no-display", vec![], vec![]),
        (
            "no-renderer",
            vec!["-DHAVE_OBS_OPENGL=OFF"],
            CASES
                .into_iter()
                .filter(|name| {
                    !name.starts_with("test_async_") && *name != "test_gl_dynamic_texture"
                })
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
        if mode == "no-display" {
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
            let result = checked(
                Command::new("ctest")
                    .arg("--test-dir")
                    .arg(&output)
                    .args(["--output-on-failure", "--no-tests=error", "-V"])
                    .env("CMOCKA_MESSAGE_OUTPUT", "STDOUT"),
            );
            assert_no_skips(&result);
            assert_eq!(
                String::from_utf8_lossy(&result.stdout)
                    .lines()
                    .filter(|line| line.contains("[  PASSED  ]"))
                    .count(),
                CASES.len()
            );
            for count in [1, 2, 3, 4, 6] {
                assert!(
                    String::from_utf8_lossy(&result.stdout)
                        .contains(&format!("[  PASSED  ] {count} test(s)."))
                );
            }
            eprintln!("CMake: {}", String::from_utf8_lossy(&result.stdout));
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
            let wrapped = name == "test_startup_alloc_stable";
            assert_eq!(command.len(), if wrapped { 3 } else { 1 });
            if wrapped {
                assert_eq!(
                    Path::new(command[0].as_str().unwrap()).file_name().unwrap(),
                    "xvfb-run"
                );
                assert_eq!(command[1], "-a");
            }
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
        if name.starts_with("test_async_") {
            assert!(
                locks.unwrap()["value"]
                    .as_array()
                    .unwrap()
                    .contains(&serde_json::json!("timing"))
            );
        } else {
            assert!(locks.is_none());
        }
    }
}

fn expected_cases(name: &str) -> usize {
    match name {
        "test_async_audio_own_clock" => 4,
        "test_async_delay_audio" => 6,
        "test_gl_dynamic_texture" => 2,
        "test_nix_startup_log" => 3,
        _ => 1,
    }
}

fn assert_cases(output: &std::process::Output, count: usize) {
    assert_no_skips(output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(&format!("[  PASSED  ] {count} test(s).")),
        "{stdout}"
    );
}
