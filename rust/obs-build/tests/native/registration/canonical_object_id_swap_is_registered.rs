use super::*;

#[test]
fn canonical_object_id_swap_is_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let graph =
        obs_build::validation::graph::Graph::load(&root.join("test/cmocka/build/test-graph.json"))
            .unwrap();
    let characterization = &graph.targets["test_object_id"];
    assert!(
        matches!(&characterization.sources[0], obs_build::validation::graph::Source::Path(p)
        if p == "test/cmocka/test_object_id.c")
    );
    let core = fs::read_to_string(root.join("libobs/build/libobs.core+.h.c")).unwrap();
    assert!(core.contains("\"libobs/obs-object-id-fields.c.h\""));
    assert!(!core.contains("\"libobs/obs-object-id.c.h\""));
    let unit: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(root.join("libobs/build/ported-source-sets/object-id.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        unit["LIBOBS_PORTED_SOURCES"],
        serde_json::json!(["build/libobs.ported-object-id+.h.c"])
    );
    let reference =
        fs::read_to_string(root.join("libobs/build/libobs.ported-object-id+.h.c")).unwrap();
    assert!(reference.contains("\"libobs/obs-object-id.c.h\""));
    // Reuse the shared demand-linked adapter archive and its existing native
    // registrations. New swaps must not invent another fixed consumer list.
    let support =
        fs::read_to_string(root.join("test/cmocka/video-info-query-link-support.c")).unwrap();
    assert!(support.contains("\"libobs/obs-object-id-fields.c.h\""));
}
