use super::*;
#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
use std::path::PathBuf;

const CASES: &[(&str, bool)] = &[
    ("test_async_upload_steps", true),
    ("test_deinterlace_draw_plan_steps", true),
    ("test_scene_render_plan_steps", true),
    ("test_source_audio_place_steps", false),
    ("test_source_draw_plan_steps", true),
    ("test_source_media_admission_steps", true),
    ("test_source_monitoring_steps", true),
    ("test_source_render_route_steps", true),
    ("test_source_video_tick_steps", false),
    ("test_transition_load_steps", false),
];

#[test]
fn canonical_rust_static_header_steps_are_registered() {
    use obs_build::validation::graph::{Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph
        .targets
        .retain(|key, _| CASES.iter().any(|(base, _)| key == &format!("{base}_rust")));
    graph
        .tests
        .retain(|key, _| CASES.iter().any(|(base, _)| key == &format!("{base}_rust")));
    for &(base, math) in CASES {
        let name = format!("{base}_rust");
        let target = graph
            .targets
            .get(&name)
            .expect("missing Rust-static header steps");
        assert!(matches!(target.sources.as_slice(), [Source::Path(path)]
            if path == &format!("test/cmocka/{base}.c")));
        assert_eq!(target.includes, ["libobs"]);
        assert_eq!(target.header_libraries, ["OBS::libobs", "SIMDe::SIMDe"]);
        assert!(target.archives.is_empty());
        assert!(target.definitions.is_empty());
        assert!(target.compile_options.is_empty());
        assert!(target.link_options.is_empty());
        assert_eq!(target.sanitizers, None);
        assert_header_step_linkage(target, math, base == "test_scene_render_plan_steps");
        let test = &graph.tests[&name];
        assert_eq!(test.target, name);
        assert_eq!(test.command, ["${target}"]);
        assert_eq!(test.timeout_seconds, None);
        assert_eq!(test.skip_return_code, None);
        let fragment =
            fs::read_to_string(root.join(format!("test/cmocka/tests.d/{base}.cmake"))).unwrap();
        assert!(!fragment.contains("add_executable("));
        assert!(!fragment.contains("add_test("));
        assert_header_step_selection(&graph, &name);
    }
}

fn assert_header_step_linkage(
    target: &obs_build::validation::graph::Target,
    math: bool,
    unix_math: bool,
) {
    use obs_build::validation::graph::{ConditionalValue, Requirement};
    assert_eq!(target.libraries.len(), if math { 3 } else { 2 });
    assert!(matches!(&target.libraries[0], ConditionalValue::Name(n) if n == "libobs_rust-static"));
    assert!(
        matches!(&target.libraries[1], ConditionalValue::Name(n) if n == "OBS::native-test-cmocka")
    );
    if unix_math {
        assert!(
            matches!(&target.libraries[2], ConditionalValue::Conditional { name, requires: Requirement::Fact(fact) }
            if name == "m" && fact == "UNIX")
        );
    } else if math {
        assert!(
            matches!(&target.libraries[2], ConditionalValue::Conditional { name, requires: Requirement::Not { not } }
                if name == "m" && matches!(not.as_ref(), Requirement::Fact(fact) if fact == "MSVC"))
        );
    }
}

fn assert_header_step_selection(graph: &obs_build::validation::graph::Graph, name: &str) {
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        for available in [false, true] {
            let facts = [
                (os.into(), true),
                ("MSVC".into(), os == "OS_WINDOWS"),
                ("UNIX".into(), os != "OS_WINDOWS"),
                ("HAVE_LIBOBS_RUST_STATIC".into(), available),
            ]
            .into();
            let selected = graph.select(&facts).unwrap();
            assert_eq!(selected.targets.contains_key(name), available);
            assert_eq!(selected.tests.contains_key(name), available);
        }
    }
}

