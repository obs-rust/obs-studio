use super::*;

/// #1390: standalone caption compatibility must reach both native consumers.
#[test]
fn canonical_caption_unmappable_is_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let name = "test_caption_unmappable";
    let target = graph
        .targets
        .get(name)
        .expect("canonical graph omits test_caption_unmappable");
    assert!(matches!(target.sources.as_slice(), [Source::Path(path)]
        if path == "test/cmocka/test_caption_unmappable.c"));
    let libraries: Vec<_> = target
        .libraries
        .iter()
        .map(|library| match library {
            ConditionalValue::Name(name) => name.as_str(),
            _ => panic!("caption linkage must be unconditional"),
        })
        .collect();
    assert_eq!(libraries, ["OBS::caption", "OBS::native-test-cmocka"]);
    assert_eq!(target.includes, ["deps/libcaption"]);
    assert!(target.header_libraries.is_empty());
    let test = &graph.tests[name];
    assert_eq!(test.target, name);
    assert_eq!(test.command, ["${target}"]);
    assert_eq!(test.suite, "core");
    assert_eq!(test.timeout_seconds, None);
    assert_eq!(test.skip_return_code, None);
    assert!(test.optional_prefixes.is_empty());
    assert_eq!(test.qt_runtime, None);
    let fragment =
        fs::read_to_string(root.join("test/cmocka/tests.d/test_caption_unmappable.cmake")).unwrap();
    assert!(!fragment.contains("add_executable("));
    assert!(!fragment.contains("add_test("));
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph.select(&[(os.into(), true)].into()).unwrap();
        assert!(selected.targets.contains_key(name), "{os}");
        assert!(selected.tests.contains_key(name), "{os}");
    }
}
