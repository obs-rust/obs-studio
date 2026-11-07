use super::*;

/// #1390: keep both unchanged display-free XComposite decision suites.
#[test]
fn canonical_xcomposite_decisions_are_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph.retain_suite("core");
    for (name, header) in [
        (
            "test_xcomposite_presence",
            "plugins/linux-capture/xcomposite-presence.h",
        ),
        (
            "test_xcomposite_search_backoff",
            "plugins/linux-capture/xcomposite-search.h",
        ),
    ] {
        let target = graph
            .targets
            .get(name)
            .unwrap_or_else(|| panic!("canonical graph omits {name}"));
        assert!(
            matches!(&target.sources[..], [Source::Path(path)] if path == &format!("test/cmocka/{name}.c"))
        );
        assert!(
            matches!(&target.libraries[..], [ConditionalValue::Name(library)] if library == "OBS::native-test-cmocka")
        );
        assert_eq!(target.includes, ["."]);
        assert_eq!(target.watch_inputs, [header]);
        let test = &graph.tests[name];
        assert_eq!(test.target, name);
        assert_eq!(test.command, ["${target}"]);
        assert_eq!(test.timeout_seconds, Some(30));
    }
    graph
        .targets
        .retain(|name, _| name.starts_with("test_xcomposite_"));
    graph
        .tests
        .retain(|name, _| name.starts_with("test_xcomposite_"));
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph.select(&[(os.into(), true)].into()).unwrap();
        assert_eq!(selected.targets.len(), 2);
        assert_eq!(selected.tests.len(), 2);
    }
}
