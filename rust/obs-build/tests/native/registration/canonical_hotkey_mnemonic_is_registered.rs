use super::*;

/// #1390: preserve the Linux-only Widgets/Test guard of the unchanged mnemonic cases.
#[test]
fn canonical_hotkey_mnemonic_is_registered() {
    use obs_build::validation::graph::Graph;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.d")).unwrap();
    graph.retain_suite("qt");
    let target = graph
        .targets
        .get("test_hotkey_capture_mnemonic")
        .expect("canonical graph omits test_hotkey_capture_mnemonic");
    assert_eq!(
        target.watch_inputs,
        ["frontend/settings/OBSHotkeyCapture.hpp"]
    );
    assert_eq!(
        graph.tests["test_hotkey_capture_mnemonic"]
            .qt_runtime
            .as_deref(),
        Some("Qt6::Core")
    );
    assert_eq!(
        graph.tests["test_hotkey_capture_mnemonic"].resource_locks,
        ["timing"]
    );
    for linux in [false, true] {
        for widgets in [false, true] {
            for qt_test in [false, true] {
                let facts = [
                    ("OS_LINUX".into(), linux),
                    ("OS_WINDOWS".into(), !linux),
                    ("HAVE_QT6_CORE".into(), false),
                    ("HAVE_QT6_WIDGETS".into(), widgets),
                    ("HAVE_QT6_TEST".into(), qt_test),
                ]
                .into();
                let selected = graph.select(&facts).unwrap();
                assert_eq!(
                    selected.tests.contains_key("test_hotkey_capture_mnemonic"),
                    linux && widgets && qt_test
                );
            }
        }
    }
}
