use super::*;

#[test]
fn canonical_encoder_priming_samples_swap_is_registered() {
    use obs_build::validation::graph::{Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let name = "test_encoder_priming_samples";
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph.select(&[(os.into(), true)].into()).unwrap();
        assert!(
            matches!(selected.targets[name].sources.as_slice(), [Source::Path(path)]
            if path == "test/cmocka/test_encoder_priming_samples.c")
        );
        assert_eq!(selected.tests[name].target, name);
    }
    let ported = fs::read_to_string(
        root.join("libobs/build/ported-source-sets/encoder-priming-samples.json"),
    )
    .unwrap();
    assert!(ported.contains("build/libobs.ported-encoder-priming-samples+.h.c"));
    let group =
        fs::read_to_string(root.join("libobs/build/libobs.ported-encoder-priming-samples+.h.c"))
            .unwrap();
    assert!(group.contains("#include \"libobs/obs-encoder-priming-samples.c.h\""));
    let hybrid =
        fs::read_to_string(root.join("libobs/build/libobs.core-encoder-priming-samples+.h.c"))
            .unwrap();
    assert!(!hybrid.contains("#include \"libobs/obs-encoder-priming-samples.c.h\""));
    assert!(hybrid.contains("#include \"libobs/obs-encoder-priming-samples-fields.c.h\""));
}
