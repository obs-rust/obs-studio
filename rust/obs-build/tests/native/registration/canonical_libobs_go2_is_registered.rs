use super::*;
use obs_build::validation::graph::Graph;

const NAMES: [&str; 3] = [
    "test_image_file_version",
    "test_image_file_version_filetime",
    "test_oss_sndstat",
];

#[test]
fn canonical_libobs_go2_is_registered() {
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
        assert_eq!(raw["tests"][name], serde_json::json!({"target": name, "suite": "core"}));
        for field in ["packages", "header_packages", "archives", "definitions", "compile_options", "link_options"] {
            assert!(target[field].is_null(), "unexpected {field}: {name}");
        }
        let fragment = fs::read_to_string(root.join(format!("test/cmocka/tests.d/{name}.cmake"))).unwrap();
        assert!(!fragment.contains("add_executable("));
        assert!(!fragment.contains("add_test("));
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
        let facts = ["OS_LINUX", "OS_WINDOWS", "OS_MACOS", "OS_FREEBSD", "OS_OPENBSD"]
            .map(|fact| (fact.into(), fact == os)).into();
        let selection = graph.select(&facts).unwrap();
        for name in NAMES {
            let expected = match name {
                "test_image_file_version" => os != "OS_WINDOWS",
                "test_oss_sndstat" => os == "OS_LINUX",
                _ => true,
            };
            assert_eq!(selection.targets.contains_key(name), expected, "{os}/{name}");
            assert_eq!(selection.tests.contains_key(name), expected, "{os}/{name}");
        }
    }
}
