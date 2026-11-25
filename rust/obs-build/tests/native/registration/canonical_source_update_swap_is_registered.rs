use super::*;

#[test]
fn canonical_source_update_swap_is_registered() {
    use obs_build::validation::graph::{Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let name = "test_source_update";
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    // The existing characterization starts core video: Linux with OpenGL.
    let selected = graph
        .select(&[("OS_LINUX".into(), true), ("HAVE_OBS_OPENGL".into(), true)].into())
        .unwrap();
    assert!(
        matches!(selected.targets[name].sources.as_slice(), [Source::Path(path)]
        if path == "test/cmocka/test_source_update.c")
    );
    assert_eq!(selected.tests[name].target, name);
    let ported =
        fs::read_to_string(root.join("libobs/build/ported-source-sets/source-update.json"))
            .unwrap();
    assert!(ported.contains("build/libobs.ported-source-update+.h.c"));
    let group =
        fs::read_to_string(root.join("libobs/build/libobs.ported-source-update+.h.c")).unwrap();
    assert!(group.contains("#include \"libobs/obs-source-update.c.h\""));
    let hybrid = fs::read_to_string(root.join("libobs/build/libobs.core-source+.h.c")).unwrap();
    assert!(!hybrid.contains("#include \"libobs/obs-source-update.c.h\""));
    assert!(hybrid.contains("#include \"libobs/obs-source-update-fields.c.h\""));
}
