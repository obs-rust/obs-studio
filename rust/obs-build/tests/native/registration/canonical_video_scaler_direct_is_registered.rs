use super::*;

#[test]
fn canonical_video_scaler_direct_is_registered() {
    use obs_build::validation::graph::Graph;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let raw: serde_json::Value = serde_json::from_slice(
        &fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap(),
    ).unwrap();
    let name = "test_video_scaler_direct";
    assert_eq!(raw["targets"][name], serde_json::json!({
        "sources": ["test/cmocka/test_video_scaler_direct.c"],
        "libraries": ["OBS::libobs", "OBS::native-test-cmocka"],
        "requires": "OS_LINUX"
    }));
    assert_eq!(raw["tests"][name], serde_json::json!({
        "target": name, "requires": "OS_LINUX", "timeout_seconds": 120
    }));
    let fragment = fs::read_to_string(root.join("test/cmocka/tests.d/test_video_scaler_direct.cmake")).unwrap();
    assert!(!fragment.contains("add_executable("));
    assert!(!fragment.contains("add_test("));
    let mut graph = Graph::parse(&raw.to_string()).unwrap();
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS", "OS_FREEBSD", "OS_OPENBSD"] {
        let facts = [("OS_LINUX".into(), os == "OS_LINUX")].into();
        let selected = graph.select(&facts).unwrap();
        assert_eq!(selected.targets.contains_key(name), os == "OS_LINUX");
        assert_eq!(selected.tests.contains_key(name), os == "OS_LINUX");
    }
}
