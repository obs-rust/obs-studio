use super::*;

#[test]
fn canonical_undo_stack_is_registered() {
    use obs_build::validation::graph::Graph;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph.retain_suite("qt");
    let target = graph
        .targets
        .get("test_undo_stack_enable")
        .expect("canonical graph omits test_undo_stack_enable");
    assert_eq!(target.qt.moc, ["frontend/utility/undo_stack.hpp"]);
    assert_eq!(target.includes_before, ["test/cmocka/undo-stack-stubs"]);
    assert_eq!(target.watch_inputs, ["test/cmocka/undo-stack-stubs"]);
    let test = &graph.tests["test_undo_stack_enable"];
    assert_eq!(test.timeout_seconds, Some(120));
    assert_eq!(test.qt_runtime.as_deref(), Some("Qt6::Core"));
    for linux in [false, true] {
        for widgets in [false, true] {
            let facts = [
                ("OS_LINUX".into(), linux),
                ("OS_WINDOWS".into(), !linux),
                ("HAVE_QT6_CORE".into(), false),
                ("HAVE_QT6_WIDGETS".into(), widgets),
                ("HAVE_QT6_TEST".into(), false),
            ]
            .into();
            let selected = graph.select(&facts).unwrap();
            assert_eq!(
                selected.tests.contains_key("test_undo_stack_enable"),
                linux && widgets
            );
        }
    }
}
