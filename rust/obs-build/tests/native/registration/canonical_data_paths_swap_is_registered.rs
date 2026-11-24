use super::*;

#[test]
fn canonical_data_paths_swap_is_registered() {
    use obs_build::validation::graph::{Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let name = "test_data_paths";
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph.select(&[(os.into(), true)].into()).unwrap();
        // The unchanged test compiles no copy of the wrappers: it calls the
        // exported symbols of the real libobs (Rust in hybrid, the original C
        // in the reference).
        assert!(
            matches!(selected.targets[name].sources.as_slice(), [Source::Path(path)]
            if path == "test/cmocka/test_data_paths.c")
        );
        assert!(selected.targets[name].definitions.is_empty());
        assert_eq!(selected.tests[name].target, name);
    }
    let ported =
        fs::read_to_string(root.join("libobs/build/ported-source-sets/data-paths.json")).unwrap();
    assert!(ported.contains("build/libobs.ported-data-paths+.h.c"));
    let group =
        fs::read_to_string(root.join("libobs/build/libobs.ported-data-paths+.h.c")).unwrap();
    assert!(group.contains("#include \"libobs/obs-data-paths.c.h\""));
    // The list obs_find_data_file() reads stays in the live core group, so
    // the original wrappers (reference) and the Rust ones (hybrid) share it.
    let hybrid = fs::read_to_string(root.join("libobs/build/libobs.core+.h.c")).unwrap();
    assert!(!hybrid.contains("#include \"libobs/obs-data-paths.c.h\""));
    assert!(hybrid.contains("#include \"libobs/obs-data-paths-fields.c.h\""));
    let original = fs::read_to_string(root.join("libobs/obs-data-paths.c.h")).unwrap();
    assert!(!original.contains("static DARRAY"));
    let exports =
        fs::read_to_string(root.join("libobs/cmake/rust-exports.d/obs-core-data_paths.txt"))
            .unwrap();
    assert_eq!(
        exports.lines().collect::<Vec<_>>(),
        ["obs_add_data_path", "obs_remove_data_path"]
    );
}
