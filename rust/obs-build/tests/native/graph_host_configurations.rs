use super::*;

fn graph_fixture(names: &[&str], setup: &str) -> tempfile::TempDir {
    let fixture = tempfile::tempdir().unwrap();
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = dunce::canonicalize(&repository)
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/");
    let graph: serde_json::Value = serde_json::from_slice(
        &fs::read(repository.join("test/cmocka/build/test-graph.json")).unwrap(),
    )
    .unwrap();
    let mut selected = serde_json::json!({"schema": 1, "targets": {}, "tests": {}});
    for name in names {
        for section in ["targets", "tests"] {
            selected[section][name] = graph[section][name].clone();
        }
    }
    fs::write(fixture.path().join("graph.json"), selected.to_string()).unwrap();
    fs::write(fixture.path().join("CMakeLists.txt"), format!(r#"
cmake_minimum_required(VERSION 3.28)
# This fixture generates build rules only; compiler probing is unrelated to the
# host facts and dependency paths under test (the compilation contract probes it).
set(CMAKE_C_COMPILER_ID_RUN TRUE)
set(CMAKE_C_COMPILER_ID GNU)
set(CMAKE_C_COMPILER_FORCED TRUE)
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
        .arg("-B")
        .arg(fixture.join("build"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn native_test_graph_lua_watch_inputs_allow_ninja_frontend_configuration() {
    let source = dunce::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
    let fixture = graph_fixture(
        &["test_lua_graphics_lock_bounded"],
        &format!(
            "set(OS_LINUX TRUE)\ninclude(\"{}/test/cmocka/tests.d/test_lua_graphics_lock_bounded.cmake\")",
            source.to_string_lossy().replace('\\', "/")
        ),
    );
    configure(fixture.path());
    let result = std::process::Command::new("ninja")
        .arg("-C")
        .arg(fixture.path().join("build"))
        .args(["-n", "build.ninja"])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn native_test_graph_portable_startup_without_output_directory() {
    for host in ["OS_MACOS", "OS_WINDOWS"] {
        let fixture = graph_fixture(
            &[
                "test_startup_alloc_stable",
                "test_startup_cycle_stress",
                "test_video_io_hidden",
            ],
            &format!("set({host} TRUE)\nunset(OBS_OUTPUT_DIR)\nunset(OBS_OUTPUT_DIR CACHE)"),
        );
        configure(fixture.path());
        // Query CTest rather than depending on CMake's generated quoting style.
        // Show-only mode does not require these configuration fixtures to be built.
        let build = fixture.path().join("build");
        let result = std::process::Command::new("ctest")
            .current_dir(&build)
            .args(["--show-only=json-v1"])
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{host}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let listing: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        let tests = listing["tests"].as_array().unwrap();
        assert_eq!(tests.len(), 3, "{host}: {listing}");
        for name in [
            "test_startup_alloc_stable",
            "test_startup_cycle_stress",
            "test_video_io_hidden",
        ] {
            let test = tests.iter().find(|test| test["name"] == name).unwrap();
            let directory = test["properties"]
                .as_array()
                .unwrap()
                .iter()
                .find(|property| property["name"] == "WORKING_DIRECTORY")
                .and_then(|property| property["value"].as_str())
                .unwrap();
            assert_eq!(
                dunce::canonicalize(directory).unwrap(),
                dunce::canonicalize(&build).unwrap(),
                "{host}/{name}: {test}"
            );
        }
    }
}

#[test]
fn native_test_graph_video_io_hidden_imports_private_header_dependencies() {
    use obs_build::validation::graph::Graph;
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for windows in [false, true] {
        let facts = std::collections::BTreeMap::from([
            ("OS_WINDOWS".into(), windows),
            ("OS_LINUX".into(), false),
        ]);
        // Evaluate only this registration: other graph entries need SDK facts.
        let raw: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(repository.join("test/cmocka/build/test-graph.json")).unwrap(),
        )
        .unwrap();
        let graph = Graph::parse(&serde_json::json!({"schema":1,"targets":{"test_video_io_hidden":raw["targets"]["test_video_io_hidden"]},"tests":{"test_video_io_hidden":raw["tests"]["test_video_io_hidden"]}}).to_string()).unwrap();
        let selected = graph.select(&facts).unwrap();
        let target = &selected.targets["test_video_io_hidden"];
        assert!(
            target
                .header_libraries
                .iter()
                .any(|name| name == "OBS::caption")
        );
        assert!(
            raw["targets"]["test_video_io_hidden"]["libraries"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!({"name":"OBS::w32-pthreads","requires":"OS_WINDOWS"}))
        );
    }
}

#[test]
fn native_test_graph_windows_slideshow_has_one_host_and_filetime_helper() {
    use obs_build::validation::graph::Graph;
    let repository =
        dunce::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
    let raw: serde_json::Value = serde_json::from_slice(
        &fs::read(repository.join("test/cmocka/build/test-graph.json")).unwrap(),
    )
    .unwrap();
    let graph = Graph::parse(
        &serde_json::json!({
            "schema": 1,
            "targets": {"test_slideshow_empty": raw["targets"]["test_slideshow_empty"]},
            "tests": {"test_slideshow_empty": raw["tests"]["test_slideshow_empty"]}
        })
        .to_string(),
    )
    .unwrap();
    let selected = graph
        .select(&std::collections::BTreeMap::from([
            ("OS_WINDOWS".into(), true),
            ("OS_MACOS".into(), false),
            ("OS_LINUX".into(), false),
            ("HAVE_SLIDESHOW_TEST_OWNER".into(), true),
        ]))
        .unwrap();
    assert!(selected.targets.contains_key("test_slideshow_empty"));
    let source = repository.to_string_lossy().replace('\\', "/");
    let fixture = graph_fixture(
        &["test_slideshow_empty"],
        &format!(
            r#"
set(OS_WINDOWS TRUE)
set(CMAKE_SYSTEM_NAME Windows)
set(HAVE_SLIDESHOW_TEST_OWNER TRUE)
set(CMAKE_SOURCE_DIR "{source}")
include("{source}/test/cmocka/tests.d/test_image_source_host.cmake")
function(check_slideshow_sources)
  file(GENERATE OUTPUT "${{CMAKE_BINARY_DIR}}/sources.txt"
    CONTENT "$<JOIN:$<TARGET_PROPERTY:test_slideshow_empty,SOURCES>,\n>")
endfunction()
cmake_language(DEFER CALL check_slideshow_sources)
"#
        ),
    );
    configure(fixture.path());
    let sources = fs::read_to_string(fixture.path().join("build/sources.txt")).unwrap();
    let mut compiled = std::collections::BTreeSet::new();
    for path in sources.lines() {
        let path = Path::new(path);
        let text = fs::read_to_string(path).unwrap();
        let mut members = vec![path.to_path_buf()];
        if path.to_string_lossy().ends_with("+.h.c") {
            members.extend(text.lines().filter_map(|line| {
                let member = line.strip_prefix("#include \"")?.strip_suffix('"')?;
                member.ends_with(".c.h").then(|| repository.join(member))
            }));
        }
        for member in members {
            assert!(
                compiled.insert(member.clone()),
                "duplicate compiled source: {}",
                member.display()
            );
        }
    }
    assert!(
        compiled.contains(&repository.join("plugins/image-source/image-file-version.c.h")),
        "Windows slideshow omits image_file_version_from_filetime helper: {sources}"
    );
    assert!(
        compiled.contains(&repository.join("plugins/image-source/image-source-host-windows.c.h"))
    );
    assert!(
        compiled.contains(&repository.join("plugins/image-source/image-file-version-win32.c.h"))
    );
}
