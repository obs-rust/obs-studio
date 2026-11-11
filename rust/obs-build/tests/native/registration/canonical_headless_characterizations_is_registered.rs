use super::*;
use obs_build::validation::graph::Graph;
const CASES: [&str; 6] = [
    "test_wayland_hotkeys",
    "test_service_state",
    "test_lua_graphics_lock",
    "test_lua_graphics_lock_bounded",
    "test_vcam_audio_config",
    "test_virtual_cam_shutdown",
];
#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
#[path = "support/headless_characterizations_runtime.rs"]
mod runtime;
fn expected() -> serde_json::Value {
    serde_json::from_str(include_str!("headless_characterizations_expected.json")).unwrap()
}
#[test]
fn canonical_headless_characterizations_is_registered() {
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
    }
}
#[test]
fn canonical_headless_characterizations_guards() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.d")).unwrap();
    graph.targets.retain(|n, _| CASES.contains(&n.as_str()));
    graph.tests.retain(|n, _| CASES.contains(&n.as_str()));
    assert_eq!(graph.targets.len(), 6);
    for os in [
        "OS_LINUX",
        "OS_FREEBSD",
        "OS_OPENBSD",
        "OS_WINDOWS",
        "OS_MACOS",
    ] {
        for owner in [false, true] {
            let mut facts: std::collections::BTreeMap<String, bool> = [
                "OS_LINUX",
                "OS_FREEBSD",
                "OS_OPENBSD",
                "OS_WINDOWS",
                "OS_MACOS",
            ]
            .into_iter()
            .map(|n| (n.into(), n == os))
            .collect();
            facts.insert("HAVE_FRONTEND_PLAIN_TEST_OWNER".into(), owner);
            let selected = graph.select(&facts).unwrap();
            let count =
                3 + usize::from(["OS_LINUX", "OS_FREEBSD"].contains(&os)) + 2 * usize::from(owner);
            assert_eq!(selected.targets.len(), count, "{os}/{owner}");
            assert_eq!(selected.tests.len(), count);
        }
    }
}
