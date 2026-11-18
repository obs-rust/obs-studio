use super::*;

#[test]
fn canonical_output_video_getter_swap_is_registered() {
    use obs_build::validation::graph::{Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let name = "test_output_video_getter";
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph.select(&[(os.into(), true)].into()).unwrap();
        assert!(
            matches!(selected.targets[name].sources.as_slice(), [Source::Path(path)]
            if path == "test/cmocka/test_output_video_getter.c")
        );
        assert_eq!(selected.tests[name].target, name);
    }
    let ported =
        fs::read_to_string(root.join("libobs/build/ported-source-sets/output-video-getter.json"))
            .unwrap();
    assert!(ported.contains("build/libobs.ported-output-video-getter+.h.c"));
    let group =
        fs::read_to_string(root.join("libobs/build/libobs.ported-output-video-getter+.h.c"))
            .unwrap();
    assert!(group.contains("#include \"libobs/obs-output-video-getter-support.c.h\""));
    assert!(group.contains("#include \"libobs/obs-output-video-getter.c.h\""));
    let hybrid = fs::read_to_string(root.join("libobs/build/libobs.core-output+.h.c")).unwrap();
    assert!(!hybrid.contains("#include \"libobs/obs-output-video-getter.c.h\""));
    let exports = fs::read_to_string(
        root.join("libobs/cmake/rust-exports.d/obs-output-output_video_getter.txt"),
    )
    .unwrap();
    assert_eq!(exports.lines().collect::<Vec<_>>(), ["obs_output_video"]);
}
