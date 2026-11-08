use super::*;
use obs_build::validation::graph::Graph;
const CASES: [&str; 2] = ["test_ws_image_mime", "test_ws_session_duration"];
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
    assert!(graph.select(&Default::default()).unwrap().tests.is_empty());
    for mime in [false, true] {
        for duration in [false, true] {
            let facts = std::collections::BTreeMap::from([
                ("HAVE_WS_IMAGE_MIME_TEST_OWNER".into(), mime),
                ("HAVE_WS_SESSION_DURATION_TEST_OWNER".into(), duration),
            ]);
            let selected = graph.select(&facts).unwrap();
            for (name, enabled) in CASES.into_iter().zip([mime, duration]) {
                assert_eq!(selected.targets.contains_key(name), enabled);
                assert_eq!(selected.tests.contains_key(name), enabled);
            }
        }
    }
}

#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
#[path = "websocket_helpers_runtime.rs"]
mod runtime;
