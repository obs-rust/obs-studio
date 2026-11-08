use super::*;
use obs_build::validation::graph::Graph;
const CASES: [&str; 6] = [
    "test_ws_image_mime",
    "test_ws_session_duration",
    "test_ws_batch_order",
    "test_ws_current_scene_removed",
    "test_ws_list_property_items",
    "test_ws_scene_count",
];
fn expected() -> serde_json::Value {
    serde_json::from_str(include_str!("websocket_helpers_expected.json")).unwrap()
}
#[test]
fn canonical_websocket_helpers_is_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let raw: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap())
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
        assert!(fragment.contains("test-graph.json"));
        assert!(!fragment.contains("add_executable("));
        assert!(!fragment.contains("add_test("));
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

#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
#[path = "websocket_helpers_runtime.rs"]
mod runtime;

#[path = "configured_headers.rs"]
mod configured_headers;
