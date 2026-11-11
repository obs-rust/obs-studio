use super::*;

/// #3658: Cargo must retain CMake's unchanged Paste Filters compatibility cases.
#[test]
fn canonical_filter_clipboard_is_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let graph: serde_json::Value = obs_build::validation::graph::Graph::read_value(
        &root.join("test/cmocka/build/test-graph.d"),
    )
    .unwrap();
    let target = graph["targets"]
        .get("test_filter_clipboard")
        .expect("canonical graph omits test_filter_clipboard");
    assert_eq!(
        target["sources"],
        serde_json::json!([
            "test/cmocka/test_filter_clipboard.c",
            "frontend/utility/filter-clipboard.c.h"
        ])
    );
    assert_eq!(target["includes"], serde_json::json!(["frontend/utility"]));
    assert_eq!(
        target["watch_inputs"],
        serde_json::json!(["frontend/utility"])
    );
    let selected_graph = serde_json::json!({
        "schema": 1,
        "targets": {"test_filter_clipboard": target},
        "tests": {"test_filter_clipboard": graph["tests"]["test_filter_clipboard"]}
    });
    let parsed = obs_build::validation::graph::Graph::parse(&selected_graph.to_string()).unwrap();
    let selected = parsed.select(&Default::default()).unwrap();
    assert!(selected.targets.contains_key("test_filter_clipboard"));
    assert!(selected.tests.contains_key("test_filter_clipboard"));
    assert_eq!(
        target["libraries"],
        serde_json::json!(["OBS::libobs", "OBS::native-test-cmocka"])
    );
    assert_eq!(
        graph["tests"]["test_filter_clipboard"],
        serde_json::json!({"target": "test_filter_clipboard", "command": ["${target}"]})
    );
    let fragment =
        fs::read_to_string(root.join("test/cmocka/tests.d/test_filter_clipboard.cmake")).unwrap();
    assert!(!fragment.contains("add_executable("));
    assert!(!fragment.contains("add_test("));
}
