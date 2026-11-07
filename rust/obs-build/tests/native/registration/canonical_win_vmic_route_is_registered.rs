use super::*;

/// #1390: portable virtual microphone routing must reach both native consumers.
#[test]
fn canonical_win_vmic_route_is_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let name = "test_win_vmic_route";
    let target = graph
        .targets
        .get(name)
        .expect("canonical graph omits test_win_vmic_route");
    assert!(matches!(target.sources.as_slice(), [Source::Path(path)]
        if path == "test/cmocka/test_win_vmic_route.c"));
    assert_eq!(target.includes, ["plugins/win-dshow"]);
    assert!(
        matches!(target.libraries.as_slice(), [ConditionalValue::Name(name)]
        if name == "OBS::native-test-cmocka")
    );
    assert!(target.header_libraries.is_empty());
    assert_eq!(target.watch_inputs, ["plugins/win-dshow/vmic-route.h"]);
    let test = &graph.tests[name];
    assert_eq!(test.target, name);
    assert_eq!(test.command, ["${target}"]);
    assert_eq!(test.suite, "core");
    assert_eq!(test.timeout_seconds, None);
    assert_eq!(test.skip_return_code, None);
    assert!(test.optional_prefixes.is_empty());
    assert_eq!(test.qt_runtime, None);
    let fragment =
        fs::read_to_string(root.join("test/cmocka/tests.d/test_win_vmic_route.cmake")).unwrap();
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
