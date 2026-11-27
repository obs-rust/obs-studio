use super::*;

#[test]
fn canonical_source_set_video_frame_internal_swap_is_registered() {
    use obs_build::validation::graph::{Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let name = "test_source_set_video_frame_internal";
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph.select(&[(os.into(), true)].into()).unwrap();
        // The test compiles no copy of the worker: it reaches the hidden
        // symbol through the exported obs_source_set_video_frame() of the
        // real libobs (Rust in hybrid, the original C in the reference).
        assert!(
            matches!(selected.targets[name].sources.as_slice(), [Source::Path(path)]
            if path == "test/cmocka/test_source_set_video_frame_internal.c")
        );
        assert!(selected.targets[name].definitions.is_empty());
        assert_eq!(selected.tests[name].target, name);
    }
    let ported = fs::read_to_string(
        root.join("libobs/build/ported-source-sets/source-set-video-frame-internal.json"),
    )
    .unwrap();
    assert!(ported.contains("build/libobs.ported-source-set-video-frame-internal+.h.c"));
    let group = fs::read_to_string(
        root.join("libobs/build/libobs.ported-source-set-video-frame-internal+.h.c"),
    )
    .unwrap();
    assert!(group.contains("#include \"libobs/obs-source-set-video-frame-internal.c.h\""));
    let hybrid = fs::read_to_string(root.join("libobs/build/libobs.core-source+.h.c")).unwrap();
    assert!(!hybrid.contains("#include \"libobs/obs-source-set-video-frame-internal.c.h\""));
    assert!(hybrid.contains("#include \"libobs/obs-source-set-video-frame-internal-fields.c.h\""));
    // The preload frame adapters it shares stay live too.
    assert!(hybrid.contains("#include \"libobs/obs-source-preload-video-internal-fields.c.h\""));
    // obs-source.c.h keeps no copy of the worker or its helpers.
    let source = fs::read_to_string(root.join("libobs/obs-source.c.h")).unwrap();
    assert!(!source.contains("obs_source_set_video_frame_internal("));
    assert!(!source.contains("preload_frame_changed("));
    let hidden = fs::read_to_string(
        root.join("libobs/cmake/rust-hidden.d/obs-source-source_set_video_frame_internal.txt"),
    )
    .unwrap();
    assert!(
        hidden
            .lines()
            .any(|line| line == "obs_source_set_video_frame_internal")
    );
    // Hidden, not exported: obs-internal.h declares it without EXPORT.
    let exports = fs::read_dir(root.join("libobs/cmake/rust-exports.d")).unwrap();
    for entry in exports {
        let text = fs::read_to_string(entry.unwrap().path()).unwrap();
        assert!(
            !text
                .lines()
                .any(|line| line == "obs_source_set_video_frame_internal")
        );
    }
}
