use super::*;

/// #3897: characterize the real libraries with portable private-header inputs.
#[test]
fn canonical_scene_transform_getters_are_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let graph =
        obs_build::validation::graph::Graph::load(&root.join("test/cmocka/build/test-graph.json"))
            .unwrap();
    let name = "test_scene_item_transform_getters";
    let target = &graph.targets[name];
    assert_eq!(target.sources.len(), 1);
    assert!(
        matches!(&target.sources[0], obs_build::validation::graph::Source::Path(path)
        if path == "test/cmocka/test_scene_item_transform_getters.c")
    );
    let raw: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(root.join("test/cmocka/build/test-graph.json")).unwrap(),
    )
    .unwrap();
    let serialized = &raw["targets"][name];
    assert_eq!(
        serialized["includes"],
        serde_json::json!(["libobs", "deps/libcaption"])
    );
    assert_eq!(
        serialized["libraries"],
        serde_json::json!([
            "OBS::libobs", "OBS::caption", "Uthash::Uthash", "OBS::native-test-cmocka",
            {"name": "OBS::w32-pthreads", "requires": "OS_WINDOWS"}
        ])
    );
    assert_eq!(graph.tests[name].command, ["${target}"]);
    assert_eq!(graph.tests[name].timeout_seconds, Some(30));
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph.select(&[(os.into(), true)].into()).unwrap();
        assert!(selected.targets.contains_key(name));
        assert!(selected.tests.contains_key(name));
    }
}
