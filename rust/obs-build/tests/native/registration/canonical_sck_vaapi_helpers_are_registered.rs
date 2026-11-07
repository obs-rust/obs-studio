use super::*;

#[test]
fn canonical_sck_vaapi_helpers_are_registered() {
    use obs_build::validation::graph::Graph;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    for name in ["test_sck_stall", "test_sck_stop_wait", "test_vaapi_probe_policy"] {
        assert!(graph.targets.contains_key(name), "canonical graph omits {name}");
        assert!(graph.tests.contains_key(name), "canonical graph omits test {name}");
    }
}
