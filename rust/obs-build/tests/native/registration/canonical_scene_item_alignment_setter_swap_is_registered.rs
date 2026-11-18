use super::*;

#[test]
fn canonical_scene_item_alignment_setter_swap_is_registered() {
    use obs_build::validation::graph::{Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let name = "test_sceneitem_alignment_setter";
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph.select(&[(os.into(), true)].into()).unwrap();
        assert!(
            matches!(selected.targets[name].sources.as_slice(), [Source::Path(path)]
            if path == "test/cmocka/test_sceneitem_alignment_setter.c")
        );
        assert_eq!(selected.tests[name].target, name);
    }
    let ported = fs::read_to_string(
        root.join("libobs/build/ported-source-sets/scene-item-alignment-setter.json"),
    )
    .unwrap();
    assert!(ported.contains("build/libobs.ported-scene-item-alignment-setter+.h.c"));
    let group = fs::read_to_string(
        root.join("libobs/build/libobs.ported-scene-item-alignment-setter+.h.c"),
    )
    .unwrap();
    assert!(group.contains("#include \"libobs/obs-scene-item-alignment-setter.c.h\""));
    let hybrid = fs::read_to_string(root.join("libobs/build/libobs.core-source+.h.c")).unwrap();
    assert!(!hybrid.contains("#include \"libobs/obs-scene-item-alignment-setter.c.h\""));
    // Both real C adapters remain in the production group, shared with earlier setters.
    assert!(hybrid.contains("#include \"libobs/obs-scene-item-bounds-setters-fields.c.h\""));
    assert!(hybrid.contains("#include \"libobs/obs-scene.c.h\""));
    let exports = fs::read_to_string(
        root.join("libobs/cmake/rust-exports.d/obs-scene-item_alignment_setter.txt"),
    )
    .unwrap();
    assert_eq!(exports.trim(), "obs_sceneitem_set_alignment");
}
