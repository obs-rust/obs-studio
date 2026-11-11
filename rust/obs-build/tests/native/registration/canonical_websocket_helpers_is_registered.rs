use super::*;
use obs_build::validation::graph::Graph;
const CASES: [&str; 9] = [
    "test_ws_image_mime",
    "test_ws_session_duration",
    "test_ws_batch_order",
    "test_ws_current_scene_removed",
    "test_ws_list_property_items",
    "test_ws_scene_count",
    "test_ws_filter_settings_event",
    "test_ws_remove_input",
    "test_ws_transform_bounds",
];
fn expected() -> serde_json::Value {
    serde_json::from_str(include_str!("websocket_helpers_expected.json")).unwrap()
}
#[test]
fn canonical_websocket_helpers_is_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let raw: serde_json::Value = obs_build::validation::graph::Graph::read_value(
        &root.join("test/cmocka/build/test-graph.d"),
    )
    .unwrap();
    let expected = expected();
    for name in CASES {
        for section in ["targets", "tests"] {
            assert_eq!(
                raw[section][name], expected[section][name],
                "{section}/{name}"
            );
        }
        let fragment =
            fs::read_to_string(root.join(format!("test/cmocka/tests.d/{name}.cmake"))).unwrap();
        assert!(fragment.contains("test-graph.d"));
        assert!(!fragment.contains("add_executable("));
        assert!(!fragment.contains("add_test("));
        assert!(!fragment.contains("obs_native_test_graph("));
    }
}
#[test]
fn canonical_websocket_helpers_optional_owners() {
    let graph = Graph::parse(&expected().to_string()).unwrap();
    let mut facts =
        std::collections::BTreeMap::from([("OS_LINUX".into(), true), ("OS_WINDOWS".into(), false)]);
    assert!(graph.select(&facts).unwrap().tests.is_empty());
    for name in CASES {
        let owner = format!(
            "HAVE_{}_TEST_OWNER",
            name.trim_start_matches("test_").to_uppercase()
        );
        facts.insert(owner.clone(), true);
        assert!(graph.select(&facts).unwrap().tests.contains_key(name));
        facts.insert(owner, false);
        assert!(!graph.select(&facts).unwrap().targets.contains_key(name));
    }
}

#[test]
fn canonical_websocket_helpers_preserve_original_hosts() {
    let graph = Graph::parse(&expected().to_string()).unwrap();
    for (linux, windows) in [(true, false), (false, false), (false, true)] {
        let mut facts = std::collections::BTreeMap::from([
            ("OS_LINUX".into(), linux),
            ("OS_WINDOWS".into(), windows),
        ]);
        for name in CASES {
            facts.insert(
                format!(
                    "HAVE_{}_TEST_OWNER",
                    name.trim_start_matches("test_").to_uppercase()
                ),
                true,
            );
        }
        let selected = graph.select(&facts).unwrap();
        for name in CASES {
            let enabled = match name {
                "test_ws_current_scene_removed"
                | "test_ws_list_property_items"
                | "test_ws_remove_input"
                | "test_ws_transform_bounds" => linux,
                "test_ws_batch_order" | "test_ws_scene_count" | "test_ws_filter_settings_event" => {
                    !windows
                }
                _ => true,
            };
            assert_eq!(
                selected.targets.contains_key(name),
                enabled,
                "{name}: {facts:?}"
            );
            assert_eq!(
                selected.tests.contains_key(name),
                enabled,
                "{name}: {facts:?}"
            );
        }
    }
}

#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
#[path = "support/websocket_helpers_runtime.rs"]
mod runtime;

#[path = "support/configured_headers.rs"]
mod configured_headers;
