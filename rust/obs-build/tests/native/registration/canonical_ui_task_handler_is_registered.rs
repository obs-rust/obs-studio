use super::*;

/// Keep the unchanged public-API characterization available on every host.
#[test]
fn canonical_ui_task_handler_is_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let name = "test_ui_task_handler";
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let target = &graph.targets[name];
    assert!(matches!(target.sources.as_slice(), [Source::Path(path)]
        if path == "test/cmocka/test_ui_task_handler.c"));
    let libraries: Vec<_> = target.libraries.iter().map(|library| match library {
        ConditionalValue::Name(name) => name.as_str(),
        _ => panic!("UI characterization requires unconditional linkage"),
    }).collect();
    assert_eq!(libraries, ["OBS::libobs", "OBS::native-test-cmocka"]);
    assert_eq!(graph.tests[name].target, name);
    assert_eq!(graph.tests[name].command, ["${target}"]);
    assert_eq!(graph.tests[name].optional_prefixes.len(), 1);
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph.select(&[("OS_LINUX".into(), os == "OS_LINUX"), (os.into(), true)].into()).unwrap();
        assert_eq!(selected.targets.len(), 1);
        assert_eq!(selected.tests.len(), 1);
        assert!(selected.targets.contains_key(name));
        assert!(selected.tests.contains_key(name));
    }
}
