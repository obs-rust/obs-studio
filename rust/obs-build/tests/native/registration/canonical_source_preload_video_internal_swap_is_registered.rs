use super::*;

#[test]
fn canonical_source_preload_video_internal_swap_is_registered() {
    use obs_build::validation::graph::{Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let name = "test_source_preload_video_internal";
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph.select(&[(os.into(), true)].into()).unwrap();
        // The test compiles no copy of the worker: it reaches the hidden
        // symbol through the exported obs_source_preload_video() of the real
        // libobs (Rust in hybrid, the original C in the reference).
        assert!(
            matches!(selected.targets[name].sources.as_slice(), [Source::Path(path)]
            if path == "test/cmocka/test_source_preload_video_internal.c")
        );
        assert!(selected.targets[name].definitions.is_empty());
        assert_eq!(selected.tests[name].target, name);
    }
    let ported = fs::read_to_string(
        root.join("libobs/build/ported-source-sets/source-preload-video-internal.json"),
    )
    .unwrap();
    assert!(ported.contains("build/libobs.ported-source-preload-video-internal+.h.c"));
    let group = fs::read_to_string(
        root.join("libobs/build/libobs.ported-source-preload-video-internal+.h.c"),
    )
    .unwrap();
    assert!(group.contains("#include \"libobs/obs-source-preload-video-internal.c.h\""));
    let hybrid = fs::read_to_string(root.join("libobs/build/libobs.core-source+.h.c")).unwrap();
    assert!(!hybrid.contains("#include \"libobs/obs-source-preload-video-internal.c.h\""));
    assert!(hybrid.contains("#include \"libobs/obs-source-preload-video-internal-fields.c.h\""));
    let hidden = fs::read_to_string(
        root.join("libobs/cmake/rust-hidden.d/obs-source-source_preload_video_internal.txt"),
    )
    .unwrap();
    assert!(
        hidden
            .lines()
            .any(|line| line == "obs_source_preload_video_internal")
    );
    // Hidden, not exported: obs-internal.h declares it without EXPORT.
    let exports = fs::read_dir(root.join("libobs/cmake/rust-exports.d")).unwrap();
    for entry in exports {
        let text = fs::read_to_string(entry.unwrap().path()).unwrap();
        assert!(
            !text
                .lines()
                .any(|line| line == "obs_source_preload_video_internal")
        );
    }
}
