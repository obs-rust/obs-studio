use super::*;

#[test]
fn canonical_canvas_release_is_registered() {
    use obs_build::validation::graph::{Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let name = "test_canvas_release";
    let target = &graph.targets[name];
    assert_eq!(target.sources.len(), 1);
    assert!(
        matches!(&target.sources[0], Source::Path(path) if path == "test/cmocka/test_canvas_release.c")
    );
    assert_eq!(graph.tests[name].target, name);
    assert_eq!(graph.tests[name].command, ["${target}"]);
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph
            .select(&[("OS_LINUX".into(), os == "OS_LINUX"), (os.into(), true)].into())
            .unwrap();
        assert_eq!(selected.targets.len(), 1);
        assert_eq!(selected.tests.len(), 1);
    }

    let extracted = root.join("libobs/obs-canvas-release.c.h");
    assert!(
        extracted.is_file(),
        "obs_canvas_release needs its own translation unit"
    );
    let old = fs::read_to_string(root.join("libobs/obs-canvas.c.h")).unwrap();
    assert!(!old.contains("void obs_canvas_release("));
    let group =
        fs::read_to_string(root.join("libobs/build/libobs.core-canvas-release+.h.c")).unwrap();
    assert!(group.contains("#include \"libobs/obs-canvas-release.c.h\""));
    let sources: Vec<String> = serde_json::from_str(
        &fs::read_to_string(root.join("libobs/build/source-sets.d/LIBOBS_SOURCES.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        sources
            .iter()
            .filter(|source| *source == "build/libobs.core-canvas-release+.h.c")
            .count(),
        1
    );
}
