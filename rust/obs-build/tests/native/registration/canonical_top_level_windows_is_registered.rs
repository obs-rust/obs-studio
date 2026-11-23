use super::*;

/// #1390: retain all five unchanged top-level window close cases. The
/// floating dock case (#5409) also watches `dock-window.hpp`. #5478: the
/// binary was `test_close_top_level`; it also holds the quit, dock window,
/// moved dock and redock cases.
#[test]
fn canonical_top_level_windows_is_registered() {
    use obs_build::validation::graph::Graph;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph.retain_suite("qt");
    let target = graph
        .targets
        .get("test_top_level_windows")
        .expect("canonical graph omits test_top_level_windows");
    assert_eq!(target.sources.len(), 1);
    assert!(
        matches!(&target.sources[0], obs_build::validation::graph::Source::Path(path) if path == "test/cmocka/test_top_level_windows.cpp")
    );
    assert!(matches!(
        target.cpp_standard,
        Some(obs_build::CppStandard::Cpp17)
    ));
    assert_eq!(target.includes, ["frontend/utility"]);
    assert_eq!(
        target.watch_inputs,
        [
            "frontend/utility/close-top-level.hpp",
            "frontend/utility/dock-window.hpp",
        ]
    );
    let test = &graph.tests["test_top_level_windows"];
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
            assert_eq!(
                selected.tests.contains_key("test_top_level_windows"),
                widgets
            );
        }
    }
}
