use super::*;

/// #1390: retain all five unchanged top-level window close cases.
#[test]
fn canonical_close_top_level_is_registered() {
    use obs_build::validation::graph::Graph;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph.retain_suite("qt");
    let target = graph
        .targets
        .get("test_close_top_level")
        .expect("canonical graph omits test_close_top_level");
    assert_eq!(target.sources.len(), 1);
    assert!(
        matches!(&target.sources[0], obs_build::validation::graph::Source::Path(path) if path == "test/cmocka/test_close_top_level.cpp")
    );
    assert!(matches!(
        target.cpp_standard,
        Some(obs_build::CppStandard::Cpp17)
    ));
    assert_eq!(target.includes, ["frontend/utility"]);
    assert_eq!(
        target.watch_inputs,
        ["frontend/utility/close-top-level.hpp"]
    );
    let test = &graph.tests["test_close_top_level"];
    assert_eq!(test.qt_runtime.as_deref(), Some("Qt6::Core"));
    assert_eq!(test.timeout_seconds, Some(120));
    for linux in [false, true] {
        for widgets in [false, true] {
            let selected = graph
                .select(
                    &[
                        ("OS_LINUX".into(), linux),
                        ("OS_WINDOWS".into(), !linux),
                        ("MSVC".into(), !linux),
                        ("HAVE_QT6_CORE".into(), true),
                        ("HAVE_QT6_WIDGETS".into(), widgets),
                        ("HAVE_QT6_TEST".into(), false),
                    ]
                    .into(),
                )
                .unwrap();
            assert_eq!(selected.tests.contains_key("test_close_top_level"), widgets);
        }
    }
}