#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
fn checked(program: &str, arguments: &[&str]) -> std::process::Output {
    let result = std::process::Command::new(program)
        .args(arguments)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{program} {arguments:?}\n{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    result
}

#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
fn replay_cmake(
    directory: &Path,
    root: &Path,
    archive: &Path,
    selected: &serde_json::Value,
) -> Vec<PathBuf> {
    fs::write(directory.join("graph.json"), selected.to_string()).unwrap();
    let source = root.to_string_lossy().replace(char::from(92), "/");
    let archive = archive.to_string_lossy().replace(char::from(92), "/");
    fs::write(
        directory.join("CMakeLists.txt"),
        format!(
            r#"cmake_minimum_required(VERSION 3.28)
project(rust-static-header-steps C)
set(MSVC FALSE)
list(APPEND CMAKE_MODULE_PATH "{source}/cmake/finders")
find_package(SIMDe REQUIRED)
get_target_property(simde_includes SIMDe::SIMDe INTERFACE_INCLUDE_DIRECTORIES)
file(WRITE "${{CMAKE_CURRENT_BINARY_DIR}}/simde-includes.txt" "${{simde_includes}}")
# Mirror only libobs's public header interface; no core symbols or stubs.
configure_file("{source}/libobs/obsconfig.h.in" "${{CMAKE_CURRENT_BINARY_DIR}}/config/obsconfig.h" @ONLY)
add_library(libobs INTERFACE)
add_library(OBS::libobs ALIAS libobs)
target_include_directories(libobs INTERFACE "{source}/libobs" "${{CMAKE_CURRENT_BINARY_DIR}}/config")
enable_testing()
find_package(PkgConfig REQUIRED)
pkg_check_modules(CMOCKA REQUIRED IMPORTED_TARGET cmocka)
add_library(OBS::native-test-cmocka ALIAS PkgConfig::CMOCKA)
option(WITH_RUST "Import the real production archive" ON)
if(WITH_RUST)
  add_library(libobs_rust-static STATIC IMPORTED)
  set_target_properties(libobs_rust-static PROPERTIES IMPORTED_LOCATION "{archive}")
  if(NOT MSVC)
    target_link_libraries(libobs_rust-static INTERFACE m)
  endif()
endif()
include("{source}/test/cmocka/native-test-graph.cmake")
obs_native_test_graph("${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json" "{source}")
"#
        ),
    )
    .unwrap();
    let output = directory.join("cmake-output");
    let output = output.to_str().unwrap();
    checked("cmake", &["-S", directory.to_str().unwrap(), "-B", output]);
    checked(
        "cmake",
        &["--build", output, "--config", "Release", "--parallel", "2"],
    );
    checked(
        "ctest",
        &[
            "--test-dir",
            output,
            "-C",
            "Release",
            "--output-on-failure",
            "--no-tests=error",
        ],
    );
    for available in [false, true] {
        checked(
            "cmake",
            &[
                "-S",
                directory.to_str().unwrap(),
                "-B",
                output,
                if available {
                    "-DWITH_RUST=ON"
                } else {
                    "-DWITH_RUST=OFF"
                },
            ],
        );
        let inventory = checked("ctest", &["--test-dir", output, "--show-only=json-v1"]);
        let inventory: serde_json::Value = serde_json::from_slice(&inventory.stdout).unwrap();
        let tests = inventory["tests"].as_array().unwrap();
        assert_eq!(tests.len(), if available { CASES.len() } else { 0 });
        for test in tests {
            assert!(
                CASES
                    .iter()
                    .any(|(base, _)| test["name"] == format!("{base}_rust"))
            );
            assert!(
                !test["properties"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|p| p["name"] == "TIMEOUT")
            );
        }
    }
    let mut includes = vec![directory.join("cmake-output/config")];
    includes.extend(
        fs::read_to_string(directory.join("cmake-output/simde-includes.txt"))
            .unwrap()
            .split(';')
            .map(PathBuf::from),
    );
    includes
}

/// #1390: run unchanged archive-only characterizations through both real consumers.
#[test]
#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
fn canonical_rust_static_header_steps_run_unchanged() {
    use obs_build::validation::graph::{Dependency, Graph};
    let root = dunce::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
    let archive = dunce::canonicalize(
        std::env::var_os("OBS_BUILD_RUST_ARCHIVE")
            .expect("set OBS_BUILD_RUST_ARCHIVE to the production staticlib"),
    )
    .unwrap();
    assert!(archive.is_file());
    let directory = tempfile::tempdir().unwrap();
    let raw: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap())
            .unwrap();
    let mut selected = serde_json::json!({"schema": 1, "targets": {}, "tests": {}});
    for &(base, _) in CASES {
        for section in ["targets", "tests"] {
            let name = format!("{base}_rust");
            selected[section][&name] = raw[section][&name].clone();
        }
    }
    let mut includes = replay_cmake(directory.path(), &root, &archive, &selected);
    let config = includes.remove(0);
    let graph = Graph::parse(&selected.to_string()).unwrap();
    let build = NativeBuild {
        host: obs_build::BUILD_HOST.into(),
        target: obs_build::BUILD_TARGET.into(),
        output: directory.path().join("cargo-output"),
        profile: Default::default(),
    };
    let interfaces = [
        (
            "OBS::libobs".into(),
            Dependency {
                include_dirs: vec![root.join("libobs"), config],
                ..Default::default()
            },
        ),
        (
            "SIMDe::SIMDe".into(),
            Dependency {
                include_dirs: includes,
                ..Default::default()
            },
        ),
        (
            "m".into(),
            Dependency {
                link_options: vec!["-lm".into()],
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
    ]
    .into();
    let facts = [
        ("HAVE_LIBOBS_RUST_STATIC".into(), true),
        ("MSVC".into(), false),
        ("UNIX".into(), true),
    ]
    .into();
    for target in graph.targets.values() {
        assert_eq!(
            target.dependency_artifacts(&facts, &interfaces).unwrap(),
            std::slice::from_ref(&archive)
        );
    }
    let artifacts = graph.compile(&build, &root, &facts, &interfaces).unwrap();
    assert_eq!(artifacts.len(), CASES.len());
    for (name, test) in &graph.tests {
        let result = test
            .command(&root, &build.output, &artifacts[name])
            .unwrap()
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{name}\n{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
}
