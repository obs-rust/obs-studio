use super::*;
use obs_build::validation::graph::Graph;
const CASES: [&str; 6] = [
    "test_async_audio_own_clock",
    "test_async_delay_audio",
    "test_gl_dynamic_texture",
    "test_nix_startup_log",
    "test_scene_size_race",
    "test_startup_alloc_stable",
];
fn expected() -> serde_json::Value {
    serde_json::from_str(include_str!("display_startup_expected.json")).unwrap()
}
#[test]
fn canonical_display_startup_is_registered() {
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
        assert!(!fragment.contains("add_executable("), "{name}");
        assert!(!fragment.contains("add_test("), "{name}");
        assert!(fragment.contains("test-graph.json"));
    }
}
#[test]
fn canonical_display_startup_guards() {
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
                for name in CASES {
                    let enabled = if name == "test_startup_alloc_stable" {
                        os != "OS_LINUX" || display == Some(true)
                    } else {
                        os == "OS_LINUX"
                            && display == Some(true)
                            && (renderer
                                || !name.starts_with("test_async_")
                                    && name != "test_gl_dynamic_texture")
                    };
                    assert_eq!(
                        selected.targets.contains_key(name),
                        enabled,
                        "{os}/{display:?}/{renderer}/{name}"
                    );
                    assert_eq!(
                        selected.tests.contains_key(name),
                        enabled,
                        "{os}/{display:?}/{renderer}/{name}"
                    );
                }
            }
        }
    }
    let typo = Graph::parse(
        r#"{"schema":1,"targets":{"test":{"sources":["test.c"],"requires":"HAVE_X11_DISPALY"}},"tests":{}}"#,
    )
    .unwrap();
    assert!(
        typo.select(&Default::default())
            .err()
            .unwrap()
            .contains("HAVE_X11_DISPALY")
    );
}

#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
#[path = "display_startup_runtime.rs"]
mod runtime;
