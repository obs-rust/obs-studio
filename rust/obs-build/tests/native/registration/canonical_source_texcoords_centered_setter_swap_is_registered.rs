use super::*;

#[test]
fn canonical_source_texcoords_centered_setter_swap_is_registered() {
    use obs_build::validation::graph::{Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let name = "test_source_texcoords_centered_setter";
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph.select(&[(os.into(), true)].into()).unwrap();
        // The unchanged test compiles the extracted C setter into itself (the
        // setter is hidden in libobs) and reads back through the getter the
        // real libobs exports; it lists no other source.
        assert!(
            matches!(selected.targets[name].sources.as_slice(), [Source::Path(path)]
            if path == "test/cmocka/test_source_texcoords_centered_setter.c")
        );
        assert!(selected.targets[name].definitions.is_empty());
        assert_eq!(selected.tests[name].target, name);
    }
    let test = fs::read_to_string(root.join("test/cmocka/test_source_texcoords_centered_setter.c"))
        .unwrap();
    let include = "#include \"libobs/obs-source-texcoords-centered-setter.c.h\"";
    assert!(test.contains(include));
    // Hybrid libobs no longer compiles the C setter: the reference-only group
    // beside the getters does, and the Rust shim stays hidden as the C was.
    let hybrid = fs::read_to_string(root.join("libobs/build/libobs.core-source+.h.c")).unwrap();
    assert!(!hybrid.contains(include));
    assert!(hybrid.contains("#include \"libobs/obs-source-boolean-query-fields.c.h\""));
    let group = "libobs.ported-source-boolean-queries+.h.c";
    let ported = fs::read_to_string(
        root.join("libobs/build/ported-source-sets/source-boolean-queries.json"),
    )
    .unwrap();
    assert!(ported.contains(&format!("build/{group}")));
    let text = fs::read_to_string(root.join(format!("libobs/build/{group}"))).unwrap();
    assert!(text.contains(include));
    let names = |dir: &str| {
        fs::read_to_string(root.join(format!(
            "libobs/cmake/{dir}/obs-source-source_boolean_queries.txt"
        )))
        .unwrap()
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map(str::to_owned)
        .collect::<Vec<_>>()
    };
    assert_eq!(
        names("rust-exports.d"),
        ["obs_source_get_texcoords_centered", "obs_source_is_hidden"]
    );
    assert_eq!(
        names("rust-hidden.d"),
        ["obs_source_set_texcoords_centered"]
    );
}
