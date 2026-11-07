use super::*;

/// #1390: missing canonical registrations must fail the routine, Qt-free gate.
#[test]
fn canonical_frontend_product_and_meter_tests_are_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let graph =
        obs_build::validation::graph::Graph::load(&root.join("test/cmocka/build/test-graph.json"))
            .unwrap();
    for name in ["test_product_name", "test_volume_meter_snapshot"] {
        assert!(
            graph.targets.contains_key(name),
            "missing canonical target {name}"
        );
        assert_eq!(graph.tests[name].target, name);
    }
}
