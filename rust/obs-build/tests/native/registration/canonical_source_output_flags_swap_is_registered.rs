use super::*;

#[test]
fn canonical_source_output_flags_swap_is_registered() {
    use obs_build::validation::graph::{Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let names = ["test_source_output_flags", "test_source_output_flags_rust"];
    graph.targets.retain(|key, _| names.contains(&key.as_str()));
    graph.tests.retain(|key, _| names.contains(&key.as_str()));
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph.select(&[(os.into(), true)].into()).unwrap();
        for name in names {
            assert!(
                matches!(selected.targets[name].sources.as_slice(), [Source::Path(path)]
                if path == "test/cmocka/test_source_output_flags.c")
            );
            assert_eq!(selected.tests[name].target, name);
        }
        // The twin runs the unchanged assertions on the exported symbol of
        // the real libobs: Rust in hybrid, the original C in the reference.
        assert!(selected.targets[names[0]].definitions.is_empty());
        assert_eq!(
            selected.targets[names[1]].definitions,
            ["SOURCE_OUTPUT_FLAGS_EXTERNAL"]
        );
    }
    let ported =
        fs::read_to_string(root.join("libobs/build/ported-source-sets/source-output-flags.json"))
            .unwrap();
    assert!(ported.contains("build/libobs.ported-source-output-flags+.h.c"));
    let group =
        fs::read_to_string(root.join("libobs/build/libobs.ported-source-output-flags+.h.c"))
            .unwrap();
    assert!(group.contains("#include \"libobs/obs-source-output-flags.c.h\""));
    let hybrid = fs::read_to_string(root.join("libobs/build/libobs.core-source+.h.c")).unwrap();
    assert!(!hybrid.contains("#include \"libobs/obs-source-output-flags.c.h\""));
    assert!(hybrid.contains("#include \"libobs/obs-source-audio-mixers-fields.c.h\""));
    let exports = fs::read_to_string(
        root.join("libobs/cmake/rust-exports.d/obs-source-source_output_flags.txt"),
    )
    .unwrap();
    assert_eq!(exports.trim(), "obs_source_get_output_flags");
}
