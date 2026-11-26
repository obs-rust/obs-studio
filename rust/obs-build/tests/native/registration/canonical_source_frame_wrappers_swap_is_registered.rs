use super::*;

#[test]
fn canonical_source_frame_wrappers_swap_is_registered() {
    use obs_build::validation::graph::{Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let name = "test_source_frame_wrappers";
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    let selected = graph
        .select(&[("OS_LINUX".into(), true), ("OS_WINDOWS".into(), false)].into())
        .unwrap();
    // The unchanged test compiles no copy of the C: it calls the six
    // exported entry points of the real libobs (Rust in hybrid, the
    // original C in the reference).
    assert!(
        matches!(selected.targets[name].sources.as_slice(), [Source::Path(path)]
        if path == "test/cmocka/test_source_frame_wrappers.c")
    );
    assert_eq!(selected.tests[name].target, name);
    let ported =
        fs::read_to_string(root.join("libobs/build/ported-source-sets/source-frame-wrappers.json"))
            .unwrap();
    assert!(ported.contains("build/libobs.ported-source-frame-wrappers+.h.c"));
    let group =
        fs::read_to_string(root.join("libobs/build/libobs.ported-source-frame-wrappers+.h.c"))
            .unwrap();
    assert!(group.contains("#include \"libobs/obs-source-frame-wrappers.c.h\""));
    let hybrid = fs::read_to_string(root.join("libobs/build/libobs.core-source+.h.c")).unwrap();
    assert!(!hybrid.contains("#include \"libobs/obs-source-frame-wrappers.c.h\""));
    assert!(hybrid.contains("#include \"libobs/obs-source-frame-wrappers-fields.c.h\""));
    let exports = fs::read_to_string(
        root.join("libobs/cmake/rust-exports.d/obs-source-source_frame_wrappers.txt"),
    )
    .unwrap();
    assert_eq!(
        exports.lines().collect::<Vec<_>>(),
        [
            "obs_source_output_video",
            "obs_source_output_video2",
            "obs_source_preload_video",
            "obs_source_preload_video2",
            "obs_source_set_video_frame",
            "obs_source_set_video_frame2"
        ]
    );
}
