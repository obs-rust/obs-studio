use super::*;

/// #1390: the QtCore-only frontend API helper must remain available without Widgets.
#[test]
fn canonical_frontend_api_call_is_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let graph =
        obs_build::validation::graph::Graph::load(&root.join("test/cmocka/build/test-graph.json"))
            .unwrap();
    let target = graph
        .targets
        .get("test_frontend_api_call")
        .expect("canonical graph omits test_frontend_api_call");
    assert_eq!(target.sources.len(), 1);
    assert_eq!(
        target.watch_inputs,
        ["frontend/utility/frontend-api-call.hpp"]
    );
    let test = &graph.tests["test_frontend_api_call"];
    assert_eq!(test.target, "test_frontend_api_call");
    assert_eq!(test.suite, "qt");
    assert_eq!(test.timeout_seconds, Some(60));
    assert_eq!(test.qt_runtime.as_deref(), Some("Qt6::Core"));
    let mut qt =
        obs_build::validation::graph::Graph::load(&root.join("test/cmocka/build/test-graph.json"))
            .unwrap();
    qt.retain_suite("qt");
    for have_core in [false, true] {
        let facts = [
            ("OS_LINUX".into(), false),
            ("MSVC".into(), false),
            ("HAVE_QT6_CORE".into(), have_core),
            ("HAVE_QT6_WIDGETS".into(), false),
            ("HAVE_QT6_TEST".into(), false),
        ]
        .into();
        let selected = qt.select(&facts).unwrap();
        assert_eq!(
            selected.tests.contains_key("test_frontend_api_call"),
            have_core
        );
        for widgets_test in ["test_remux_queue_model", "test_profile_import"] {
            assert!(!selected.tests.contains_key(widgets_test));
        }
    }
}
