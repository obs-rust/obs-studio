use super::*;

#[test]
fn canonical_source_skip_video_filter_swap_is_registered() {
    use obs_build::validation::graph::{Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let name = "test_source_skip_video_filter";
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    // Linux with OpenGL only: it renders on the graphics thread, as
    // test_filter_skip_removed.
    let selected = graph
        .select(&[("OS_LINUX".into(), true), ("HAVE_OBS_OPENGL".into(), true)].into())
        .unwrap();
    // The test compiles no copy of the C: it calls the exported function of
    // the real libobs (Rust in hybrid, the original C in the reference).
    assert!(
        matches!(selected.targets[name].sources.as_slice(), [Source::Path(path)]
        if path == "test/cmocka/test_source_skip_video_filter.c")
    );
    assert!(selected.targets[name].definitions.is_empty());
    assert_eq!(selected.tests[name].target, name);
    let selected = graph
        .select(&[("OS_LINUX".into(), true), ("HAVE_OBS_OPENGL".into(), false)].into())
        .unwrap();
    assert!(!selected.targets.contains_key(name));
    let ported = fs::read_to_string(
        root.join("libobs/build/ported-source-sets/source-skip-video-filter.json"),
    )
    .unwrap();
    assert!(ported.contains("build/libobs.ported-source-skip-video-filter+.h.c"));
    let group =
        fs::read_to_string(root.join("libobs/build/libobs.ported-source-skip-video-filter+.h.c"))
            .unwrap();
    assert!(group.contains("#include \"libobs/obs-source-skip-video-filter.c.h\""));
    let hybrid = fs::read_to_string(root.join("libobs/build/libobs.core-source+.h.c")).unwrap();
    assert!(!hybrid.contains("#include \"libobs/obs-source-skip-video-filter.c.h\""));
    assert!(hybrid.contains("#include \"libobs/obs-source-skip-video-filter-fields.c.h\""));
    let exports = fs::read_to_string(
        root.join("libobs/cmake/rust-exports.d/obs-source-source_skip_video_filter.txt"),
    )
    .unwrap();
    assert_eq!(
        exports.lines().collect::<Vec<_>>(),
        ["obs_source_skip_video_filter"]
    );
}
