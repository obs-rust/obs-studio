use super::*;

#[test]
fn canonical_frontend_utility_tests_are_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let graph =
        obs_build::validation::graph::Graph::load(&root.join("test/cmocka/build/test-graph.d"))
            .unwrap();
    for name in [
        "test_stream_service_select",
        "test_saved_transition",
        "test_record_disk_space",
        "test_multitrack_ladder",
        "test_multitrack_encoder_override",
    ] {
        assert!(
            graph.targets.contains_key(name),
            "missing canonical target {name}"
        );
        assert_eq!(graph.tests[name].target, name);
        assert_eq!(graph.targets[name].watch_inputs, ["frontend/utility"]);
    }
}
