use super::*;

#[test]
fn canonical_source_refs_swap_is_registered() {
    use obs_build::validation::graph::{Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let name = "test_source_refs";
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph.select(&[(os.into(), true)].into()).unwrap();
        // The test compiles no copy of the C: it calls the exported entries
        // of the real libobs (Rust in hybrid, the original C in the
        // reference).
        assert!(
            matches!(selected.targets[name].sources.as_slice(), [Source::Path(path)]
            if path == "test/cmocka/test_source_refs.c")
        );
        assert!(selected.targets[name].definitions.is_empty());
        assert_eq!(selected.tests[name].target, name);
    }
    let ported =
        fs::read_to_string(root.join("libobs/build/ported-source-sets/source-refs.json")).unwrap();
    assert!(ported.contains("build/libobs.ported-source-refs+.h.c"));
    let group =
        fs::read_to_string(root.join("libobs/build/libobs.ported-source-refs+.h.c")).unwrap();
    assert!(group.contains("#include \"libobs/obs-source-refs.c.h\""));
    let hybrid = fs::read_to_string(root.join("libobs/build/libobs.core-source+.h.c")).unwrap();
    assert!(!hybrid.contains("#include \"libobs/obs-source-refs.c.h\""));
    assert!(hybrid.contains("#include \"libobs/obs-source-refs-fields.c.h\""));
    let names = |path: &str| {
        fs::read_to_string(root.join(path))
            .unwrap()
            .lines()
            .filter(|line| !line.starts_with('#'))
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    // EXPORT in obs.h: exported.
    assert_eq!(
        names("libobs/cmake/rust-exports.d/obs-source-source_refs.txt"),
        [
            "obs_source_release",
            "obs_source_get_ref",
            "obs_weak_source_get_source"
        ]
    );
    // Declared in obs-internal.h without EXPORT: hidden, never exported.
    assert_eq!(
        names("libobs/cmake/rust-hidden.d/obs-source-source_refs.txt"),
        ["obs_source_addref"]
    );
}
