use super::*;
use obs_build::validation::graph::Dependency;
use std::process::Command;
fn checked(command: &mut Command) -> std::process::Output {
    let result = command.output().unwrap();
    assert!(
        result.status.success(),
        "{command:?}\n{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    result
}
fn cases(output: &std::process::Output, name: &str) {
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
    .replace("\r\n", "\n");
    let count = match name {
        "test_ws_current_scene_removed"
        | "test_ws_filter_settings_event"
        | "test_ws_transform_bounds" => 4,
        "test_ws_remove_input" => 2,
        "test_ws_list_property_items" => 3,
        _ => 5,
    };
    assert!(
        text.contains(&format!("[  PASSED  ] {count} test(s).")),
        "{name}: {text}"
    );
    assert!(!text.contains("[  SKIPPED ]"), "{name}: {text}");
    assert!(!text.contains("[  FAILED  ]"), "{name}: {text}");
    eprintln!("{name}: PASS {count} cases");
}
fn unavailable_owners(directory: &Path, root: &str) {
    let absent = directory.join("absent-sources");
    fs::create_dir_all(&absent).unwrap();
    for (mode, source_root, qt, mut expected_names) in [
        ("no-source", absent.as_path(), "ON", vec![]),
        (
            "no-qt",
            Path::new(root),
            "OFF",
            vec![
                "test_ws_current_scene_removed",
                "test_ws_image_mime",
                "test_ws_list_property_items",
                "test_ws_scene_count",
                "test_ws_filter_settings_event",
                "test_ws_remove_input",
                "test_ws_transform_bounds",
            ],
        ),
    ] {
        let output = directory.join(mode);
        checked(
            Command::new("cmake")
                .arg("-S")
                .arg(directory)
                .arg("-B")
                .arg(&output)
                .args(["-G", "Ninja"])
                .arg(format!("-DWITH_QT={qt}"))
                .arg(format!(
                    "-DGRAPH_ROOT={}",
                    source_root.to_string_lossy().replace('\\', "/")
                )),
        );
        let inventory = checked(
            Command::new("ctest")
                .arg("--test-dir")
                .arg(&output)
                .arg("--show-only=json-v1"),
        );
        let inventory: serde_json::Value = serde_json::from_slice(&inventory.stdout).unwrap();
        let names: Vec<_> = inventory["tests"]
            .as_array()
            .unwrap()
            .iter()
            .map(|test| test["name"].as_str().unwrap())
            .collect();
        expected_names.sort_unstable();
        assert_eq!(names, expected_names);
    }
}

#[test]
fn canonical_websocket_helpers_run_unchanged() {
    let root = dunce::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let build = NativeBuild {
        host: obs_build::BUILD_HOST.into(),
        target: obs_build::BUILD_TARGET.into(),
        output: directory.path().join("native"),
        profile: Default::default(),
    };
    let core = built_core(&build, &root);
    let interfaces = helper_interfaces(&build, &core);
    let mut facts = std::collections::BTreeMap::from([
        ("OS_LINUX".into(), true),
        ("OS_WINDOWS".into(), false),
        ("HAVE_WS_BATCH_ORDER_TEST_OWNER".into(), true),
        ("HAVE_WS_CURRENT_SCENE_REMOVED_TEST_OWNER".into(), true),
        ("HAVE_WS_LIST_PROPERTY_ITEMS_TEST_OWNER".into(), true),
        ("HAVE_WS_SCENE_COUNT_TEST_OWNER".into(), true),
        ("HAVE_WS_IMAGE_MIME_TEST_OWNER".into(), true),
        ("HAVE_WS_SESSION_DURATION_TEST_OWNER".into(), true),
    ]);
    for name in CASES {
        facts.insert(
            format!(
                "HAVE_{}_TEST_OWNER",
                name.trim_start_matches("test_").to_uppercase()
            ),
            true,
        );
    }
    let raw: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap())
            .unwrap();
    let mut selected = expected();
    for section in ["targets", "tests"] {
        for name in CASES {
            selected[section][name] = raw[section][name].clone();
        }
    }
    let graph = Graph::parse(&selected.to_string()).unwrap();
    let artifacts = graph.compile(&build, &root, &facts, &interfaces).unwrap();
    assert_eq!(artifacts.len(), CASES.len());
    for name in CASES {
        let loaded = checked(Command::new("ldd").arg(&artifacts[name]));
        let loaded = String::from_utf8_lossy(&loaded.stdout);
        if matches!(
            name,
            "test_ws_batch_order"
                | "test_ws_current_scene_removed"
                | "test_ws_list_property_items"
                | "test_ws_filter_settings_event"
                | "test_ws_remove_input"
                | "test_ws_transform_bounds"
        ) {
            assert!(
                loaded.contains(core.dependency.artifacts[0].to_str().unwrap()),
                "{name}: {loaded}"
            );
        } else {
            assert!(!loaded.contains("libobs.so"), "{name}: {loaded}");
        }
        let test = graph.tests[name].resolve_runtime(&interfaces).unwrap();
        for _ in 0..repetitions(name) {
            cases(
                &checked(
                    &mut test
                        .command(&root, &build.output, &artifacts[name])
                        .unwrap(),
                ),
                name,
            );
        }
    }
    cmake_replay(directory.path(), &root, &core);
}

