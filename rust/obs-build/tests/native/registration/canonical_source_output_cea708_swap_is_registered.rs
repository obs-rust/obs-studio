use super::*;

#[test]
fn canonical_source_output_cea708_swap_is_registered() {
    use obs_build::validation::graph::{Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let name = "test_source_output_cea708";
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    let selected = graph
        .select(&[("OS_LINUX".into(), true), ("OS_WINDOWS".into(), false)].into())
        .unwrap();
    // The unchanged test compiles no copy of the C: it calls the exported
    // obs_source_output_cea708() of the real libobs (Rust in hybrid, the
    // original C in the reference).
    assert!(
        matches!(selected.targets[name].sources.as_slice(), [Source::Path(path)]
        if path == "test/cmocka/test_source_output_cea708.c")
    );
    assert_eq!(selected.tests[name].target, name);
    let ported =
        fs::read_to_string(root.join("libobs/build/ported-source-sets/source-output-cea708.json"))
            .unwrap();
    assert!(ported.contains("build/libobs.ported-source-output-cea708+.h.c"));
    let group =
        fs::read_to_string(root.join("libobs/build/libobs.ported-source-output-cea708+.h.c"))
            .unwrap();
    assert!(group.contains("#include \"libobs/obs-source-output-cea708.c.h\""));
    let hybrid = fs::read_to_string(root.join("libobs/build/libobs.core-source+.h.c")).unwrap();
    assert!(!hybrid.contains("#include \"libobs/obs-source-output-cea708.c.h\""));
    assert!(hybrid.contains("#include \"libobs/obs-source-caption-callback-fields.c.h\""));
    let exports = fs::read_to_string(
        root.join("libobs/cmake/rust-exports.d/obs-source-source_output_cea708.txt"),
    )
    .unwrap();
    assert_eq!(
        exports.lines().collect::<Vec<_>>(),
        ["obs_source_output_cea708"]
    );
}
