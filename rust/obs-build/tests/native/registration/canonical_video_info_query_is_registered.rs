use super::*;

#[test]
fn canonical_video_info_query_is_registered() {
    use obs_build::validation::graph::{Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let c = &graph.targets["test_video_info_query"];
    let rust = &graph.targets["test_video_info_query_rust"];
    assert!(
        matches!(&c.sources[0], Source::Path(path) if path == "test/cmocka/test_video_info_query.c")
    );
    assert!(
        matches!(&rust.sources[0], Source::Path(path) if path == "test/cmocka/test_video_info_query.c")
    );
    assert!(
        matches!(&c.sources[1], Source::Path(path) if path == "libobs/obs-video-info-query.c.h")
    );
    assert!(
        matches!(&rust.sources[1], Source::Path(path) if path == "test/cmocka/video-info-query-link-support.c")
    );
    graph
        .targets
        .retain(|name, _| name.starts_with("test_video_info_query"));
    graph
        .tests
        .retain(|name, _| name.starts_with("test_video_info_query"));
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph
            .select(&[(os.into(), true), ("HAVE_LIBOBS_RUST_STATIC".into(), true)].into())
            .unwrap();
        for name in ["test_video_info_query", "test_video_info_query_rust"] {
            assert!(selected.targets.contains_key(name), "{os}: {name}");
            assert!(selected.tests.contains_key(name), "{os}: {name}");
        }
        assert!(
            selected.targets["test_video_info_query_rust"]
                .libraries
                .iter()
                .any(|lib| matches!(lib, obs_build::validation::graph::ConditionalValue::Name(name) if name == "libobs_rust-static"))
        );
    }
    let group = fs::read_to_string(root.join("libobs/build/libobs.core+.h.c")).unwrap();
    assert!(group.contains("obs-video-info-query-fields.c.h"));
    assert!(!group.contains("\"libobs/obs-video-info-query.c.h\""));
}
