use super::*;
use obs_build::validation::graph::Graph;

const NAMES: [&str; 5] = [
    "test_graphics_host_exports",
    "test_graphics_host_functions",
    "test_image_source_host",
    "test_ingest_refresh_wait",
    "test_nvenc_caps_cache",
];

#[test]
fn canonical_libobs_go_is_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let raw: serde_json::Value = serde_json::from_slice(
        &fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap(),
    ).unwrap();
    for name in NAMES {
        let target = &raw["targets"][name];
        assert!(!target.is_null(), "canonical graph omits {name}");
        assert_eq!(target["sources"][0], format!("test/cmocka/{name}.c"));
        assert_eq!(target["libraries"][0], "OBS::libobs");
        assert_eq!(target["libraries"][1], "OBS::native-test-cmocka");
        let test = &raw["tests"][name];
        assert_eq!(test["target"], name);
        assert_eq!(test["timeout_seconds"], if name == "test_ingest_refresh_wait" { serde_json::json!(60) } else { serde_json::Value::Null });
        for field in ["optional_prefixes", "skip_return_code", "qt_runtime"] {
            assert!(test[field].is_null(), "unexpected {field}: {name}");
        }
    }
    assert_os_selection(&raw);
}

fn assert_os_selection(raw: &serde_json::Value) {
    let mut raw = raw.clone();
    for section in ["targets", "tests"] {
        raw[section].as_object_mut().unwrap().retain(|name, _| NAMES.contains(&name.as_str()));
    }
    let graph = Graph::parse(&raw.to_string()).unwrap();
    for os in ["OS_LINUX", "OS_WINDOWS", "OS_MACOS", "OS_FREEBSD", "OS_OPENBSD"] {
        let facts = ["OS_LINUX", "OS_WINDOWS", "OS_MACOS", "OS_FREEBSD", "OS_OPENBSD"].map(|fact| (fact.into(), fact == os)).into();
        let selection = graph.select(&facts).unwrap();
        for name in NAMES {
            let expected = match name {
                "test_ingest_refresh_wait" => true,
                "test_image_source_host" => os != "OS_WINDOWS" && os != "OS_MACOS",
                _ => os == "OS_LINUX",
            };
            assert_eq!(selection.targets.contains_key(name), expected, "{os}/{name}");
            assert_eq!(selection.tests.contains_key(name), expected, "{os}/{name}");
        }
    }
}
