use super::*;
use obs_build::validation::graph::Graph;

#[test]
fn canonical_v4l2_freetype_sdk_absent_configures() {
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
    fs::write(directory.path().join("graph.json"), selected.to_string()).unwrap();
    let source = root.to_string_lossy().replace('\\', "/");
    fs::write(
        directory.path().join("CMakeLists.txt"),
        format!(
            r#"cmake_minimum_required(VERSION 3.28)
project(no-sdk NONE)
enable_testing()
set(CMAKE_DISABLE_FIND_PACKAGE_PkgConfig TRUE)
set(CMAKE_DISABLE_FIND_PACKAGE_Libudev TRUE)
set(PKG_CONFIG_FOUND FALSE)
add_library(OBS::v4l2 INTERFACE IMPORTED)
set(HAVE_OBS_V4L2 TRUE)
include("{source}/test/cmocka/tests.d/test_v4l2_busy.cmake")
include("{source}/test/cmocka/tests.d/test_v4l2_udev_poll_error.cmake")
include("{source}/test/cmocka/tests.d/test_text_freetype2_platform.cmake")
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

#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
#[path = "v4l2_freetype_sdk_runtime.rs"]
mod runtime;

const CASES: [&str; 3] = [
    "test_v4l2_busy",
    "test_v4l2_udev_poll_error",
    "test_text_freetype2_platform",
];

fn requirement(name: &str) -> serde_json::Value {
    if name == CASES[2] {
        serde_json::json!({"all": [{"any": ["OS_LINUX", "OS_FREEBSD", "OS_OPENBSD"]}, "HAVE_FREETYPE_PLATFORM_SDK"]})
    } else {
        serde_json::json!({"all": ["OS_LINUX", "HAVE_OBS_V4L2", if name == CASES[0] { "HAVE_V4L2_BUSY_SDK" } else { "HAVE_V4L2_UDEV_SDK" }]})
    }
}

fn expected_target(name: &str) -> serde_json::Value {
    let text = name == CASES[2];
    let plugin = if text { "text-freetype2" } else { "linux-v4l2" };
    let mut target = serde_json::json!({
        "sources": [format!("test/cmocka/{name}.c")],
        "includes": [".", format!("plugins/{plugin}")],
        "watch_inputs": [format!("plugins/{plugin}/build/{plugin}-nix.pch.h")],
        "libraries": ["OBS::libobs", "OBS::native-test-cmocka"],
        "header_packages": if text { vec!["freetype2", "fontconfig"] } else if name == CASES[0] { vec!["libv4l2", "libavcodec", "libavformat", "libavutil"] } else { vec![] },
        "requires": requirement(name)
    });
    if name == CASES[1] {
        target.as_object_mut().unwrap().remove("header_packages");
        target["watch_inputs"] =
            serde_json::json!(["plugins/linux-v4l2/build/linux-v4l2-nix-udev.pch.h"]);
        target["libraries"] =
            serde_json::json!(["OBS::libobs", "Libudev::Libudev", "OBS::native-test-cmocka"]);
        target["watch_inputs"]
            .as_array_mut()
            .unwrap()
            .push("plugins/linux-v4l2/v4l2-udev.c.h".into());
    } else {
        let source = if text {
            "text-freetype2-host-posix"
        } else {
            "v4l2-busy"
        };
        target["sources"]
            .as_array_mut()
            .unwrap()
            .push(format!("plugins/{plugin}/{source}.c.h").into());
    }
    target
}

#[test]
fn canonical_v4l2_freetype_sdk_is_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let raw: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap())
            .unwrap();
    for name in CASES {
        assert_eq!(raw["targets"][name], expected_target(name), "{name}");
        let mut test = serde_json::json!({"target": name, "requires": requirement(name)});
        if name != CASES[2] {
            test["timeout_seconds"] = 60.into();
        }
        assert_eq!(raw["tests"][name], test, "{name}");
        let fragment =
            fs::read_to_string(root.join(format!("test/cmocka/tests.d/{name}.cmake"))).unwrap();
        assert!(!fragment.contains("add_executable("));
        assert!(!fragment.contains("add_test("));
    }
}

#[test]
fn canonical_v4l2_freetype_sdk_guards() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph
        .targets
        .retain(|name, _| CASES.contains(&name.as_str()));
    graph.tests.retain(|name, _| CASES.contains(&name.as_str()));
    assert_eq!(graph.targets.len(), CASES.len());
    for os in [
        "OS_LINUX",
        "OS_FREEBSD",
        "OS_OPENBSD",
        "OS_WINDOWS",
        "OS_MACOS",
    ] {
        for owner in [false, true] {
            for sdk in [false, true] {
                let mut facts: std::collections::BTreeMap<String, bool> = [
                    "OS_LINUX",
                    "OS_FREEBSD",
                    "OS_OPENBSD",
                    "OS_WINDOWS",
                    "OS_MACOS",
                ]
                .map(|name| (name.into(), name == os))
                .into();
                facts.extend([
                    ("HAVE_OBS_V4L2".into(), owner),
                    ("HAVE_V4L2_BUSY_SDK".into(), sdk),
                    ("HAVE_V4L2_UDEV_SDK".into(), sdk),
                    ("HAVE_FREETYPE_PLATFORM_SDK".into(), sdk),
                ]);
                let selected = graph.select(&facts).unwrap();
                let text = sdk && ["OS_LINUX", "OS_FREEBSD", "OS_OPENBSD"].contains(&os);
                let count = usize::from(text)
                    + if sdk && owner && os == "OS_LINUX" {
                        2
                    } else {
                        0
                    };
                assert_eq!(selected.targets.len(), count);
                assert_eq!(selected.tests.len(), count);
            }
        }
    }
}
