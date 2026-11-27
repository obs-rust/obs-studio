use super::*;

#[test]
fn canonical_source_activate_swap_is_registered() {
    use obs_build::validation::graph::{Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let name = "test_source_activate";
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph.select(&[(os.into(), true)].into()).unwrap();
        // The test compiles no copy of the C: it reaches the two hidden
        // entries through the exported counters of the real libobs (Rust in
        // hybrid, the original C in the reference).
        assert!(
            matches!(selected.targets[name].sources.as_slice(), [Source::Path(path)]
            if path == "test/cmocka/test_source_activate.c")
        );
        assert!(selected.targets[name].definitions.is_empty());
        assert_eq!(selected.tests[name].target, name);
    }
    let ported =
        fs::read_to_string(root.join("libobs/build/ported-source-sets/source-activate.json"))
            .unwrap();
    assert!(ported.contains("build/libobs.ported-source-activate+.h.c"));
    let group =
        fs::read_to_string(root.join("libobs/build/libobs.ported-source-activate+.h.c")).unwrap();
    assert!(group.contains("#include \"libobs/obs-source-activate.c.h\""));
    let hybrid = fs::read_to_string(root.join("libobs/build/libobs.core-source+.h.c")).unwrap();
    assert!(!hybrid.contains("#include \"libobs/obs-source-activate.c.h\""));
    assert!(hybrid.contains("#include \"libobs/obs-source-activate-fields.c.h\""));
    // Declared in obs-internal.h without EXPORT: hidden, never exported.
    let hidden =
        fs::read_to_string(root.join("libobs/cmake/rust-hidden.d/obs-source-source_activate.txt"))
            .unwrap();
    assert_eq!(
        hidden
            .lines()
            .filter(|line| !line.starts_with('#'))
            .collect::<Vec<_>>(),
        ["obs_source_activate", "obs_source_deactivate"]
    );
    assert!(
        !root
            .join("libobs/cmake/rust-exports.d/obs-source-source_activate.txt")
            .exists()
    );
}
