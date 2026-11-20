use super::*;

#[test]
fn canonical_output_get_name_swap_is_registered() {
    use obs_build::validation::graph::{Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let name = "test_output_get_name";
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph.select(&[(os.into(), true)].into()).unwrap();
        assert!(
            matches!(selected.targets[name].sources.as_slice(), [Source::Path(path)]
            if path == "test/cmocka/test_output_get_name.c")
        );
        assert_eq!(selected.tests[name].target, name);
    }
    let ported =
        fs::read_to_string(root.join("libobs/build/ported-source-sets/output-get-name.json"))
            .unwrap();
    assert!(ported.contains("build/libobs.ported-output-get-name+.h.c"));
    let original = "#include \"libobs/obs-output-get-name.c.h\"";
    let group =
        fs::read_to_string(root.join("libobs/build/libobs.ported-output-get-name+.h.c")).unwrap();
    assert!(group.contains(original));
    let hybrid = fs::read_to_string(root.join("libobs/build/libobs.core-output+.h.c")).unwrap();
    assert!(!hybrid.contains(original));
    // The context-name adapter and the interleaver's name seams stay in the core.
    for adapter in [
        "obs-output-get-name-fields.c.h",
        "obs-output-interleave-name-fields.c.h",
    ] {
        assert!(hybrid.contains(&format!("#include \"libobs/{adapter}\"")));
    }
    let support =
        fs::read_to_string(root.join("test/cmocka/build/rust-static-support.json")).unwrap();
    assert!(support.contains("\"test/cmocka/output-get-name-link-support.c\""));
    let exports =
        fs::read_to_string(root.join("libobs/cmake/rust-exports.d/obs-output-output_get_name.txt"))
            .unwrap();
    assert_eq!(exports.lines().collect::<Vec<_>>(), ["obs_output_get_name"]);
}
