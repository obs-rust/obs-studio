use super::*;

/// #1390: standalone plugin configuration helpers must reach both native consumers.
#[test]
fn canonical_plugin_configuration_helpers_are_registered() {
    use obs_build::validation::graph::Graph;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    for (name, include, headers) in [
        (
            "test_slideshow_size",
            "plugins/image-source",
            vec![
                "plugins/image-source/slideshow-size.h",
                "plugins/image-source/build/image-source.pch.h",
            ],
        ),
        (
            "test_stinger_monitoring_migration",
            "plugins/obs-transitions",
            vec!["plugins/obs-transitions/transition-stinger-monitoring.h"],
        ),
    ] {
        assert_plugin_configuration_target(&graph, name, include, &headers);
        assert_plugin_configuration_test(&root, &graph, name);
    }
}
