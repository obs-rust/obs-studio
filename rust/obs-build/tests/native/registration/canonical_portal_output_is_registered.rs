use super::*;
use obs_build::validation::graph::Graph;

fn expected() -> Graph {
    Graph::parse(include_str!("portal_output_expected.json")).unwrap()
}

#[test]
fn canonical_portal_output_is_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let actual = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let expected = expected();
    for (name, target) in &expected.targets {
        assert_eq!(
            format!("{:?}", actual.targets.get(name).expect(name)),
            format!("{target:?}")
        );
        assert_eq!(
            format!("{:?}", actual.tests.get(name).unwrap()),
            format!("{:?}", expected.tests[name])
        );
    }
    for host in ["OS_LINUX", "OS_FREEBSD", "OS_WINDOWS", "OS_MACOS"] {
        for gio in [false, true] {
            for renderer in [false, true] {
                let mut facts = std::collections::BTreeMap::from([
                    ("UNIX".into(), host != "OS_WINDOWS"),
                    ("HAVE_PORTAL_TEST_GIO".into(), gio),
                    ("HAVE_OBS_OPENGL".into(), renderer),
                ]);
                for os in ["OS_LINUX", "OS_FREEBSD", "OS_WINDOWS", "OS_MACOS"] {
                    facts.insert(os.into(), os == host);
                }
                let selected = expected.select(&facts).unwrap();
                let count = 1
                    + usize::from(host != "OS_WINDOWS")
                    + 3 * usize::from(gio && matches!(host, "OS_LINUX" | "OS_FREEBSD"))
                    + usize::from(renderer && host == "OS_LINUX");
                assert_eq!(selected.targets.len(), count);
                assert_eq!(selected.tests.len(), count);
            }
        }
    }
}

#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
#[path = "portal_output_runtime.rs"]
mod runtime;

#[test]
fn portal_output_other_hosts_configure_without_daemon_sdks() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (host, unix, expected_names) in [
        ("OS_WINDOWS", "OFF", vec!["test_encoder_counter_getters"]),
        (
            "OS_MACOS",
            "ON",
            vec!["test_config_close_fault", "test_encoder_counter_getters"],
        ),
    ] {
        let directory = tempfile::tempdir().unwrap();
        fs::write(
            directory.path().join("graph.json"),
            include_str!("portal_output_expected.json"),
        )
        .unwrap();
        fs::write(
            directory.path().join("CMakeLists.txt"),
            host_cmake(&root, host, unix),
        )
        .unwrap();
        let result = std::process::Command::new("cmake")
            .arg("-S")
            .arg(directory.path())
            .arg("-B")
            .arg(directory.path().join("build"))
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let inventory = std::process::Command::new("ctest")
            .arg("--test-dir")
            .arg(directory.path().join("build"))
            .arg("--show-only=json-v1")
            .output()
            .unwrap();
        assert!(inventory.status.success());
        let inventory: serde_json::Value = serde_json::from_slice(&inventory.stdout).unwrap();
        let mut names: Vec<_> = inventory["tests"]
            .as_array()
            .unwrap()
            .iter()
            .map(|test| test["name"].as_str().unwrap())
            .collect();
        names.sort();
        assert_eq!(names, expected_names);
    }
}

fn host_cmake(root: &Path, host: &str, unix: &str) -> String {
    format!(
        r#"cmake_minimum_required(VERSION 3.28)
project(portal-host-guards C)
enable_testing()
set(OS_LINUX OFF)
set(OS_FREEBSD OFF)
set(OS_WINDOWS OFF)
set(OS_MACOS OFF)
set({host} ON)
set(UNIX {unix})
set(HAVE_OBS_OPENGL OFF)
foreach(name IN ITEMS OBS::libobs OBS::caption Uthash::Uthash OBS::w32-pthreads OBS::native-test-cmocka)
  add_library(${{name}} INTERFACE IMPORTED)
endforeach()
include("{root}/test/cmocka/native-test-graph.cmake")
obs_native_test_graph("${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json" "{root}")
"#,
        root = root.to_string_lossy().replace('\\', "/")
    )
}

#[test]
fn portal_output_cmake_normalizes_windows_paths() {
    let text = host_cmake(Path::new(r"D:\a\obs-studio\test"), "OS_WINDOWS", "OFF");
    assert!(
        text.contains(r#"include("D:/a/obs-studio/test/test/cmocka/native-test-graph.cmake")"#),
        "{text}"
    );
    assert!(
        !text.contains('\\'),
        "CMake quoted paths must not contain Windows escapes: {text}"
    );
}
