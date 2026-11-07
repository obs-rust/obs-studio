use super::*;
use obs_build::validation::graph::Graph;

#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
#[path = "v4l2_freetype_sdk_runtime.rs"]
mod runtime;

const CASES: [&str; 3] = [
    "test_v4l2_busy",
    "test_v4l2_udev_poll_error",
    "test_text_freetype2_platform",
];

fn requirement(name: &str) -> serde_json::Value {
    if name == CASES[2] {
        serde_json::json!({"all": [{"any": ["OS_LINUX", "OS_FREEBSD", "OS_OPENBSD"]}, "HAVE_FREETYPE_PLATFORM_SDK"]})
    } else {
        serde_json::json!({"all": ["OS_LINUX", "HAVE_OBS_V4L2", "HAVE_V4L2_HEADLESS_SDK"]})
    }
}

fn expected_target(name: &str) -> serde_json::Value {
    let text = name == CASES[2];
    let plugin = if text { "text-freetype2" } else { "linux-v4l2" };
    let mut target = serde_json::json!({
        "sources": [format!("test/cmocka/{name}.c")],
        "includes": [".", format!("plugins/{plugin}")],
        "watch_inputs": [format!("plugins/{plugin}/build/{plugin}-nix.pch.h")],
        "libraries": ["OBS::libobs", "OBS::native-test-cmocka"],
        "header_packages": if text { vec!["freetype2", "fontconfig"] } else { vec!["libv4l2", "libavcodec", "libavutil", "libudev"] },
        "requires": requirement(name)
    });
    if name == CASES[1] {
        target["libraries"] = serde_json::json!(["OBS::libobs", "Libudev::Libudev", "OBS::native-test-cmocka"]);
        target["watch_inputs"].as_array_mut().unwrap().push("plugins/linux-v4l2/v4l2-udev.c.h".into());
    } else {
        let source = if text { "text-freetype2-host-posix" } else { "v4l2-busy" };
        target["sources"].as_array_mut().unwrap().push(format!("plugins/{plugin}/{source}.c.h").into());
    }
    target
}

#[test]
fn canonical_v4l2_freetype_sdk_is_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let raw: serde_json::Value = serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap()).unwrap();
    for name in CASES {
        assert_eq!(raw["targets"][name], expected_target(name), "{name}");
        let mut test = serde_json::json!({"target": name, "requires": requirement(name)});
        if name != CASES[2] { test["timeout_seconds"] = 60.into(); }
        assert_eq!(raw["tests"][name], test, "{name}");
        let fragment = fs::read_to_string(root.join(format!("test/cmocka/tests.d/{name}.cmake"))).unwrap();
        assert!(!fragment.contains("add_executable("));
        assert!(!fragment.contains("add_test("));
    }
}

#[test]
fn canonical_v4l2_freetype_sdk_guards() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph.targets.retain(|name, _| CASES.contains(&name.as_str()));
    graph.tests.retain(|name, _| CASES.contains(&name.as_str()));
    assert_eq!(graph.targets.len(), CASES.len());
    for os in ["OS_LINUX", "OS_FREEBSD", "OS_OPENBSD", "OS_WINDOWS", "OS_MACOS"] {
        for owner in [false, true] {
            for sdk in [false, true] {
                let mut facts: std::collections::BTreeMap<String, bool> = ["OS_LINUX", "OS_FREEBSD", "OS_OPENBSD", "OS_WINDOWS", "OS_MACOS"].map(|name| (name.into(), name == os)).into();
                facts.extend([("HAVE_OBS_V4L2".into(), owner), ("HAVE_V4L2_HEADLESS_SDK".into(), sdk), ("HAVE_FREETYPE_PLATFORM_SDK".into(), sdk)]);
                let selected = graph.select(&facts).unwrap();
                let text = sdk && ["OS_LINUX", "OS_FREEBSD", "OS_OPENBSD"].contains(&os);
                let count = usize::from(text) + if sdk && owner && os == "OS_LINUX" { 2 } else { 0 };
                assert_eq!(selected.targets.len(), count);
                assert_eq!(selected.tests.len(), count);
            }
        }
    }
}
