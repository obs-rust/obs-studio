use super::*;
use obs_build::validation::graph::Graph;

const CASES: [&str; 6] = [
    "test_pipewire_camera_control_refresh",
    "test_pipewire_camera_format_dedup",
    "test_pipewire_camera_nv12",
    "test_pipewire_camera_renegotiate",
    "test_pipewire_camera_resolution_restart",
    "test_pipewire_camera_unload",
];

#[test]
fn canonical_pipewire_cameras_is_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    for name in CASES {
        assert!(graph.targets.contains_key(name), "missing target {name}");
        assert!(graph.tests.contains_key(name), "missing test {name}");
        let fragment = fs::read_to_string(root.join(format!("test/cmocka/tests.d/{name}.cmake"))).unwrap();
        assert!(fragment.contains("test-graph.json"), "{name}");
        assert!(!fragment.contains("add_executable("), "{name}");
        assert!(!fragment.contains("add_test("), "{name}");
    }
}

#[test]
fn canonical_pipewire_cameras_optional_owner() {
    let graph = Graph::parse(r#"{"schema":1,"targets":{"camera":{"sources":["test.c"],"requires":"HAVE_PIPEWIRE_CAMERA_TEST_OWNER"}},"tests":{"camera":{"target":"camera","requires":"HAVE_PIPEWIRE_CAMERA_TEST_OWNER"}}}"#).unwrap();
    assert!(graph.select(&Default::default()).unwrap().targets.is_empty());
}
