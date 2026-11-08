use super::*;
use obs_build::validation::graph::{Dependency, Graph};
use std::process::{Command, Output};

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
        "test_effect_param_types_steps_rust" | "test_graphics_ffmpeg_route_steps_rust" => 10,
        "test_effect_parser_lifecycle_rust" => 8,
        "test_graphics_ffmpeg_pq_steps_rust" => 9,
        "test_graphics_ffmpeg_wic_checks_rust" => 4,
        "test_nsgif_decode_rust" => 39,
        "test_graphics_immediate_steps_rust" => 11,
        "test_graphics_state_stacks_rust" => 5,
        "test_scene_bounds_place_rust" | "test_scene_item_order_steps_rust" => 16,
        "test_scene_item_names_steps_rust" => 14,
        "test_scene_item_transform_steps_rust" => 13,
        "test_scene_transform_update_steps_rust" => 9,
        "test_pulse_monitor_steps_rust" | "test_sprite_geometry_steps_rust" => 20,
        "test_source_duplicate_steps_rust" => 13,
        "test_source_filter_chain_steps_rust" => 12,
        "test_transition_clock_steps_rust" => 20,
        "test_transition_sample_time_steps_rust" => 9,
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
    let selected = text
        .lines()
        .find(|line| line.trim_start().starts_with("libobs.so"));
    let Some(selected) = selected else {
        // Strict as-needed matches the original private-only scene fixtures:
        // no public libobs symbol is referenced, so the DSO is not retained.
        assert!(matches!(
            binary.file_name().unwrap().to_str().unwrap(),
            "test_scene_bounds_place_rust"
                | "test_scene_item_order_steps_rust"
                | "test_scene_transform_update_steps_rust"
                | "test_pulse_monitor_steps_rust"
                | "test_source_duplicate_steps_rust"
                | "test_source_filter_chain_steps_rust"
                | "test_transition_sample_time_steps_rust"
        ));
        assert!(!text.contains("not found"), "{text}");
        eprintln!(
            "Static-only fixture {}: no libobs DSO retained",
            binary.display()
        );
        return;
    };
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
}

fn strict_graph(family: &str) -> Graph {
    let mut graph = rust_static_entries::assert_family(family);
    for target in graph.targets.values_mut() {
        target.link_options.extend([
            "-Wl,--no-copy-dt-needed-entries".into(),
            "-Wl,--as-needed".into(),
        ]);
    }
    graph
}

pub(super) fn run(family: &str, cases: &[String]) {
    let root = dunce::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
    let archive = dunce::canonicalize(
        std::env::var_os("OBS_BUILD_RUST_ARCHIVE").expect("production Rust archive required"),
    )
    .unwrap();
    let graph = strict_graph(family);
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
        ("Uthash::Uthash".into(), Dependency::default()),
        ("SIMDe::SIMDe".into(), Dependency::default()),
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
            Dependency::rust_static(&build.target, &archive).unwrap(),
        ),
        (
            "PulseAudio::PulseAudio".into(),
            Dependency::package(&build.target, "libpulse").unwrap(),
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
        ("HAVE_LIBOBS_RUST_STATIC".into(), true),
        ("OS_WINDOWS".into(), false),
        ("MSVC".into(), false),
        ("OS_LINUX".into(), true),
        ("OS_FREEBSD".into(), false),
        ("ENABLE_PULSEAUDIO".into(), true),
    ]
    .into();
    let artifacts = graph.compile(&build, &root, &facts, &interfaces).unwrap();
    assert_eq!(artifacts.len(), cases.len());
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
    let raw: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap())
            .unwrap();
    let mut selected = serde_json::json!({"schema":1,"targets":{},"tests":{}});
    for section in ["targets", "tests"] {
        for base in cases.iter() {
            let name = format!("{base}_rust");
            selected[section][&name] = raw[section][&name].clone();
        }
    }
    fs::write(directory.path().join("graph.json"), selected.to_string()).unwrap();
    replay_cmake(
        directory.path(),
        &root,
        &archive,
        &caption,
        &core.cmake,
        &core_path,
        cases,
    );
}

