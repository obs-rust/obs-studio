use super::*;

#[test]
fn canonical_output_get_last_error_swap_is_registered() {
    use obs_build::validation::graph::{Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let name = "test_output_get_last_error";
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph.select(&[(os.into(), true)].into()).unwrap();
        assert!(
            matches!(selected.targets[name].sources.as_slice(), [Source::Path(path)]
            if path == "test/cmocka/test_output_get_last_error.c")
        );
        assert_eq!(selected.tests[name].target, name);
    }
    let ported =
        fs::read_to_string(root.join("libobs/build/ported-source-sets/output-get-last-error.json"))
            .unwrap();
    assert!(ported.contains("build/libobs.ported-output-get-last-error+.h.c"));
    let original = "#include \"libobs/obs-output-get-last-error.c.h\"";
    let group =
        fs::read_to_string(root.join("libobs/build/libobs.ported-output-get-last-error+.h.c"))
            .unwrap();
    assert!(group.contains(original));
    let hybrid = fs::read_to_string(root.join("libobs/build/libobs.core-output+.h.c")).unwrap();
    assert!(!hybrid.contains(original));
    // The production adapters the Rust query reads through stay in the core.
    for adapter in [
        "obs-output-last-error-fields.c.h",
        "obs-output-video-encoder-getter-fields.c.h",
        "obs-output-encoder-getters-fields.c.h",
        "obs-encoder-last-error-fields.c.h",
    ] {
        assert!(hybrid.contains(&format!("#include \"libobs/{adapter}\"")));
    }
    let exports = fs::read_to_string(
        root.join("libobs/cmake/rust-exports.d/obs-output-output_get_last_error.txt"),
    )
    .unwrap();
    assert_eq!(
        exports.lines().collect::<Vec<_>>(),
        ["obs_output_get_last_error"]
    );
}
