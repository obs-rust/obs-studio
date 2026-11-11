use super::*;

/// #1390: the original nine dock cases need Linux, Widgets and Qt Test. Since
/// #5230 they test the adapter (frontend/utility/DockLayout) with the
/// frontend's Rust model, so they link its owner interface and libobs.
#[test]
fn canonical_dock_proportions_is_registered() {
    use obs_build::validation::graph::Graph;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.d")).unwrap();
    graph.retain_suite("qt");
    let target = graph
        .targets
        .get("test_dock_proportions")
        .expect("canonical graph omits test_dock_proportions");
    assert_eq!(target.sources.len(), 2);
    assert!(
        matches!(&target.sources[0], obs_build::validation::graph::Source::Path(path) if path == "test/cmocka/test_dock_proportions.cpp")
    );
    assert!(
        matches!(&target.sources[1], obs_build::validation::graph::Source::Path(path) if path == "frontend/build/frontend.dock-layout+.hpp.cpp")
    );
    assert_eq!(target.includes, ["frontend"]);
    assert_eq!(
        target
            .libraries
            .iter()
            .map(|library| match library {
                obs_build::validation::graph::Library::Name(name) => name.as_str(),
                _ => panic!("dock libraries must be unconditional"),
            })
            .collect::<Vec<_>>(),
        [
            "OBS::fallback-warning-test-frontend",
            "OBS::libobs",
            "Qt6::Widgets",
            "Qt6::Test",
            "OBS::native-test-cmocka"
        ]
    );
    assert_eq!(
        target.watch_inputs,
        [
            "frontend/build/frontend-dock-layout.pch.hpp",
            "frontend/utility/DockLayout.cpp.hpp",
            "frontend/utility/DockLayout.hpp",
            "frontend/utility/ExtraDocks.cpp.hpp",
            "frontend/utility/ExtraDocks.hpp",
            "frontend/utility/obs-dock-layout.h"
        ]
    );
    let test = &graph.tests["test_dock_proportions"];
    assert_eq!(test.timeout_seconds, Some(120));
    assert_eq!(test.resource_locks, ["timing"]);
    assert_eq!(test.qt_runtime.as_deref(), Some("Qt6::Core"));
    for linux in [false, true] {
        for widgets in [false, true] {
            for qt_test in [false, true] {
                let facts = [
                    ("OS_LINUX".into(), linux),
                    ("OS_WINDOWS".into(), !linux),
                    ("HAVE_QT6_CORE".into(), false),
                    ("HAVE_QT6_WIDGETS".into(), widgets),
                    ("HAVE_QT6_TEST".into(), qt_test),
                    ("HAVE_QT_WIDGETS_TEST_OWNER".into(), widgets),
                    ("HAVE_QT_WIDGETS_TESTLIB_OWNER".into(), qt_test),
                ]
                .into();
                let selected = graph.select(&facts).unwrap();
                assert_eq!(
                    selected.tests.contains_key("test_dock_proportions"),
                    linux && widgets && qt_test
                );
            }
        }
    }
}