fn replay_cmake(
    directory: &Path,
    root: &Path,
    archive: &Path,
    caption: &Path,
    core: &str,
    core_path: &Path,
    cases: &[String],
) {
    let policy = fs::read_to_string(root.join("test/cmocka/CMakeLists.txt")).unwrap();
    let policy = policy.split_once("function(obs_test_sanitize ").unwrap().1;
    let policy = policy.split_once("endfunction()").unwrap().0;
    fs::write(
        directory.join("sanitize.cmake"),
        format!("function(obs_test_sanitize {policy}endfunction()\n"),
    )
    .unwrap();
    fs::write(directory.join("CMakeLists.txt"), format!(r#"cmake_minimum_required(VERSION 3.28)
project(graphics-private-contract C)
enable_testing()
set(OS_WINDOWS OFF)
set(MSVC OFF)
set(OS_LINUX ON)
set(OS_FREEBSD OFF)
set(ENABLE_PULSEAUDIO ON)
set(CMAKE_MODULE_PATH "{root}/cmake/finders")
find_package(PulseAudio REQUIRED)
find_package(Uthash REQUIRED)
find_package(SIMDe REQUIRED)
option(OBS_TEST_SANITIZE_PER_TARGET "Per-target ASan/UBSan" ON)
include("${{CMAKE_CURRENT_SOURCE_DIR}}/sanitize.cmake")
{core}
find_package(PkgConfig REQUIRED)
pkg_check_modules(CMOCKA REQUIRED IMPORTED_TARGET cmocka)
add_library(OBS::native-test-cmocka ALIAS PkgConfig::CMOCKA)
add_library(OBS::caption STATIC IMPORTED)
set_target_properties(OBS::caption PROPERTIES IMPORTED_LOCATION "{caption}" INTERFACE_INCLUDE_DIRECTORIES "{root}/deps/libcaption")
option(WITH_RUST "Production Rust static archive" ON)
if(WITH_RUST)
 add_library(libobs_rust-static STATIC IMPORTED)
 set_target_properties(libobs_rust-static PROPERTIES IMPORTED_LOCATION "{archive}" INTERFACE_LINK_LIBRARIES "dl;pthread;m")
endif()
add_link_options("LINKER:--no-copy-dt-needed-entries" "LINKER:--as-needed")
include("{root}/test/cmocka/native-test-graph.cmake")
obs_native_test_graph("${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json" "{root}")
"#, root=root.display(), archive=archive.display(), caption=caption.display())).unwrap();
    for enabled in [true, false] {
        let output = directory.join(if enabled { "cmake-output" } else { "no-rust" });
        checked(
            Command::new("cmake")
                .arg("-S")
                .arg(directory)
                .arg("-B")
                .arg(&output)
                .args(["-G", "Ninja", "-DCMAKE_BUILD_TYPE=RelWithDebInfo"])
                .arg(if enabled {
                    "-DWITH_RUST=ON"
                } else {
                    "-DWITH_RUST=OFF"
                }),
        );
        let inventory = checked(
            Command::new("ctest")
                .arg("--test-dir")
                .arg(&output)
                .arg("--show-only=json-v1"),
        );
        let inventory: serde_json::Value = serde_json::from_slice(&inventory.stdout).unwrap();
        let mut names: Vec<_> = inventory["tests"]
            .as_array()
            .unwrap()
            .iter()
            .map(|test| test["name"].as_str().unwrap().to_owned())
            .collect();
        names.sort();
        let expected: Vec<_> = if enabled {
            cases.iter().map(|base| format!("{base}_rust")).collect()
        } else {
            vec![]
        };
        assert_eq!(names, expected);
        if enabled {
            checked(
                Command::new("cmake")
                    .arg("--build")
                    .arg(&output)
                    .args(["--parallel", "4"]),
            );
            for name in names {
                assert_binding(&output.join(&name), core_path);
                let result = checked(
                    Command::new("ctest")
                        .arg("--test-dir")
                        .arg(&output)
                        .args(["-V", "--no-tests=error", "-R"])
                        .arg(format!("^{name}$"))
                        .env("CMOCKA_MESSAGE_OUTPUT", "STDOUT"),
                );
                assert_cases(&result, &name);
                eprintln!("CMake {name}: {}", String::from_utf8_lossy(&result.stdout));
            }
        }
    }
}
