use super::*;

#[test]
fn canonical_gs_perspective_is_registered() {
    use obs_build::validation::graph::{Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph
        .targets
        .retain(|name, _| name == "test_gs_perspective");
    graph.tests.retain(|name, _| name == "test_gs_perspective");
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph.select(&[(os.into(), true)].into()).unwrap();
        let target = &selected.targets["test_gs_perspective"];
        assert!(matches!(target.sources.as_slice(), [Source::Path(path)]
            if path == "test/cmocka/test_gs_perspective.c"));
        assert_eq!(
            selected.tests["test_gs_perspective"].target,
            "test_gs_perspective"
        );
    }
    let extracted = root.join("libobs/graphics/graphics-perspective.c.h");
    assert!(
        extracted.is_file(),
        "gs_perspective must have its own C unit"
    );
    let original = fs::read_to_string(root.join("libobs/graphics/graphics.c.h")).unwrap();
    assert!(!original.contains("void gs_perspective("));
    let group =
        fs::read_to_string(root.join("libobs/build/libobs.graphics-perspective+.h.c")).unwrap();
    assert!(group.contains("#include \"libobs/graphics/graphics-perspective.c.h\""));
}
