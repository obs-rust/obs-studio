use super::*;

fn graph_fixture(names: &[&str], setup: &str) -> tempfile::TempDir {
    let fixture = tempfile::tempdir().unwrap();
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = repository.canonicalize().unwrap().to_string_lossy().replace('\\', "/");
    let graph: serde_json::Value = serde_json::from_slice(
        &fs::read(repository.join("test/cmocka/build/test-graph.json")).unwrap(),
    ).unwrap();
    let mut selected = serde_json::json!({"schema": 1, "targets": {}, "tests": {}});
    for name in names {
        for section in ["targets", "tests"] {
            selected[section][name] = graph[section][name].clone();
        }
    }
    fs::write(fixture.path().join("graph.json"), selected.to_string()).unwrap();
    fs::write(fixture.path().join("CMakeLists.txt"), format!(r#"
cmake_minimum_required(VERSION 3.28)
project(host-graph-contract C)
enable_testing()
set(OS_LINUX FALSE)
set(OS_WINDOWS FALSE)
set(OS_MACOS FALSE)
set(ENABLE_FRONTEND ON)
foreach(dependency IN ITEMS OBS::libobs OBS::native-test-cmocka OBS::caption OBS::w32-pthreads Threads::Threads Uthash::Uthash)
  add_library(${{dependency}} INTERFACE IMPORTED)
endforeach()
{setup}
include("{source}/test/cmocka/native-test-graph.cmake")
obs_native_test_graph("${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json" "{source}")
"#)).unwrap();
    fixture
}

fn configure(fixture: &Path) {
    let result = std::process::Command::new("cmake")
        .args(["-G", "Ninja", "-S"])
        .arg(fixture)
        .arg("-B").arg(fixture.join("build"))
        .output().unwrap();
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
}

#[test]
fn native_test_graph_lua_watch_inputs_allow_ninja_frontend_configuration() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap();
    let fixture = graph_fixture(&["test_lua_graphics_lock_bounded"], &format!(
        "set(OS_LINUX TRUE)\ninclude(\"{}/test/cmocka/tests.d/test_lua_graphics_lock_bounded.cmake\")",
        source.to_string_lossy().replace('\\', "/")
    ));
    configure(fixture.path());
    let result = std::process::Command::new("ninja")
        .arg("-C").arg(fixture.path().join("build"))
        .args(["-n", "build.ninja"]).output().unwrap();
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
}

#[test]
fn native_test_graph_portable_startup_without_output_directory() {
    for host in ["OS_MACOS", "OS_WINDOWS"] {
        let fixture = graph_fixture(
            &["test_startup_alloc_stable", "test_startup_cycle_stress", "test_video_io_hidden"],
            &format!("set({host} TRUE)\nunset(OBS_OUTPUT_DIR)\nunset(OBS_OUTPUT_DIR CACHE)"),
        );
        configure(fixture.path());
        let tests = fs::read_to_string(fixture.path().join("build/CTestTestfile.cmake")).unwrap();
        for name in ["test_startup_alloc_stable", "test_startup_cycle_stress", "test_video_io_hidden"] {
            assert!(tests.contains(&format!("add_test([=[{name}]=]")), "{host}/{name}: {tests}");
        }
    }
}

#[test]
fn native_test_graph_video_io_hidden_imports_private_header_dependencies() {
    use obs_build::validation::graph::Graph;
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for windows in [false, true] {
        let facts = std::collections::BTreeMap::from([
            ("OS_WINDOWS".into(), windows), ("OS_LINUX".into(), false),
        ]);
        // Evaluate only this registration: other graph entries need SDK facts.
        let raw: serde_json::Value = serde_json::from_str(&fs::read_to_string(repository.join("test/cmocka/build/test-graph.json")).unwrap()).unwrap();
        let graph = Graph::parse(&serde_json::json!({"schema":1,"targets":{"test_video_io_hidden":raw["targets"]["test_video_io_hidden"]},"tests":{"test_video_io_hidden":raw["tests"]["test_video_io_hidden"]}}).to_string()).unwrap();
        let selected = graph.select(&facts).unwrap();
        let target = &selected.targets["test_video_io_hidden"];
        assert!(target.header_libraries.iter().any(|name| name == "OBS::caption"));
        assert!(raw["targets"]["test_video_io_hidden"]["libraries"].as_array().unwrap().contains(&serde_json::json!({"name":"OBS::w32-pthreads","requires":"OS_WINDOWS"})));
    }
}
