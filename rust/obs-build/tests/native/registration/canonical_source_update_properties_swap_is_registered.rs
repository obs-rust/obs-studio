use super::*;

#[test]
fn canonical_source_update_properties_swap_is_registered() {
    use obs_build::validation::graph::{Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let name = "test_source_update_properties";
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph.select(&[(os.into(), true)].into()).unwrap();
        assert!(
            matches!(selected.targets[name].sources.as_slice(), [Source::Path(path)]
            if path == "test/cmocka/test_source_update_properties.c")
        );
        assert_eq!(selected.tests[name].target, name);
    }
    let ported = fs::read_to_string(
        root.join("libobs/build/ported-source-sets/source-update-properties.json"),
    )
    .unwrap();
    assert!(ported.contains("build/libobs.ported-source-update-properties+.h.c"));
    let group =
        fs::read_to_string(root.join("libobs/build/libobs.ported-source-update-properties+.h.c"))
            .unwrap();
    assert!(group.contains("#include \"libobs/obs-source-update-properties.c.h\""));
    let hybrid = fs::read_to_string(root.join("libobs/build/libobs.core-source+.h.c")).unwrap();
    assert!(!hybrid.contains("#include \"libobs/obs-source-update-properties.c.h\""));
    assert!(hybrid.contains("#include \"libobs/obs-source-update-properties-fields.c.h\""));
}
