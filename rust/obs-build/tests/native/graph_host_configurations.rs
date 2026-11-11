use super::*;

#[test]
fn qt_replay_requirements_know_the_real_host() {
    use obs_build::validation::graph::Graph;
    let graph = Graph::parse(
        &serde_json::json!({
            "schema": 1,
            "targets": {
                "windows": {"sources": ["probe.c"], "requires": "OS_WINDOWS"},
                "other": {"sources": ["probe.c"], "requires": {"not": "OS_WINDOWS"}},
                "linux": {"sources": ["probe.c"], "requires": "OS_LINUX"}
            },
            "tests": {"other": {"target": "other"}}
        })
        .to_string(),
    )
    .unwrap();
    let facts = qt_replay_facts(obs_build::BUILD_TARGET, false);
    let selected = graph.select(&facts).unwrap();
    assert_eq!(selected.targets.contains_key("windows"), cfg!(windows));
    assert_eq!(selected.targets.contains_key("other"), !cfg!(windows));
    assert_eq!(
        selected.targets.contains_key("linux"),
        cfg!(target_os = "linux")
    );
}

fn graph_fixture(names: &[&str], setup: &str) -> tempfile::TempDir {
    let fixture = tempfile::tempdir().unwrap();
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = dunce::canonicalize(&repository)
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/");
    let graph: serde_json::Value = obs_build::validation::graph::Graph::read_value(
        &repository.join("test/cmocka/build/test-graph.d"),
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
        let raw: serde_json::Value = obs_build::validation::graph::Graph::read_value(
            &repository.join("test/cmocka/build/test-graph.d"),
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
    let raw: serde_json::Value = obs_build::validation::graph::Graph::read_value(
        &repository.join("test/cmocka/build/test-graph.d"),
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
        {
            // Both explicit unity groups and generated .c.h wrappers compile members.
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

#[path = "../../registration_modules.rs"]
mod registration_modules;

#[test]
fn native_registration_discovery_is_sorted_and_independent() {
    let fixture = tempfile::tempdir().unwrap();
    fs::write(fixture.path().join("z.rs"), "").unwrap();
    fs::write(fixture.path().join("a.rs"), "").unwrap();
    fs::write(fixture.path().join("ignored.json"), "{}").unwrap();
    fs::create_dir(fixture.path().join("nested")).unwrap();
    let declarations = registration_modules::declarations(fixture.path()).unwrap();
    assert!(declarations.find("mod a;").unwrap() < declarations.find("mod z;").unwrap());
    assert!(!declarations.contains("ignored"));
    assert_eq!(
        declarations,
        registration_modules::declarations(fixture.path()).unwrap()
    );
    fs::write(fixture.path().join("not-a-module.rs"), "").unwrap();
    assert!(registration_modules::declarations(fixture.path()).is_err());
}

#[test]
fn native_graph_fragments_preserve_cross_target_references() {
    use obs_build::validation::graph::Graph;
    let fixture = tempfile::tempdir().unwrap();
    for name in ["z", "a"] {
        fs::write(
            fixture.path().join(format!("{name}.json")),
            serde_json::json!({
                "schema": 1, "targets": {name: {"sources": [format!("{name}.c")]}},
                "tests": {name: {"target": name}}
            })
            .to_string(),
        )
        .unwrap();
    }
    let graph = Graph::load(fixture.path()).unwrap();
    assert_eq!(
        graph.targets.keys().map(String::as_str).collect::<Vec<_>>(),
        ["a", "z"]
    );
    assert_eq!(
        graph.tests.keys().map(String::as_str).collect::<Vec<_>>(),
        ["a", "z"]
    );
}

/// The optional baseline is a migration audit input, never a checked-in second graph.
#[test]
fn canonical_graph_fragments_match_documents_across_hosts() {
    use obs_build::validation::graph::Graph;
    use std::collections::{BTreeMap, BTreeSet};
    fn requirements(value: &serde_json::Value, names: &mut BTreeSet<String>) {
        match value {
            serde_json::Value::String(name) => {
                names.insert(name.clone());
            }
            serde_json::Value::Array(values) => {
                for value in values {
                    requirements(value, names);
                }
            }
            serde_json::Value::Object(values) => {
                for value in values.values() {
                    requirements(value, names);
                }
            }
            _ => {}
        }
    }
    fn facts_in(value: &serde_json::Value, names: &mut BTreeSet<String>) {
        match value {
            serde_json::Value::Object(values) => {
                for (key, value) in values {
                    if key == "requires" || key == "optional_prefixes_requires" {
                        requirements(value, names);
                    } else {
                        facts_in(value, names);
                    }
                }
            }
            serde_json::Value::Array(values) => {
                for value in values {
                    facts_in(value, names);
                }
            }
            _ => {}
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let document = if let Some(path) = std::env::var_os("OBS_GRAPH_BASELINE") {
        fs::read_to_string(path).unwrap()
    } else {
        let result = std::process::Command::new("uv")
            .args(["run", "--no-project", "python", "-c",
                "import json; from pathlib import Path; from ci.native_test_graph import load_graph; print(json.dumps(load_graph(Path('test/cmocka/build/test-graph.d'))))"])
            .current_dir(&root).output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        String::from_utf8(result.stdout).unwrap()
    };
    let raw: serde_json::Value = serde_json::from_str(&document).unwrap();
    let assembled = Graph::read_value(&root.join("test/cmocka/build/test-graph.d")).unwrap();
    assert_eq!(raw, assembled, "fragment migration changed the raw graph");
    let before = Graph::parse(&document).unwrap();
    let after = Graph::load(&root.join("test/cmocka/build/test-graph.d")).unwrap();
    let mut names = BTreeSet::new();
    facts_in(&raw, &mut names);
    for host in [
        "x86_64-unknown-linux-gnu",
        "aarch64-apple-darwin",
        "x86_64-pc-windows-msvc",
        "x86_64-pc-windows-gnu",
        "x86_64-unknown-freebsd",
        "x86_64-unknown-openbsd",
    ] {
        for enabled in [false, true] {
            let mut facts: BTreeMap<String, bool> =
                names.iter().map(|name| (name.clone(), enabled)).collect();
            facts.extend(obs_build::validation::native_test_facts(
                host, enabled, enabled,
            ));
            facts.extend([
                ("APPLE".into(), host.contains("apple")),
                ("WIN32".into(), host.contains("windows")),
            ]);
            let expected = before.select(&facts).unwrap();
            let actual = after.select(&facts).unwrap();
            assert_eq!(
                format!("{actual:?}"),
                format!("{expected:?}"),
                "{host} optional={enabled}"
            );
            println!(
                "graph equality {host} optional={enabled}: {} targets, {} tests",
                actual.targets.len(),
                actual.tests.len()
            );
        }
    }
}