fn cmake_replay(directory: &Path, root: &Path, core: &BuiltCore) {
    fs::write(directory.join("graph.json"), expected().to_string()).unwrap();
    let root = root.to_string_lossy().replace('\\', "/");
    fs::write(
        directory.join("CMakeLists.txt"),
        format!(
            r#"cmake_minimum_required(VERSION 3.28)
project(websocket-helper-contract C CXX)
enable_testing()
set(OS_LINUX TRUE)
set(OS_WINDOWS FALSE)
{core_cmake}
find_package(X11 REQUIRED)
find_package(nlohmann_json 3.11 REQUIRED)
find_package(PkgConfig REQUIRED)
pkg_check_modules(CMOCKA REQUIRED IMPORTED_TARGET cmocka)
add_library(OBS::native-test-cmocka ALIAS PkgConfig::CMOCKA)
option(WITH_QT "Optional Qt owner" ON)
if(WITH_QT)
  find_package(Qt6 QUIET COMPONENTS Core)
endif()
set(GRAPH_ROOT "{root}" CACHE PATH "Fixture source root")
include("{root}/test/cmocka/native-test-graph.cmake")
obs_native_test_graph("${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json" "${{GRAPH_ROOT}}")
"#,
            core_cmake = core.cmake,
        ),
    )
    .unwrap();
    let output = directory.join("cmake");
    checked(
        Command::new("cmake")
            .arg("-S")
            .arg(directory)
            .arg("-B")
            .arg(&output)
            .args(["-G", "Ninja"]),
    );
    checked(
        Command::new("cmake")
            .arg("--build")
            .arg(&output)
            .args(["--parallel", "4"]),
    );
    for name in CASES {
        let loaded = checked(Command::new("ldd").arg(output.join(name)));
        let loaded = String::from_utf8_lossy(&loaded.stdout);
        if matches!(
            name,
            "test_ws_batch_order"
                | "test_ws_current_scene_removed"
                | "test_ws_list_property_items"
                | "test_ws_filter_settings_event"
                | "test_ws_remove_input"
                | "test_ws_transform_bounds"
        ) {
            assert!(
                loaded.contains(core.dependency.artifacts[0].to_str().unwrap()),
                "{name}: {loaded}"
            );
        } else {
            assert!(!loaded.contains("libobs.so"), "{name}: {loaded}");
        }
        for _ in 0..repetitions(name) {
            cases(
                &checked(
                    Command::new("ctest")
                        .arg("--test-dir")
                        .arg(&output)
                        .args(["-V", "--no-tests=error", "-R"])
                        .arg(format!("^{name}$")),
                ),
                name,
            );
        }
    }
    unavailable_owners(directory, &root);
}

fn repetitions(name: &str) -> usize {
    match name {
        "test_ws_filter_settings_event" | "test_ws_remove_input" | "test_ws_transform_bounds" => 5,
        _ => 1,
    }
}
fn helper_interfaces(
    build: &NativeBuild,
    core: &BuiltCore,
) -> std::collections::BTreeMap<String, Dependency> {
    let mut interfaces = std::collections::BTreeMap::from([
        ("OBS::libobs".into(), core.dependency.clone()),
        (
            "nlohmann_json::nlohmann_json".into(),
            Dependency::package(&build.target, "nlohmann_json").unwrap(),
        ),
        (
            "OBS::native-test-cmocka".into(),
            Dependency::package(&build.target, "cmocka").unwrap(),
        ),
        (
            "Qt6::Core".into(),
            Dependency::package(&build.target, "Qt6Core").unwrap(),
        ),
    ]);
    interfaces.insert(
        "X11::X11".into(),
        Dependency::package(&build.target, "x11").unwrap(),
    );
    interfaces
}
