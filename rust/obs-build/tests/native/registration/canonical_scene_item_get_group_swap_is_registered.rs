use super::*;

#[test]
fn canonical_scene_item_get_group_swap_is_registered() {
    use obs_build::validation::graph::{Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let name = "test_sceneitem_get_group";
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph.select(&[(os.into(), true)].into()).unwrap();
        assert!(
            matches!(selected.targets[name].sources.as_slice(), [Source::Path(path)]
            if path == "test/cmocka/test_sceneitem_get_group.c")
        );
        assert_eq!(selected.tests[name].target, name);
    }
    let ported =
        fs::read_to_string(root.join("libobs/build/ported-source-sets/scene-item-get-group.json"))
            .unwrap();
    assert!(ported.contains("build/libobs.ported-scene-item-get-group+.h.c"));
    let group =
        fs::read_to_string(root.join("libobs/build/libobs.ported-scene-item-get-group+.h.c"))
            .unwrap();
    assert!(group.contains("#include \"libobs/obs-scene-item-get-group.c.h\""));
    let hybrid = fs::read_to_string(root.join("libobs/build/libobs.core-source+.h.c")).unwrap();
    assert!(!hybrid.contains("#include \"libobs/obs-scene-item-get-group.c.h\""));
    // The C-owned seams stay in the production group: the full lock and the
    // shared parent-group walk in obs-scene.c.h, the group-flag adapter beside it.
    assert!(hybrid.contains("#include \"libobs/obs-scene.c.h\""));
    assert!(hybrid.contains("#include \"libobs/obs-scene-item-group-scene-fields.c.h\""));
    let exports =
        fs::read_to_string(root.join("libobs/cmake/rust-exports.d/obs-scene-item_get_group.txt"))
            .unwrap();
    assert_eq!(exports.trim(), "obs_sceneitem_get_group");
}
