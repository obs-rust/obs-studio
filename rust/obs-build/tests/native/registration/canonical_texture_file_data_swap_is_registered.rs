use super::*;

#[test]
fn canonical_texture_file_data_swap_is_registered() {
    use obs_build::validation::graph::Graph;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let name = "test_texture_file_data_steps_rust";
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph
            .select(&[(os.into(), true), ("HAVE_LIBOBS_RUST_STATIC".into(), true)].into())
            .unwrap();
        assert_eq!(selected.tests[name].target, name);
        assert!(format!("{:?}", selected.targets[name]).contains("libobs_rust-static"));
    }
    let group =
        fs::read_to_string(root.join("libobs/build/libobs.ported-texture-file-data+.h.c")).unwrap();
    assert!(group.contains("libobs/graphics/graphics-texture-file-data.c.h"));
    let media = fs::read_to_string(root.join("libobs/build/libobs.media+.h.c")).unwrap();
    assert!(!media.contains("libobs/graphics/graphics-texture-file-data.c.h"));
}
