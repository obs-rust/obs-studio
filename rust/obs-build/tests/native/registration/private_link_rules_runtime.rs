use super::*;
use obs_build::validation::graph::Dependency;
use std::process::{Command, Output};

fn rust_archive() -> PathBuf {
    PathBuf::from(std::env::var_os("OBS_BUILD_RUST_ARCHIVE").expect("production archive required"))
}

fn checked(command: &mut Command) -> Output {
    let result = command.output().unwrap();
    assert!(
        result.status.success(),
        "{command:?}\n{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    result
}

fn assert_cases(output: &Output, name: &str) {
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
    .replace("\r\n", "\n");
    let cases = match name {
        "test_graphics_state_stacks_rust" => 5,
        "test_window_match_steps_rust" => 20,
        _ => panic!("unknown fixture {name}"),
    };
    assert!(
        text.contains(&format!("[  PASSED  ] {cases} test(s).")),
        "{name}: {text}"
    );
    assert!(!text.contains("[  SKIPPED ]"), "{name}: {text}");
    assert!(!text.contains("[  FAILED  ]"), "{name}: {text}");
}

fn assert_binding(binary: &Path, core: &Path) {
    let result = checked(Command::new("ldd").arg(binary));
    let text = String::from_utf8_lossy(&result.stdout);
    assert!(!text.contains("not found"), "{text}");
    if let Some(selected) = text
        .lines()
        .find(|line| line.trim_start().starts_with("libobs.so"))
    {
        let loaded = selected
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
    } else {
        // The original window-match archive closure may lose the public DSO
        // under as-needed; unresolved libraries and wrong retained cores fail.
        assert_eq!(binary.file_name().unwrap(), "test_window_match_steps_rust");
        eprintln!(
            "Static-only fixture {}: no libobs DSO retained",
            binary.display()
        );
    }
}

#[test]
fn canonical_private_link_rules_run_unchanged() {
    let root = dunce::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
    let mut graph = graph();
    for target in graph.targets.values_mut() {
        target.link_options.extend([
            "-Wl,--no-copy-dt-needed-entries".into(),
            "-Wl,--as-needed".into(),
        ]);
    }
    let directory = tempfile::tempdir().unwrap();
    let build = NativeBuild {
        host: obs_build::BUILD_HOST.into(),
        target: obs_build::BUILD_TARGET.into(),
        output: directory.path().join("cargo-output"),
        profile: Default::default(),
    };
    let core = built_core(&build, &root);
    let core_path = core.dependency.artifacts[0].clone();
    let caption = obs_build::libobs::build_caption(&build, &root).unwrap();
    let interfaces = [
        ("OBS::libobs".into(), core.dependency),
        (
            "OBS::caption".into(),
            Dependency {
                artifacts: vec![caption.clone()],
                include_dirs: vec![root.join("deps/libcaption")],
                ..Default::default()
            },
        ),
        (
            "OBS::native-test-cmocka".into(),
            Dependency::package(&build.target, "cmocka").unwrap(),
        ),
        (
            "libobs_rust-static".into(),
            Dependency::rust_static(&build.target, &rust_archive()).unwrap(),
        ),
        (
            "LibUUID::LibUUID".into(),
            Dependency::package(&build.target, "uuid").unwrap(),
        ),
        (
            "PkgConfig::_window_match_test_gio".into(),
            Dependency::package(&build.target, "gio-2.0").unwrap(),
        ),
        (
            "m".into(),
            Dependency {
                link_options: vec!["-lm".into()],
                ..Default::default()
            },
        ),
    ]
    .into();
    let facts = [
        ("OS_WINDOWS".into(), false),
        ("OS_LINUX".into(), true),
        ("MSVC".into(), false),
        ("HAVE_LIBOBS_RUST_STATIC".into(), true),
        ("HAVE_WINDOW_MATCH_TEST_GIO".into(), true),
    ]
    .into();
    let artifacts = graph.compile(&build, &root, &facts, &interfaces).unwrap();
    assert_eq!(artifacts.len(), CASES.len());
    for (name, artifact) in &artifacts {
        assert_binding(artifact, &core_path);
        let output = checked(
            graph.tests[name]
                .command(&root, &build.output, artifact)
                .unwrap()
                .env("CMOCKA_MESSAGE_OUTPUT", "STDOUT"),
        );
        assert_cases(&output, name);
        eprintln!(
            "Cargo {name}: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let mut selected = expected();
    selected["schema"] = 1.into();
    fs::write(directory.path().join("graph.json"), selected.to_string()).unwrap();
    replay_cmake(directory.path(), &root, &caption, &core.cmake, &core_path);
}

fn write_cmake(directory: &Path, root: &Path, caption: &Path, core: &str) {
    let policy = fs::read_to_string(root.join("test/cmocka/CMakeLists.txt")).unwrap();
    let policy = policy.split_once("function(obs_test_sanitize ").unwrap().1;
    let policy = policy.split_once("endfunction()").unwrap().0;
    fs::write(
        directory.join("sanitize.cmake"),
        format!("function(obs_test_sanitize {policy}endfunction()\n"),
    )
    .unwrap();
    fs::write(directory.join("CMakeLists.txt"), format!(r#"cmake_minimum_required(VERSION 3.28)
project(private-hosts-contract C)
enable_testing()
option(OS_WINDOWS "Windows guard" OFF)
option(OS_LINUX "Linux guard" ON)
option(OS_MACOS "macOS guard" OFF)
option(TEST_ARCHIVE "Production archive" ON)
set(CMAKE_MODULE_PATH "{root}/cmake/finders")
option(OBS_TEST_SANITIZE_PER_TARGET "Per-target ASan/UBSan" ON)
include("${{CMAKE_CURRENT_SOURCE_DIR}}/sanitize.cmake")
{core}
find_package(PkgConfig REQUIRED)
pkg_check_modules(CMOCKA REQUIRED IMPORTED_TARGET cmocka)
add_library(OBS::native-test-cmocka ALIAS PkgConfig::CMOCKA)
add_library(OBS::caption STATIC IMPORTED)
set_target_properties(OBS::caption PROPERTIES IMPORTED_LOCATION "{caption}" INTERFACE_INCLUDE_DIRECTORIES "{root}/deps/libcaption")
if(TEST_ARCHIVE)
  add_library(libobs_rust-static STATIC IMPORTED)
  set_target_properties(libobs_rust-static PROPERTIES IMPORTED_LOCATION "{archive}" INTERFACE_LINK_LIBRARIES "dl;pthread;m")
endif()
add_library(OBS::w32-pthreads INTERFACE IMPORTED)
include("{root}/test/cmocka/tests.d/test_graphics_state_stacks.cmake")
include("{root}/test/cmocka/tests.d/test_window_match_steps.cmake")
if(TEST_MSVC)
  set(MSVC TRUE)
endif()
if(TEST_NO_GIO)
  set(HAVE_WINDOW_MATCH_TEST_GIO FALSE)
endif()
add_link_options("LINKER:--no-copy-dt-needed-entries" "LINKER:--as-needed")
include("{root}/test/cmocka/native-test-graph.cmake")
obs_native_test_graph("${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json" "{root}")
if(TARGET test_graphics_state_stacks_rust)
  get_target_property(flags test_graphics_state_stacks_rust LINK_OPTIONS)
  file(WRITE "${{CMAKE_CURRENT_BINARY_DIR}}/link-flags.txt" "${{flags}}")
endif()
if(TARGET test_window_match_steps_rust)
  get_target_property(libs test_window_match_steps_rust LINK_LIBRARIES)
  file(WRITE "${{CMAKE_CURRENT_BINARY_DIR}}/window-libs.txt" "${{libs}}")
endif()
"#, root=root.display(), caption=caption.display(), archive=rust_archive().display())).unwrap();
}

fn replay_cmake(directory: &Path, root: &Path, caption: &Path, core: &str, core_path: &Path) {
    write_cmake(directory, root, caption, core);
    for (mode, options, names) in [
        ("enabled", vec![], CASES.to_vec()),
        ("no-archive", vec!["-DTEST_ARCHIVE=OFF"], vec![]),
        (
            "windows",
            vec!["-DOS_WINDOWS=ON", "-DOS_LINUX=OFF", "-DTEST_MSVC=ON"],
            vec![CASES[0]],
        ),
        ("other-host", vec!["-DOS_LINUX=OFF"], vec![CASES[0]]),
        ("no-gio", vec!["-DTEST_NO_GIO=ON"], CASES.to_vec()),
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
        let inventory = checked(
            Command::new("ctest")
                .arg("--test-dir")
                .arg(&output)
                .arg("--show-only=json-v1"),
        );
        let inventory: serde_json::Value = serde_json::from_slice(&inventory.stdout).unwrap();
        let tests = inventory["tests"].as_array().unwrap();
        let actual: Vec<_> = tests
            .iter()
            .map(|test| test["name"].as_str().unwrap())
            .collect();
        assert_eq!(actual, names, "{mode}");
        for test in tests {
            assert!(
                !test["properties"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|p| p["name"] == "SKIP_RETURN_CODE")
            );
        }
        if mode != "no-archive" {
            let flags = fs::read_to_string(output.join("link-flags.txt")).unwrap();
            for flag in [
                "/FORCE:MULTIPLE",
                "/INCREMENTAL:NO",
                "/WX:NO",
                "/IGNORE:4006",
                "/IGNORE:4075",
            ] {
                assert_eq!(
                    flags.split(';').any(|f| f == flag),
                    mode == "windows",
                    "{mode}/{flags}"
                );
            }
        }
        if mode == "enabled" || mode == "no-gio" {
            let libraries = fs::read_to_string(output.join("window-libs.txt")).unwrap();
            assert!(libraries.contains("LibUUID::LibUUID"));
            assert_eq!(
                libraries.contains("PkgConfig::_window_match_test_gio"),
                mode == "enabled"
            );
        }
        if mode == "enabled" {
            checked(
                Command::new("cmake")
                    .arg("--build")
                    .arg(&output)
                    .args(["--parallel", "4"]),
            );
            for name in names {
                assert_binding(&output.join(name), core_path);
                let result = checked(
                    Command::new("ctest")
                        .arg("--test-dir")
                        .arg(&output)
                        .args(["-V", "--no-tests=error", "-R"])
                        .arg(format!("^{name}$"))
                        .env("CMOCKA_MESSAGE_OUTPUT", "STDOUT"),
                );
                assert_cases(&result, name);
                eprintln!("CMake {name}: {}", String::from_utf8_lossy(&result.stdout));
            }
        }
    }
}

#[test]
fn conditional_link_options_reach_only_the_selected_linker() {
    let directory = tempfile::tempdir().unwrap();
    fs::write(
        directory.path().join("main.c"),
        "int main(void) { return 0; }\n",
    )
    .unwrap();
    let graph = Graph::parse(&serde_json::json!({
        "schema": 1,
        "targets": {"link_probe": {"sources": ["main.c"],
            "conditional_link_options": [{"name": "--obs-invalid-selected-link-option", "requires": "REJECT_LINK"}]}},
        "tests": {"link_probe": {"target": "link_probe"}}
    }).to_string()).unwrap();
    for reject in [false, true] {
        let build = NativeBuild {
            host: obs_build::BUILD_HOST.into(),
            target: obs_build::BUILD_TARGET.into(),
            output: directory
                .path()
                .join(if reject { "rejected" } else { "accepted" }),
            profile: Default::default(),
        };
        let result = graph.compile(
            &build,
            directory.path(),
            &[("REJECT_LINK".into(), reject)].into(),
            &Default::default(),
        );
        assert_eq!(result.is_err(), reject, "{result:?}");
    }
}
