use super::*;
use obs_build::validation::graph::Graph;
const CASES: [&str; 6] = ["obs_runtime_bench", "test_media_cover_art_monitor", "test_media_sync_offset_restart", "test_screencast_portal_teardown", "test_stinger_media_duration", "test_xshm_capture_live"];
fn expected() -> serde_json::Value {
    serde_json::from_str(include_str!("display_live_expected.json")).unwrap()
}
#[test]
fn canonical_display_live_is_registered() {
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
        let fragment_name = if name == "test_xshm_capture_live" { "test_xshm_capture_pipeline" } else {name};
        let fragment =
            fs::read_to_string(root.join(format!("test/cmocka/tests.d/{fragment_name}.cmake"))).unwrap();
        assert!(fragment.contains("test-graph.json"));
        assert!(!fragment.contains("add_executable("), "{name}");
        assert!(!fragment.contains("add_test("), "{name}");
    }
}
#[test]
fn canonical_display_live_guards() {
    let graph = Graph::parse(&expected().to_string()).unwrap();
    for os in ["OS_LINUX", "OS_WINDOWS", "OS_MACOS"] {
        for display in [None, Some(false), Some(true)] {
            for renderer in [false, true] {
                for owners in [false, true] {
                    let mut facts = std::collections::BTreeMap::from([
                        ("OS_LINUX".into(), os == "OS_LINUX"),
                        ("HAVE_OBS_OPENGL".into(), renderer),
                        ("HAVE_MEDIA_PLAYBACK_TEST_OWNER".into(), owners),
                        ("HAVE_TRANSITIONS_TEST_OWNER".into(), owners),
                        ("HAVE_OBS_PIPEWIRE".into(), owners),
                        ("HAVE_LINUX_CAPTURE".into(), owners),
                    ]);
                    if let Some(value) = display { facts.insert("HAVE_X11_DISPLAY".into(), value); }
                    let selected = graph.select(&facts).unwrap();
                    for name in CASES {
                        let enabled = os == "OS_LINUX" && display == Some(true)
                            && (owners || name == "obs_runtime_bench")
                            && (renderer || name == "test_screencast_portal_teardown");
                        assert_eq!(selected.targets.contains_key(name), enabled, "{os}/{display:?}/{renderer}/{owners}/{name}");
                    }
                    for (name, test) in &graph.tests {
                        assert_eq!(selected.tests.contains_key(name.as_str()), selected.targets.contains_key(test.target.as_str()), "{name}");
                    }
                }
            }
        }
    }
}

