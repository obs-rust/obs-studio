use super::*;

/// #4349: retain the unchanged public-API test on every native host.
#[test]
fn canonical_source_properties_is_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let name = "test_source_properties";
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let target = &graph.targets[name];
    assert!(matches!(target.sources.as_slice(), [Source::Path(path)]
        if path == "test/cmocka/test_source_properties.c"));
    let libraries: Vec<_> = target
        .libraries
        .iter()
        .map(|library| match library {
            ConditionalValue::Name(name) => name.as_str(),
            _ => panic!("source properties require unconditional library linkage"),
        })
        .collect();
    assert_eq!(libraries, ["OBS::libobs", "OBS::native-test-cmocka"]);
    assert_eq!(graph.tests[name].target, name);
    assert_eq!(graph.tests[name].command, ["${target}"]);
    assert_eq!(graph.tests[name].timeout_seconds, Some(30));
    assert_eq!(
        graph.tests[name].optional_prefixes,
        [vec![
            "xvfb-run",
            "-a",
            "-s",
            "-screen 0 1280x1024x24 -noreset"
        ]]
    );
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph
            .select(&[("OS_LINUX".into(), os == "OS_LINUX"), (os.into(), true)].into())
            .unwrap();
        assert!(selected.targets.contains_key(name));
        assert!(selected.tests.contains_key(name));
    }
}
