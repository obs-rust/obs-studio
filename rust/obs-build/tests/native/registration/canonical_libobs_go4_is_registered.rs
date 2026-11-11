use super::*;
use obs_build::validation::graph::Graph;

const NAMES: [&str; 2] = ["test_image_exif_orientation", "test_media_remux"];

#[test]
fn canonical_libobs_go4_is_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut raw: serde_json::Value = obs_build::validation::graph::Graph::read_value(
        &root.join("test/cmocka/build/test-graph.d"),
    )
    .unwrap();
    for name in NAMES {
        let mut libraries = vec!["OBS::libobs", "FFmpeg::avcodec"];
        if name == "test_media_remux" {
            libraries.push("FFmpeg::avformat");
        }
        libraries.extend(["FFmpeg::avutil", "OBS::native-test-cmocka"]);
        assert_eq!(
            raw["targets"][name],
            serde_json::json!({
                "sources": [format!("test/cmocka/{name}.c")],
                "libraries": libraries
            }),
            "canonical target differs: {name}"
        );
        assert_eq!(
            raw["tests"][name],
            serde_json::json!({
                "target": name, "suite": "core", "timeout_seconds": 60
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
    for os in [
        "OS_LINUX",
        "OS_WINDOWS",
        "OS_MACOS",
        "OS_FREEBSD",
        "OS_OPENBSD",
    ] {
        let facts = [(os.into(), true)].into();
        let selected = graph.select(&facts).unwrap();
        for name in NAMES {
            assert!(selected.targets.contains_key(name), "{os}/{name}");
            assert!(selected.tests.contains_key(name), "{os}/{name}");
        }
    }
}
