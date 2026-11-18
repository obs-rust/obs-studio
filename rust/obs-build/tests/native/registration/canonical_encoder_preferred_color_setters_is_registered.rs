use super::*;

#[test]
fn canonical_encoder_preferred_color_setters_is_registered() {
    use obs_build::validation::graph::{Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let name = "test_encoder_preferred_color_setters";
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph
            .select(
                &["OS_LINUX", "OS_MACOS", "OS_WINDOWS"]
                    .into_iter()
                    .map(|fact| (fact.into(), fact == os))
                    .collect(),
            )
            .unwrap();
        assert!(
            matches!(selected.targets[name].sources.as_slice(), [Source::Path(path)]
            if path == "test/cmocka/test_encoder_preferred_color_setters.c")
        );
        assert_eq!(selected.tests[name].target, name);
    }

    // The common sets feed both CMake and Cargo on every host. The C body
    // belongs only to the reference; production retains the real field adapter.
    let ported = fs::read_to_string(
        root.join("libobs/build/ported-source-sets/encoder-preferred-color-setters.json"),
    )
    .unwrap();
    let ported: serde_json::Value = serde_json::from_str(&ported).unwrap();
    assert_eq!(
        ported["LIBOBS_PORTED_SOURCES"],
        serde_json::json!(["build/libobs.ported-encoder-preferred-color-setters+.h.c"])
    );
    let common =
        fs::read_to_string(root.join("libobs/build/source-sets.d/LIBOBS_SOURCES.json")).unwrap();
    assert!(common.contains("build/libobs.core-output+.h.c"));
    assert!(!common.contains("build/libobs.core-encoder-preferred-color-setters+.h.c"));
    let group = fs::read_to_string(
        root.join("libobs/build/libobs.ported-encoder-preferred-color-setters+.h.c"),
    )
    .unwrap();
    assert!(group.contains("#include \"libobs/obs-encoder-preferred-color-setters.c.h\""));
    let hybrid = fs::read_to_string(root.join("libobs/build/libobs.core-output+.h.c")).unwrap();
    assert!(!hybrid.contains("#include \"libobs/obs-encoder-preferred-color-setters.c.h\""));
    assert!(hybrid.contains("#include \"libobs/obs-encoder-preferred-color-setters-fields.c.h\""));
}
