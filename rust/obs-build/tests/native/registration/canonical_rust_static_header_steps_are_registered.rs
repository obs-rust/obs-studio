use super::*;

const CASES: &[(&str, bool)] = &[
    ("test_async_upload_steps", true),
    ("test_deinterlace_draw_plan_steps", true),
    ("test_scene_render_plan_steps", true),
    ("test_source_audio_place_steps", false),
    ("test_source_draw_plan_steps", true),
    ("test_source_media_admission_steps", true),
    ("test_source_monitoring_steps", true),
    ("test_source_render_route_steps", true),
    ("test_source_video_tick_steps", false),
    ("test_transition_load_steps", false),
];

#[test]
fn canonical_rust_static_header_steps_are_registered() {
    use obs_build::validation::graph::{Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph
        .targets
        .retain(|key, _| CASES.iter().any(|(base, _)| key == &format!("{base}_rust")));
    graph
        .tests
        .retain(|key, _| CASES.iter().any(|(base, _)| key == &format!("{base}_rust")));
    for &(base, math) in CASES {
        let name = format!("{base}_rust");
        let target = graph
            .targets
            .get(&name)
            .expect("missing Rust-static header steps");
        assert!(matches!(target.sources.as_slice(), [Source::Path(path)]
            if path == &format!("test/cmocka/{base}.c")));
        assert_eq!(target.includes, ["libobs"]);
        assert_eq!(target.header_libraries, ["libobs", "SIMDe::SIMDe"]);
        assert!(target.archives.is_empty());
        assert!(target.definitions.is_empty());
        assert!(target.compile_options.is_empty());
        assert!(target.link_options.is_empty());
        assert_eq!(target.sanitizers, None);
        assert_header_step_linkage(target, math, base == "test_scene_render_plan_steps");
        let test = &graph.tests[&name];
        assert_eq!(test.target, name);
        assert_eq!(test.command, ["${target}"]);
        assert_eq!(test.timeout_seconds, None);
        assert_eq!(test.skip_return_code, None);
        let fragment =
            fs::read_to_string(root.join(format!("test/cmocka/tests.d/{base}.cmake"))).unwrap();
        assert!(!fragment.contains("add_executable("));
        assert!(!fragment.contains("add_test("));
        assert_header_step_selection(&graph, &name);
    }
}

fn assert_header_step_linkage(target: &obs_build::validation::graph::Target, math: bool, unix_math: bool) {
    use obs_build::validation::graph::{ConditionalValue, Requirement};
    assert_eq!(target.libraries.len(), if math { 3 } else { 2 });
    assert!(matches!(&target.libraries[0], ConditionalValue::Name(n) if n == "libobs_rust-static"));
    assert!(
        matches!(&target.libraries[1], ConditionalValue::Name(n) if n == "OBS::native-test-cmocka")
    );
    if unix_math {
        assert!(matches!(&target.libraries[2], ConditionalValue::Conditional { name, requires: Requirement::Fact(fact) }
            if name == "m" && fact == "UNIX"));
    } else if math {
        assert!(
            matches!(&target.libraries[2], ConditionalValue::Conditional { name, requires: Requirement::Not { not } }
                if name == "m" && matches!(not.as_ref(), Requirement::Fact(fact) if fact == "MSVC"))
        );
    }
}

fn assert_header_step_selection(graph: &obs_build::validation::graph::Graph, name: &str) {
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        for available in [false, true] {
            let facts = [
                (os.into(), true),
                ("MSVC".into(), os == "OS_WINDOWS"),
                ("UNIX".into(), os != "OS_WINDOWS"),
                ("HAVE_LIBOBS_RUST_STATIC".into(), available),
            ]
            .into();
            let selected = graph.select(&facts).unwrap();
            assert_eq!(selected.targets.contains_key(name), available);
            assert_eq!(selected.tests.contains_key(name), available);
        }
    }
}

