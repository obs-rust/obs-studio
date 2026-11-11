use super::*;
use obs_build::validation::graph::Graph;

fn expected() -> Graph {
    Graph::parse(include_str!("qt_core_filename_expected.json")).unwrap()
}

#[test]
fn canonical_qt_core_filename_is_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let actual = Graph::load(&root.join("test/cmocka/build/test-graph.d")).unwrap();
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
        for qt in [false, true] {
            for json in [false, true] {
                let mut facts = std::collections::BTreeMap::from([
                    ("HAVE_QT_CORE_TEST_OWNER".into(), qt),
                    ("HAVE_CRASH_LOG_TEST_OWNER".into(), qt && json),
                    ("HAVE_OBS_OPENGL".into(), false),
                ]);
                for os in ["OS_LINUX", "OS_FREEBSD", "OS_WINDOWS", "OS_MACOS"] {
                    facts.insert(os.into(), os == host);
                }
                let selected = expected.select(&facts).unwrap();
                let count = 1
                    + 2 * usize::from(qt)
                    + usize::from(qt && host != "OS_WINDOWS")
                    + usize::from(qt && json && host == "OS_LINUX");
                assert_eq!(selected.targets.len(), count);
                assert_eq!(selected.tests.len(), count);
            }
        }
    }
}

#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
#[path = "support/qt_core_filename_runtime.rs"]
mod runtime;

#[test]
fn qt_core_filename_other_hosts_configure_without_optional_sdks() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (host, unix) in [("OS_WINDOWS", "OFF"), ("OS_MACOS", "ON")] {
        let directory = tempfile::tempdir().unwrap();
        fs::write(
            directory.path().join("graph.json"),
            include_str!("qt_core_filename_expected.json"),
        )
        .unwrap();
        fs::write(
            directory.path().join("CMakeLists.txt"),
            format!(
                r#"cmake_minimum_required(VERSION 3.28)
project(qt-core-host-guards C CXX)
enable_testing()
set(OS_LINUX OFF)
set(OS_FREEBSD OFF)
set(OS_WINDOWS OFF)
set(OS_MACOS OFF)
set({host} ON)
set(UNIX {unix})
set(HAVE_OBS_OPENGL OFF)
foreach(name IN ITEMS OBS::libobs OBS::native-test-cmocka)
  add_library(${{name}} INTERFACE IMPORTED)
endforeach()
include("{root}/test/cmocka/native-test-graph.cmake")
obs_native_test_graph("${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json" "{root}")
"#,
                root = root.to_string_lossy().replace('\\', "/")
            ),
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
        let tests = inventory["tests"].as_array().unwrap();
        assert_eq!(tests.len(), 1);
        assert_eq!(tests[0]["name"], "test_output_filename_reserve");
    }
}
