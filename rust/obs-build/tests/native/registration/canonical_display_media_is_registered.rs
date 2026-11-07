use super::*;
use obs_build::validation::graph::Graph;
const CASES: [&str; 6] = [
    "test_media_cover_art_seek",
    "test_media_gif_last_frame",
    "test_media_play_after_stop",
    "test_media_restart_first_frame",
    "test_media_restart_stale_frame",
    "test_v4l2_busy_signal",
];
fn expected() -> serde_json::Value {
    serde_json::from_str(include_str!("display_media_expected.json")).unwrap()
}
#[test]
fn canonical_display_media_is_registered() {
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
fn canonical_display_media_guards() {
    let graph = Graph::parse(&expected().to_string()).unwrap();
    for os in ["OS_LINUX", "OS_WINDOWS", "OS_MACOS"] {
        for display in [None, Some(false), Some(true)] {
            for renderer in [false, true] {
                for owners in [false, true] {
                    let mut facts = std::collections::BTreeMap::from([
                        ("OS_LINUX".into(), os == "OS_LINUX"),
                        ("HAVE_OBS_OPENGL".into(), renderer),
                        ("HAVE_MEDIA_PLAYBACK_TEST_OWNER".into(), owners),
                        ("HAVE_OBS_V4L2".into(), owners),
                    ]);
                    if let Some(value) = display {
                        facts.insert("HAVE_X11_DISPLAY".into(), value);
                    }
                    let selected = graph.select(&facts).unwrap();
                    for name in CASES {
                        let enabled = os == "OS_LINUX"
                            && display == Some(true)
                            && owners
                            && (name == "test_v4l2_busy_signal" || renderer);
                        assert_eq!(
                            selected.targets.contains_key(name),
                            enabled,
                            "{os}/{display:?}/{renderer}/{owners}/{name}"
                        );
                        assert_eq!(selected.tests.contains_key(name), enabled, "{name}");
                    }
                }
            }
        }
    }
}

#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
#[path = "display_media_runtime.rs"]
mod runtime;
