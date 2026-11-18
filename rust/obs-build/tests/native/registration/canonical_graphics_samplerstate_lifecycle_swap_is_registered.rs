use super::*;

#[test]
fn canonical_graphics_samplerstate_lifecycle_swap_is_registered() {
    use obs_build::validation::graph::{Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let name = "test_samplerstate_lifecycle";
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph.select(&[(os.into(), true)].into()).unwrap();
        assert!(
            matches!(selected.targets[name].sources.as_slice(), [Source::Path(path)]
            if path == "test/cmocka/test_samplerstate_lifecycle.c")
        );
        assert_eq!(selected.tests[name].target, name);
    }
    let ported = fs::read_to_string(
        root.join("libobs/build/ported-source-sets/graphics-samplerstate-lifecycle.json"),
    )
    .unwrap();
    assert!(ported.contains("build/libobs.ported-graphics-samplerstate-lifecycle+.h.c"));
    let group = fs::read_to_string(
        root.join("libobs/build/libobs.ported-graphics-samplerstate-lifecycle+.h.c"),
    )
    .unwrap();
    assert!(group.contains("#include \"libobs/graphics/graphics-samplerstate-lifecycle.c.h\""));
    let hybrid = fs::read_to_string(root.join("libobs/build/libobs.graphics+.h.c")).unwrap();
    assert!(!hybrid.contains("#include \"libobs/graphics/graphics-samplerstate-lifecycle.c.h\""));
    assert!(
        hybrid.contains("#include \"libobs/graphics/graphics-samplerstate-lifecycle-fields.c.h\"")
    );
}
