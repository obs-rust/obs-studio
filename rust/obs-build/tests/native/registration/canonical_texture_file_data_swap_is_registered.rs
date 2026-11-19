use super::*;

#[test]
fn canonical_texture_file_data_swap_is_registered() {
    use obs_build::validation::graph::Graph;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let name = "test_texture_file_data_steps_rust";
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph
            .select(
                &[
                    (os.into(), true),
                    ("OS_FREEBSD".into(), false),
                    ("HAVE_LIBOBS_RUST_STATIC".into(), true),
                ]
                .into(),
            )
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

#[test]
fn standalone_texture_file_data_adapters_supply_private_max_size_callback() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let graph: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap())
            .unwrap();
    let adapter = "libobs/build/libobs.media-texture-file-data-adapter+.h.c";
    let callback = "test/cmocka/texture-file-data-max-size-link-stub.c";
    let mut consumers = 0;
    for (name, target) in graph["targets"].as_object().unwrap() {
        let sources = target["sources"].as_array().unwrap();
        if sources.contains(&serde_json::json!(adapter)) {
            consumers += 1;
            assert!(sources.contains(&serde_json::json!(callback)), "{name}");
        }
    }
    assert!(consumers > 0);
    let fragment = fs::read_to_string(
        root.join("test/cmocka/tests.d/texture_file_data_link_support.cmake"),
    )
    .unwrap();
    assert!(fragment.contains(callback));
}
