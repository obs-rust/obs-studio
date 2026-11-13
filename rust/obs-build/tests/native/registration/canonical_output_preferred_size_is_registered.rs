use super::*;

#[test]
fn canonical_output_preferred_size_is_registered() {
    use obs_build::validation::graph::{Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let name = "test_output_preferred_size";
    let target = &graph.targets[name];
    assert_eq!(target.sources.len(), 1);
    assert!(
        matches!(&target.sources[0], Source::Path(path) if path == "test/cmocka/test_output_preferred_size.c")
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
        assert!(selected.targets.contains_key(name));
        assert!(selected.tests.contains_key(name));
    }
    let extracted = root.join("libobs/obs-output-preferred-size.c.h");
    assert!(
        extracted.is_file(),
        "legacy wrapper must have its own C unit"
    );
    let group = fs::read_to_string(root.join("libobs/build/libobs.core-output+.h.c")).unwrap();
    assert!(group.contains("#include \"libobs/obs-output-preferred-size.c.h\""));
    let original = fs::read_to_string(root.join("libobs/obs-output.c.h")).unwrap();
    assert!(!original.contains("void obs_output_set_preferred_size("));
    assert!(original.contains("void obs_output_set_preferred_size2("));
}
