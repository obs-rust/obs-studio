use super::*;

/// #1390: preserve the original Linux/Core-only volume-meter compatibility test.
#[test]
fn canonical_volume_meter_ticker_is_registered() {
    use obs_build::validation::graph::Graph;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph.retain_suite("qt");
    let target = graph
        .targets
        .get("test_volume_meter_ticker")
        .expect("canonical graph omits test_volume_meter_ticker");
    assert_eq!(target.sources.len(), 1);
    assert_eq!(target.includes, ["frontend/components"]);
    assert_eq!(
        target.watch_inputs,
        ["frontend/components/VolumeMeterTicker.hpp"]
    );
    let test = &graph.tests["test_volume_meter_ticker"];
    assert_eq!(test.qt_runtime.as_deref(), Some("Qt6::Core"));
    assert_eq!(test.resource_locks, ["timing"]);
    for linux in [false, true] {
        for core in [false, true] {
            let facts = [
                ("OS_LINUX".into(), linux),
                ("OS_WINDOWS".into(), !linux),
                ("MSVC".into(), false),
                ("HAVE_QT6_CORE".into(), core),
                ("HAVE_QT6_WIDGETS".into(), false),
                ("HAVE_QT6_TEST".into(), false),
            ]
            .into();
            let selected = graph.select(&facts).unwrap();
            assert_eq!(
                selected.tests.contains_key("test_volume_meter_ticker"),
                linux && core
            );
        }
    }
}
