use super::*;

#[test]
fn canonical_type_catalog_enum_swap_is_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let graph =
        obs_build::validation::graph::Graph::load(&root.join("test/cmocka/build/test-graph.json"))
            .unwrap();
    let characterization = &graph.targets["test_type_catalog_enum"];
    assert!(
        matches!(&characterization.sources[0], obs_build::validation::graph::Source::Path(p)
        if p == "test/cmocka/test_type_catalog_enum.c")
    );
    let core = fs::read_to_string(root.join("libobs/build/libobs.core+.h.c")).unwrap();
    assert!(core.contains("obs-type-catalog-enum-fields.c.h"));
    assert!(!core.contains("\"libobs/obs-type-catalog-enum.c.h\""));
    let unit: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(root.join("libobs/build/ported-source-sets/type-catalog-enum.json"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        unit["LIBOBS_PORTED_SOURCES"],
        serde_json::json!(["build/libobs.ported-type-catalog-enum+.h.c"])
    );
    let reference =
        fs::read_to_string(root.join("libobs/build/libobs.ported-type-catalog-enum+.h.c")).unwrap();
    assert!(reference.contains("\"libobs/obs-type-catalog-enum.c.h\""));
    // #5190's shared archive and canonical graph already consume this real
    // wrapper. Adding a port must not introduce another fixed consumer list.
    let support =
        fs::read_to_string(root.join("test/cmocka/video-info-query-link-support.c")).unwrap();
    assert!(support.contains("\"libobs/obs-type-catalog-enum-fields.c.h\""));
}
