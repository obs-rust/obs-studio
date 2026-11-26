use super::*;

#[test]
fn canonical_scene_item_set_order_swap_is_registered() {
    use obs_build::validation::graph::{Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let name = "test_scene_canvas_characterization";
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph.select(&[(os.into(), true)].into()).unwrap();
        // The unchanged test compiles no copy of the move: it calls the
        // exported symbol of the real libobs (Rust in hybrid, the original C
        // in the reference).
        assert!(
            matches!(selected.targets[name].sources.as_slice(), [Source::Path(path)]
            if path == "test/cmocka/test_scene_canvas_characterization.c")
        );
        assert_eq!(selected.tests[name].target, name);
    }
    let ported =
        fs::read_to_string(root.join("libobs/build/ported-source-sets/scene-item-set-order.json"))
            .unwrap();
    assert!(ported.contains("build/libobs.ported-scene-item-set-order+.h.c"));
    let group =
        fs::read_to_string(root.join("libobs/build/libobs.ported-scene-item-set-order+.h.c"))
            .unwrap();
    // The reference reaches obs-scene.c.h's static helpers through the live
    // adapters, declared before the original that calls them.
    let support = group
        .find("#include \"libobs/obs-scene-item-set-order-support.c.h\"")
        .unwrap();
    let original = group
        .find("#include \"libobs/obs-scene-item-set-order.c.h\"")
        .unwrap();
    assert!(support < original);
    let hybrid = fs::read_to_string(root.join("libobs/build/libobs.core-source+.h.c")).unwrap();
    assert!(!hybrid.contains("#include \"libobs/obs-scene-item-set-order.c.h\""));
    assert!(!hybrid.contains("#include \"libobs/obs-scene-item-set-order-support.c.h\""));
    // The C-owned seams stay in production groups: the helpers and the full
    // lock in obs-scene.c.h, the set-order and parent adapters beside them,
    // and the link adapter of the scene lookup.
    assert!(hybrid.contains("#include \"libobs/obs-scene.c.h\""));
    assert!(hybrid.contains("#include \"libobs/obs-scene-item-set-order-fields.c.h\""));
    assert!(hybrid.contains("#include \"libobs/obs-scene-item-parent-fields.c.h\""));
    let lookup =
        fs::read_to_string(root.join("libobs/build/libobs.core-scene-lookup+.h.c")).unwrap();
    assert!(lookup.contains("#include \"libobs/obs-scene-lookup-fields.c.h\""));
    let exports =
        fs::read_to_string(root.join("libobs/cmake/rust-exports.d/obs-scene-item_set_order.txt"))
            .unwrap();
    assert_eq!(
        exports.lines().collect::<Vec<_>>(),
        ["obs_sceneitem_set_order"]
    );
}
