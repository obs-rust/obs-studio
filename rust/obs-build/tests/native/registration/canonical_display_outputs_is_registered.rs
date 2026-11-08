use super::*;
use obs_build::validation::graph::Graph;
const CASES: [&str; 6] = [
    "test_ffmpeg_mux_unwritable_path",
    "test_ffmpeg_output_alpha",
    "test_ffmpeg_output_drain",
    "test_ffmpeg_output_stop",
    "test_pulse_monitor_reset",
    "test_slideshow_empty",
];
fn expected() -> serde_json::Value {
    serde_json::from_str(include_str!("display_outputs_expected.json")).unwrap()
}
#[test]
fn canonical_display_outputs_is_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let raw: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap())
            .unwrap();
    let expected = expected();
    for section in ["targets", "tests"] {
        for (name, record) in expected[section].as_object().unwrap() {
            assert_eq!(raw[section][name], *record, "{section}/{name}");
        }
    }
    for name in CASES {
        let fragment =
            fs::read_to_string(root.join(format!("test/cmocka/tests.d/{name}.cmake"))).unwrap();
        assert!(fragment.contains("test-graph.json"));
        assert!(!fragment.contains("add_executable("), "{name}");
        assert!(!fragment.contains("add_test("), "{name}");
    }
}
#[test]
fn canonical_display_outputs_guards() {
    let graph = Graph::parse(&expected().to_string()).unwrap();
    for os in ["OS_LINUX", "OS_WINDOWS", "OS_MACOS"] {
        for display in [None, Some(false), Some(true)] {
            for renderer in [false, true] {
                for pulse in [false, true] {
                    for owners in [false, true] {
                        let mut facts = std::collections::BTreeMap::from([
                            ("OS_LINUX".into(), os == "OS_LINUX"),
                            ("OS_WINDOWS".into(), os == "OS_WINDOWS"),
                            ("OS_MACOS".into(), os == "OS_MACOS"),
                            ("HAVE_OBS_OPENGL".into(), renderer),
                            ("ENABLE_PULSEAUDIO".into(), pulse),
                            ("HAVE_SLIDESHOW_TEST_OWNER".into(), owners),
                        ]);
                        if let Some(value) = display {
                            facts.insert("HAVE_X11_DISPLAY".into(), value);
                        }
                        let selected = graph.select(&facts).unwrap();
                        for name in CASES {
                            let enabled = match name {
                                "test_slideshow_empty" => {
                                    owners && (os != "OS_LINUX" || display == Some(true))
                                }
                                "test_pulse_monitor_reset" => {
                                    os == "OS_LINUX" && display == Some(true) && pulse
                                }
                                _ => os == "OS_LINUX" && display == Some(true) && renderer,
                            };
                            assert_eq!(
                                selected.targets.contains_key(name),
                                enabled,
                                "{os}/{display:?}/{renderer}/{pulse}/{owners}/{name}"
                            );
                            for (test_name, spec) in &graph.tests {
                                if spec.target == name {
                                    assert_eq!(
                                        selected.tests.contains_key(test_name.as_str()),
                                        enabled,
                                        "{test_name}"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
#[path = "display_outputs_runtime.rs"]
mod runtime;
