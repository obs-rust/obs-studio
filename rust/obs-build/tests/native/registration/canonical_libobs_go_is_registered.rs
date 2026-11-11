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
    let raw: serde_json::Value = obs_build::validation::graph::Graph::read_value(
        &root.join("test/cmocka/build/test-graph.d"),
    )
    .unwrap();
    for name in NAMES {
        let target = &raw["targets"][name];
        assert!(!target.is_null(), "canonical graph omits {name}");
        assert_eq!(target["sources"][0], format!("test/cmocka/{name}.c"));
        assert_eq!(target["libraries"][0], "OBS::libobs");
        assert_eq!(target["libraries"][1], "OBS::native-test-cmocka");
        let test = &raw["tests"][name];
        assert_eq!(test["target"], name);
        assert_eq!(
            test["timeout_seconds"],
            if name == "test_ingest_refresh_wait" {
                serde_json::json!(60)
            } else {
                serde_json::Value::Null
            }
        );
        for field in ["optional_prefixes", "skip_return_code", "qt_runtime"] {
            assert!(test[field].is_null(), "unexpected {field}: {name}");
        }
    }
    assert_os_selection(&raw);
    assert_source_contracts(&root, &raw);
}

fn assert_os_selection(raw: &serde_json::Value) {
    let mut raw = raw.clone();
    for section in ["targets", "tests"] {
        raw[section]
            .as_object_mut()
            .unwrap()
            .retain(|name, _| NAMES.contains(&name.as_str()));
    }
    let graph = Graph::parse(&raw.to_string()).unwrap();
    for os in [
        "OS_LINUX",
        "OS_WINDOWS",
        "OS_MACOS",
        "OS_FREEBSD",
        "OS_OPENBSD",
    ] {
        let facts = [
            "OS_LINUX",
            "OS_WINDOWS",
            "OS_MACOS",
            "OS_FREEBSD",
            "OS_OPENBSD",
        ]
        .map(|fact| (fact.into(), fact == os))
        .into();
        let selection = graph.select(&facts).unwrap();
        for name in NAMES {
            let expected = match name {
                "test_ingest_refresh_wait" => true,
                "test_image_source_host" => os != "OS_WINDOWS" && os != "OS_MACOS",
                _ => os == "OS_LINUX",
            };
            assert_eq!(
                selection.targets.contains_key(name),
                expected,
                "{os}/{name}"
            );
            assert_eq!(selection.tests.contains_key(name), expected, "{os}/{name}");
        }
    }
}

fn assert_source_contracts(root: &Path, raw: &serde_json::Value) {
    for name in NAMES {
        let target = &raw["targets"][name];
        let source_count = if name == "test_nvenc_caps_cache" {
            3
        } else {
            1
        };
        assert_eq!(target["sources"].as_array().unwrap().len(), source_count);
        let libraries = target["libraries"].as_array().unwrap();
        assert_eq!(
            libraries.len(),
            if name == "test_ingest_refresh_wait" {
                3
            } else {
                2
            }
        );
        for field in [
            "header_libraries",
            "packages",
            "header_packages",
            "archives",
            "definitions",
            "compile_options",
            "link_options",
        ] {
            assert!(target[field].is_null(), "unexpected {field}: {name}");
        }
        let test = &raw["tests"][name];
        assert!(test["command"].is_null());
        let fragment =
            fs::read_to_string(root.join(format!("test/cmocka/tests.d/{name}.cmake"))).unwrap();
        assert!(!fragment.contains("add_executable("));
        assert!(!fragment.contains("add_test("));
    }
    assert_eq!(
        raw["targets"]["test_ingest_refresh_wait"]["libraries"][2],
        serde_json::json!({"name": "OBS::w32-pthreads", "requires": "OS_WINDOWS"})
    );
    assert_eq!(
        raw["targets"]["test_nvenc_caps_cache"]["sources"],
        serde_json::json!([
            "test/cmocka/test_nvenc_caps_cache.c",
            "plugins/obs-nvenc/nvenc-caps-cache.c.h",
            "plugins/obs-nvenc/nvenc-caps-cache-linux.c.h"
        ])
    );
}
