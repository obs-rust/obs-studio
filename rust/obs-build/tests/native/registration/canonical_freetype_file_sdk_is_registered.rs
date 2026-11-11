use super::*;
use obs_build::validation::graph::Graph;

const NAME: &str = "test_text_aa_file";

#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
#[path = "support/freetype_file_sdk_runtime.rs"]
mod runtime;

fn requirement() -> serde_json::Value {
    serde_json::json!({"all": ["OS_LINUX", "HAVE_OBS_TEXT_FREETYPE2", "HAVE_FREETYPE_FILE_SDK"]})
}

fn expected_target() -> serde_json::Value {
    let mut sources = vec![format!("test/cmocka/{NAME}.c")];
    sources.extend(
        [
            "text-freetype2",
            "text-functionality",
            "text-file",
            "glyph-atlas",
            "obs-convenience",
            "find-font-unix",
            "text-freetype2-host-posix",
        ]
        .map(|name| format!("plugins/text-freetype2/{name}.c.h")),
    );
    serde_json::json!({
        "sources": sources,
        "includes": [".", "plugins/text-freetype2"],
        "definitions": ["TEST_FONT_PATH=\"${source}/test/cmocka/data/text-aa.ttf\""],
        "watch_inputs": ["plugins/text-freetype2/build/text-freetype2-nix.pch.h", "test/cmocka/data/text-aa.ttf"],
        "libraries": ["OBS::libobs", "Freetype::Freetype", "Fontconfig::Fontconfig", "OBS::native-test-cmocka"],
        "requires": requirement()
    })
}

#[test]
fn canonical_freetype_file_sdk_is_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let raw: serde_json::Value = obs_build::validation::graph::Graph::read_value(
        &root.join("test/cmocka/build/test-graph.d"),
    )
    .unwrap();
    assert_eq!(raw["targets"][NAME], expected_target());
    assert_eq!(
        raw["tests"][NAME],
        serde_json::json!({"target": NAME, "requires": requirement()})
    );
    let fragment =
        fs::read_to_string(root.join(format!("test/cmocka/tests.d/{NAME}.cmake"))).unwrap();
    assert!(!fragment.contains("add_executable("));
    assert!(!fragment.contains("add_test("));
}

#[test]
fn canonical_freetype_file_sdk_guards() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.d")).unwrap();
    graph.targets.retain(|name, _| name == NAME);
    graph.tests.retain(|name, _| name == NAME);
    assert_eq!(graph.targets.len(), 1);
    for linux in [false, true] {
        for owner in [false, true] {
            for sdk in [false, true] {
                let facts = [
                    ("OS_LINUX".into(), linux),
                    ("HAVE_OBS_TEXT_FREETYPE2".into(), owner),
                    ("HAVE_FREETYPE_FILE_SDK".into(), sdk),
                ]
                .into();
                let selected = graph.select(&facts).unwrap();
                let count = usize::from(linux && owner && sdk);
                assert_eq!(selected.targets.len(), count);
                assert_eq!(selected.tests.len(), count);
            }
        }
    }
}

#[test]
fn canonical_freetype_file_sdk_absent_configures() {
    let root = dunce::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let selected = serde_json::json!({"schema": 1, "targets": {NAME: expected_target()}, "tests": {NAME: {"target": NAME, "requires": requirement()}}});
    fs::write(directory.path().join("graph.json"), selected.to_string()).unwrap();
    let source = root.to_string_lossy().replace('\\', "/");
    fs::write(
        directory.path().join("CMakeLists.txt"),
        format!(
            r#"cmake_minimum_required(VERSION 3.28)
project(no-sdk NONE)
enable_testing()
set(CMAKE_DISABLE_FIND_PACKAGE_Freetype TRUE)
set(CMAKE_DISABLE_FIND_PACKAGE_Fontconfig TRUE)
add_library(OBS::text-freetype2 INTERFACE IMPORTED)
include("{source}/test/cmocka/tests.d/test_text_aa_file.cmake")
include("{source}/test/cmocka/native-test-graph.cmake")
obs_native_test_graph("${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json" "{source}")
get_property(tests DIRECTORY PROPERTY TESTS)
if(tests)
  message(FATAL_ERROR "Unavailable SDK selected native tests")
endif()
"#
        ),
    )
    .unwrap();
    for os in [
        "OS_LINUX",
        "OS_FREEBSD",
        "OS_OPENBSD",
        "OS_WINDOWS",
        "OS_MACOS",
    ] {
        let mut command = std::process::Command::new("cmake");
        command
            .arg("-S")
            .arg(directory.path())
            .arg("-B")
            .arg(directory.path().join(os));
        for fact in [
            "OS_LINUX",
            "OS_FREEBSD",
            "OS_OPENBSD",
            "OS_WINDOWS",
            "OS_MACOS",
        ] {
            command.arg(format!(
                "-D{fact}={}",
                if fact == os { "ON" } else { "OFF" }
            ));
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{command:?}\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
