use super::*;
use obs_build::validation::graph::Graph;

#[test]
fn canonical_libobs_go5_is_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut raw: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap())
            .unwrap();
    let name = "test_image_file";
    assert_eq!(raw["targets"][name], serde_json::json!({
        "sources": ["test/cmocka/test_image_file.c"],
        "definitions": ["IMAGE_FILE_DATA_DIR=\"${source}/test/cmocka/data\""],
        "libraries": ["OBS::libobs", "OBS::native-test-cmocka",
            {"name": "OBS::w32-pthreads", "requires": "OS_WINDOWS"}]
    }));
    assert_eq!(raw["tests"][name], serde_json::json!({"target": name, "suite": "core"}));
    let fragment = fs::read_to_string(root.join("test/cmocka/tests.d/test_image_file.cmake")).unwrap();
    assert!(!fragment.contains("add_executable("));
    assert!(!fragment.contains("add_test("));
    for section in ["targets", "tests"] {
        raw[section].as_object_mut().unwrap().retain(|key, _| key == name);
    }
    let graph = Graph::parse(&raw.to_string()).unwrap();
    for os in ["OS_LINUX", "OS_WINDOWS", "OS_MACOS", "OS_FREEBSD", "OS_OPENBSD"] {
        let facts = [(os.into(), true)].into();
        let selected = graph.select(&facts).unwrap();
        assert!(selected.targets.contains_key(name), "{os}");
        assert!(selected.tests.contains_key(name), "{os}");
    }
}
