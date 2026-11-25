use super::*;

#[test]
fn canonical_source_enum_filters_swap_is_registered() {
    use obs_build::validation::graph::{Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let name = "test_source_enum_filters";
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph.select(&[(os.into(), true)].into()).unwrap();
        // The unchanged test compiles no copy of the walk: it calls the
        // exported symbol of the real libobs (Rust in hybrid, the original C
        // in the reference).
        assert!(
            matches!(selected.targets[name].sources.as_slice(), [Source::Path(path)]
            if path == "test/cmocka/test_source_enum_filters.c")
        );
        assert!(selected.targets[name].definitions.is_empty());
        assert_eq!(selected.tests[name].target, name);
    }
    let ported =
        fs::read_to_string(root.join("libobs/build/ported-source-sets/source-enum-filters.json"))
            .unwrap();
    assert!(ported.contains("build/libobs.ported-source-enum-filters+.h.c"));
    let group =
        fs::read_to_string(root.join("libobs/build/libobs.ported-source-enum-filters+.h.c"))
            .unwrap();
    assert!(group.contains("#include \"libobs/obs-source-enum-filters.c.h\""));
    let hybrid = fs::read_to_string(root.join("libobs/build/libobs.core-source+.h.c")).unwrap();
    assert!(!hybrid.contains("#include \"libobs/obs-source-enum-filters.c.h\""));
    assert!(hybrid.contains("#include \"libobs/obs-source-enum-filters-fields.c.h\""));
    let exports = fs::read_to_string(
        root.join("libobs/cmake/rust-exports.d/obs-source-source_enum_filters.txt"),
    )
    .unwrap();
    assert_eq!(
        exports.lines().collect::<Vec<_>>(),
        ["obs_source_enum_filters"]
    );
}
