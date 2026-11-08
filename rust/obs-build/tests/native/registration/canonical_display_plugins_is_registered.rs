use super::*;
use obs_build::validation::graph::Graph;
const CASES: [&str; 5] = [
    "test_color_correction_16f",
    "test_rtmps_upgrade",
    "test_scroll_filter_wide_source",
    "test_slideshow_manual_end",
    "test_x11_volume_keys",
];
fn expected() -> serde_json::Value {
    serde_json::from_str(include_str!("display_plugins_expected.json")).unwrap()
}
#[test]
fn canonical_display_plugins_is_registered() {
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
fn canonical_display_plugins_guards() {
    let graph = Graph::parse(&expected().to_string()).unwrap();
    for os in ["OS_LINUX", "OS_WINDOWS", "OS_MACOS"] {
        for display in [None, Some(false), Some(true)] {
            for renderer in [false, true] {
                for capabilities in 0..16 {
                    let x11 = capabilities & 1 != 0;
                    let filters = capabilities & 2 != 0;
                    let owners = capabilities & 4 != 0;
                    let qt = capabilities & 8 != 0;
                    let mut facts = std::collections::BTreeMap::from([
                        ("OS_LINUX".into(), os == "OS_LINUX"),
                        ("OS_WINDOWS".into(), os == "OS_WINDOWS"),
                        ("HAVE_STARTUP_X11".into(), x11),
                        ("HAVE_OBS_FILTERS".into(), filters),
                        ("HAVE_RTMP_SERVICES_MODULE".into(), owners),
                        ("HAVE_QT6_WIDGETS".into(), qt),
                        ("HAVE_OBS_OPENGL".into(), renderer),
                    ]);
                    if let Some(display) = display {
                        facts.insert("HAVE_X11_DISPLAY".into(), display);
                    }
                    let selected = graph.select(&facts).unwrap();
                    for name in CASES {
                        let visible = os == "OS_LINUX" && display == Some(true);
                        let enabled = match name {
                            "test_rtmps_upgrade" => visible && owners,
                            "test_x11_volume_keys" => visible && x11,
                            "test_color_correction_16f" | "test_scroll_filter_wide_source" => {
                                visible && renderer && filters
                            }
                            _ => visible && renderer,
                        };
                        assert_eq!(
                            selected.targets.contains_key(name),
                            enabled,
                            "{os}/{display:?}/{renderer}/{capabilities}/{name}"
                        );
                        assert_eq!(
                            selected.tests.contains_key(name),
                            enabled,
                            "{os}/{display:?}/{renderer}/{capabilities}/{name}"
                        );
                    }
                }
            }
        }
    }
}

#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
#[path = "display_plugins_runtime.rs"]
mod runtime;
