use super::*;

#[test]
fn canonical_module_locale_getters_is_registered() {
    use obs_build::validation::graph::{Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let name = "test_module_locale_getters";
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph.select(&[(os.into(), true)].into()).unwrap();
        let target = &selected.targets[name];
        assert!(matches!(&target.sources[..], [Source::Path(path)] if path == "test/cmocka/test_module_locale_getters.c"));
        let test = &selected.tests[name];
        assert_eq!(test.target, name);
        assert_eq!(test.command, ["${target}"]);
    }
}
