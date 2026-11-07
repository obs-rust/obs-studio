use super::*;
use obs_build::validation::graph::Graph;

const NAMES: [&str; 2] = ["test_happy_eyeballs", "test_happy_eyeballs_gai_error"];

#[test]
fn canonical_libobs_go3_is_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut raw: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap())
            .unwrap();
    for name in NAMES {
        assert!(
            !raw["targets"][name].is_null(),
            "canonical graph omits {name}"
        );
        assert_eq!(
            raw["targets"][name],
            serde_json::json!({
                "sources": [format!("test/cmocka/{name}.c")],
                "requires": "HAVE_HAPPY_EYEBALLS",
                "libraries": ["OBS::libobs", "OBS::happy-eyeballs", "OBS::native-test-cmocka",
                    {"name": "ws2_32", "requires": "OS_WINDOWS"}],
                "watch_inputs": ["shared/happy-eyeballs"]
            })
        );
        assert_eq!(
            raw["tests"][name],
            serde_json::json!({
                "target": name, "suite": "core", "requires": "HAVE_HAPPY_EYEBALLS"
            })
        );
        let fragment =
            fs::read_to_string(root.join(format!("test/cmocka/tests.d/{name}.cmake"))).unwrap();
        assert!(!fragment.contains("add_executable("));
        assert!(!fragment.contains("add_test("));
    }
    for section in ["targets", "tests"] {
        raw[section]
            .as_object_mut()
            .unwrap()
            .retain(|name, _| NAMES.contains(&name.as_str()));
    }
    let graph = Graph::parse(&raw.to_string()).unwrap();
    for windows in [false, true] {
        for available in [false, true] {
            let facts = [
                ("OS_WINDOWS".into(), windows),
                ("HAVE_HAPPY_EYEBALLS".into(), available),
            ]
            .into();
            let selected = graph.select(&facts).unwrap();
            for name in NAMES {
                assert_eq!(selected.targets.contains_key(name), available);
                assert_eq!(selected.tests.contains_key(name), available);
            }
        }
    }
}
