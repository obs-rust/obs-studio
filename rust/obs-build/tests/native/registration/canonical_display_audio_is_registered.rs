use super::*;
use obs_build::validation::graph::Graph;
const CASES: [&str; 6] = [
    "test_async_delay_audio",
    "test_encoder_error_stop",
    "test_encoder_stop_packet_times",
    "test_obs_startup_failure_diagnostics",
    "test_scene_audio_actions",
    "test_shutdown_leaked_scenes",
];
fn expected() -> serde_json::Value {
    serde_json::from_str(include_str!("display_audio_expected.json")).unwrap()
}
#[test]
fn canonical_display_audio_is_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let raw: serde_json::Value = obs_build::validation::graph::Graph::read_value(
        &root.join("test/cmocka/build/test-graph.d"),
    )
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
        assert!(fragment.contains("test-graph.d"));
        assert!(!fragment.contains("add_executable("), "{name}");
        assert!(!fragment.contains("add_test("), "{name}");
    }
}
#[test]
fn canonical_display_audio_guards() {
    let graph = Graph::parse(&expected().to_string()).unwrap();
    for os in ["OS_LINUX", "OS_WINDOWS", "OS_MACOS"] {
        for display in [None, Some(false), Some(true)] {
            for renderer in [false, true] {
                for x11 in [false, true] {
                    let mut facts = std::collections::BTreeMap::from([
                        ("OS_LINUX".into(), os == "OS_LINUX"),
                        ("OS_WINDOWS".into(), os == "OS_WINDOWS"),
                        ("MSVC".into(), os == "OS_WINDOWS"),
                        ("HAVE_STARTUP_X11".into(), x11),
                        ("HAVE_OBS_OPENGL".into(), renderer),
                    ]);
                    if let Some(display) = display {
                        facts.insert("HAVE_X11_DISPLAY".into(), display);
                    }
                    let selected = graph.select(&facts).unwrap();
                    for name in CASES {
                        let enabled = match name {
                            "test_shutdown_leaked_scenes" => {
                                os != "OS_LINUX" || display == Some(true)
                            }
                            "test_obs_startup_failure_diagnostics" => os == "OS_LINUX" && x11,
                            _ => os == "OS_LINUX" && display == Some(true) && renderer,
                        };
                        assert_eq!(
                            selected.targets.contains_key(name),
                            enabled,
                            "{os}/{display:?}/{renderer}/{x11}/{name}"
                        );
                        assert_eq!(
                            selected.tests.contains_key(name),
                            enabled,
                            "{os}/{display:?}/{renderer}/{x11}/{name}"
                        );
                    }
                }
            }
        }
    }
}

#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
#[path = "support/display_audio_runtime.rs"]
mod runtime;
