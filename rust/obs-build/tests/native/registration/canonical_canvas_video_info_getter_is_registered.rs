use super::*;

#[test]
fn canonical_canvas_video_info_getter_is_registered() {
    use obs_build::validation::graph::{Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let name = "test_canvas_video_info_getter";
    assert!(matches!(&graph.targets[name].sources[0], Source::Path(path) if path == "test/cmocka/test_canvas_video_info_getter.c"));
    assert_eq!(graph.tests[name].target, name);
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    let selected = graph.select(&[("OS_LINUX".into(), true), ("HAVE_OBS_OPENGL".into(), true)].into()).unwrap();
    assert_eq!(selected.targets.len(), 1);
    assert_eq!(selected.tests.len(), 1);
    assert!(selected.targets.contains_key(name));
}
