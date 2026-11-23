use super::*;

/// #1390: scalar audio filters need real libobs and strict floating-point math.
#[test]
fn canonical_audio_filter_scalars_are_registered() {
    use obs_build::validation::graph::Graph;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let raw: serde_json::Value = obs_build::validation::graph::Graph::read_value(
        &root.join("test/cmocka/build/test-graph.d"),
    )
    .unwrap();
    let names = ["test_dynamics_threshold", "test_expander_unity"];
    for name in names {
        let target = raw["targets"]
            .get(name)
            .expect("missing scalar audio filter");
        assert_eq!(
            target["sources"],
            serde_json::json!([format!("test/cmocka/{name}.c")])
        );
        assert_eq!(target["requires"], true);
        assert_eq!(
            target["libraries"],
            serde_json::json!([
                "OBS::libobs", {"name": "m", "requires": {"not": "MSVC"}},
                {"name": "OBS::w32-pthreads", "requires": "OS_WINDOWS"}
            ])
        );
        assert_eq!(
            target["compile_options"],
            serde_json::json!([
                {"name": "/fp:strict", "requires": "MSVC"},
                {"name": "-fno-fast-math", "requires": {"not": "MSVC"}},
                {"name": "-ffp-contract=off", "requires": {"not": "MSVC"}}
            ])
        );
        assert_eq!(
            target["watch_inputs"],
            serde_json::json!(["plugins/obs-filters"])
        );
        let fragment =
            fs::read_to_string(root.join(format!("test/cmocka/tests.d/{name}.cmake"))).unwrap();
        assert!(!fragment.contains("add_executable("));
        assert!(!fragment.contains("add_test("));
    }
    let mut graph = Graph::parse(&raw.to_string()).unwrap();
    graph
        .targets
        .retain(|name, _| names.contains(&name.as_str()));
    graph.tests.retain(|name, _| names.contains(&name.as_str()));
    for os in [
        "OS_LINUX",
        "OS_MACOS",
        "OS_WINDOWS",
        "OS_FREEBSD",
        "OS_OPENBSD",
    ] {
        let facts = [
            (os.into(), true),
            ("MSVC".into(), os == "OS_WINDOWS"),
            ("OS_WINDOWS".into(), os == "OS_WINDOWS"),
        ]
        .into();
        let selected = graph.select(&facts).unwrap();
        assert_eq!(selected.targets.len(), names.len());
        assert_eq!(selected.tests.len(), names.len());
        for name in names {
            assert_scalar_audio_execution(&graph.tests[name], name);
        }
    }
}

fn assert_scalar_audio_execution(test: &obs_build::validation::graph::Test, name: &str) {
    assert_eq!(test.target, name);
    assert_eq!(test.command, ["${target}"]);
    assert_eq!(test.timeout_seconds, None);
    assert_eq!(test.skip_return_code, None);
    assert!(test.optional_prefixes.is_empty());
    assert_eq!(test.qt_runtime, None);
}
