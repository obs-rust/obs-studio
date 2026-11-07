use super::*;

/// #1390: retain the unchanged settings UI layout tests in the native graph.
#[test]
fn canonical_settings_track_rows_is_registered() {
    use obs_build::validation::graph::Graph;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph.retain_suite("qt");
    let target = graph
        .targets
        .get("test_settings_track_rows")
        .expect("canonical graph omits test_settings_track_rows");
    assert_eq!(target.sources.len(), 1);
    assert_eq!(target.qt.uic, ["frontend/forms/OBSBasicSettings.ui"]);
    assert_eq!(target.includes_before, ["test/cmocka/settings-ui-stubs"]);
    assert_eq!(target.watch_inputs, ["test/cmocka/settings-ui-stubs"]);
    let test = &graph.tests["test_settings_track_rows"];
    assert_eq!(test.qt_runtime.as_deref(), Some("Qt6::Core"));
    assert_eq!(test.timeout_seconds, Some(120));
    for linux in [false, true] {
        for widgets in [false, true] {
            let facts = [
                ("OS_LINUX".into(), linux),
                ("MSVC".into(), !linux),
                ("HAVE_QT6_CORE".into(), true),
                ("HAVE_QT6_WIDGETS".into(), widgets),
                ("HAVE_QT6_TEST".into(), false),
            ]
            .into();
            let selected = graph.select(&facts).unwrap();
            assert_eq!(
                selected.tests.contains_key("test_settings_track_rows"),
                widgets
            );
        }
    }
}
