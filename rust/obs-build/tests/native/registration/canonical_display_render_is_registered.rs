use super::*;
use obs_build::validation::graph::Graph;
const CASES: [&str; 6] = [
    "test_darken_transparent",
    "test_image_size_precheck",
    "test_scaled_mix_alpha",
    "test_sprite_flip_render",
    "test_encoding_lag_rescale",
    "test_output_stop_without_packets",
];
fn expected() -> serde_json::Value {
    serde_json::from_str(include_str!("display_render_expected.json")).unwrap()
}
#[test]
fn canonical_display_render_is_registered() {
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
        assert!(!fragment.contains("add_executable("), "{name}");
        assert!(!fragment.contains("add_test("), "{name}");
    }
}
#[test]
fn canonical_display_render_guards() {
    let graph = Graph::parse(&expected().to_string()).unwrap();
    for os in ["OS_LINUX", "OS_WINDOWS", "OS_MACOS"] {
        for display in [None, Some(false), Some(true)] {
            for renderer in [false, true] {
                let mut facts = std::collections::BTreeMap::from([
                    ("OS_LINUX".into(), os == "OS_LINUX"),
                    ("HAVE_OBS_OPENGL".into(), renderer),
                ]);
                if let Some(display) = display {
                    facts.insert("HAVE_X11_DISPLAY".into(), display);
                }
                let selected = graph.select(&facts).unwrap();
                let enabled = os == "OS_LINUX" && display == Some(true) && renderer;
                assert_eq!(selected.targets.len(), if enabled { 6 } else { 0 });
                assert_eq!(selected.tests.len(), if enabled { 6 } else { 0 });
            }
        }
    }
}

#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
#[path = "display_render_runtime.rs"]
mod runtime;
