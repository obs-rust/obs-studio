use super::*;

#[test]
fn canonical_source_last_async_ts_swap_is_registered() {
    use obs_build::validation::graph::{Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let name = "test_source_last_async_ts";
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph.select(&[(os.into(), true)].into()).unwrap();
        // The unchanged test compiles the extracted C getter into itself (the
        // getter is hidden in libobs); it lists no other source.
        assert!(
            matches!(selected.targets[name].sources.as_slice(), [Source::Path(path)]
            if path == "test/cmocka/test_source_last_async_ts.c")
        );
        assert!(selected.targets[name].definitions.is_empty());
        assert_eq!(selected.tests[name].target, name);
    }
    let test = fs::read_to_string(root.join("test/cmocka/test_source_last_async_ts.c")).unwrap();
    let include = "#include \"libobs/obs-source-async-last-ts-getter.c.h\"";
    assert!(test.contains(include));
    // Hybrid libobs no longer compiles the C getter: the reference-only group
    // beside the async flag getters does, and the Rust shim stays hidden as
    // the C was.
    let hybrid = fs::read_to_string(root.join("libobs/build/libobs.core-source+.h.c")).unwrap();
    assert!(!hybrid.contains(include));
    assert!(hybrid.contains("#include \"libobs/obs-source-async-flag-fields.c.h\""));
    let group = "libobs.ported-source-async-flag-getters+.h.c";
    let ported = fs::read_to_string(
        root.join("libobs/build/ported-source-sets/source-async-flag-getters.json"),
    )
    .unwrap();
    assert!(ported.contains(&format!("build/{group}")));
    let text = fs::read_to_string(root.join(format!("libobs/build/{group}"))).unwrap();
    assert!(text.contains(include));
    let names = |dir: &str| {
        fs::read_to_string(root.join(format!(
            "libobs/cmake/{dir}/obs-source-source_async_flag_getters.txt"
        )))
        .unwrap()
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map(str::to_owned)
        .collect::<Vec<_>>()
    };
    assert!(!names("rust-exports.d").contains(&"obs_source_get_last_async_ts".to_owned()));
    assert_eq!(names("rust-hidden.d"), ["obs_source_get_last_async_ts"]);
}
