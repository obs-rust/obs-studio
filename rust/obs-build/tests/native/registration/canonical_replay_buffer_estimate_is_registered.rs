use super::*;

/// #1390: keep the original Linux replay-buffer estimate cases in the Qt graph.
#[test]
fn canonical_replay_buffer_estimate_is_registered() {
    use obs_build::validation::graph::Graph;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.d")).unwrap();
    graph.retain_suite("qt");
    let target = graph
        .targets
        .get("test_replay_buffer_estimate")
        .expect("canonical graph omits test_replay_buffer_estimate");
    assert_eq!(
        target
            .sources
            .iter()
            .map(|source| match source {
                obs_build::validation::graph::Source::Path(path) => path.as_str(),
                obs_build::validation::graph::Source::Conditional { .. } =>
                    panic!("replay sources must be unconditional"),
            })
            .collect::<Vec<_>>(),
        [
            "test/cmocka/test_replay_buffer_estimate.cpp",
            "frontend/utility/replay-buffer-estimate.cpp.hpp",
        ]
    );
    assert_eq!(target.includes, ["frontend/utility"]);
    assert_eq!(
        target.watch_inputs,
        ["frontend/utility/replay-buffer-estimate.hpp"]
    );
    assert_eq!(
        target
            .libraries
            .iter()
            .map(|library| match library {
                obs_build::validation::graph::ConditionalValue::Name(name) => name.as_str(),
                obs_build::validation::graph::ConditionalValue::Conditional { .. } =>
                    panic!("replay libraries must be unconditional"),
            })
            .collect::<Vec<_>>(),
        ["Qt6::Widgets", "OBS::native-test-cmocka"]
    );
    let test = &graph.tests["test_replay_buffer_estimate"];
    assert_eq!(test.qt_runtime.as_deref(), Some("Qt6::Core"));
    for linux in [false, true] {
        for widgets in [false, true] {
            let facts = [
                ("OS_LINUX".into(), linux),
                ("OS_WINDOWS".into(), !linux),
                ("MSVC".into(), !linux),
                ("HAVE_QT6_CORE".into(), true),
                ("HAVE_QT6_WIDGETS".into(), widgets),
                ("HAVE_QT6_TEST".into(), false),
            ]
            .into();
            let selected = graph.select(&facts).unwrap();
            assert_eq!(
                selected.tests.contains_key("test_replay_buffer_estimate"),
                linux && widgets
            );
        }
    }
}
