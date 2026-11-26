use super::*;

#[test]
fn canonical_source_get_release_frame_swap_is_registered() {
    use obs_build::validation::graph::{Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let name = "test_source_get_release_frame";
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    let selected = graph
        .select(&[("OS_LINUX".into(), true), ("OS_WINDOWS".into(), false)].into())
        .unwrap();
    // The unchanged test compiles no copy of the C: it calls the exported
    // obs_source_get_frame() and obs_source_release_frame() of the real
    // libobs (Rust in hybrid, the original C in the reference).
    assert!(
        matches!(selected.targets[name].sources.as_slice(), [Source::Path(path)]
        if path == "test/cmocka/test_source_get_release_frame.c")
    );
    assert_eq!(selected.tests[name].target, name);
    let ported = fs::read_to_string(
        root.join("libobs/build/ported-source-sets/source-get-release-frame.json"),
    )
    .unwrap();
    assert!(ported.contains("build/libobs.ported-source-get-release-frame+.h.c"));
    let group =
        fs::read_to_string(root.join("libobs/build/libobs.ported-source-get-release-frame+.h.c"))
            .unwrap();
    assert!(group.contains("#include \"libobs/obs-source-get-release-frame.c.h\""));
    let hybrid = fs::read_to_string(root.join("libobs/build/libobs.core-source+.h.c")).unwrap();
    assert!(!hybrid.contains("#include \"libobs/obs-source-get-release-frame.c.h\""));
    assert!(hybrid.contains("#include \"libobs/obs-source-get-release-frame-fields.c.h\""));
    let support =
        fs::read_to_string(root.join("test/cmocka/build/rust-static-support.json")).unwrap();
    assert!(support.contains("test/cmocka/source-get-release-frame-link-support.c"));
    let exports = fs::read_to_string(
        root.join("libobs/cmake/rust-exports.d/obs-source-source_get_release_frame.txt"),
    )
    .unwrap();
    assert_eq!(
        exports.lines().collect::<Vec<_>>(),
        ["obs_source_get_frame", "obs_source_release_frame"]
    );
}
