use super::*;

/// #1390: unchanged process-pipe contracts retain their real core dependency.
#[test]
fn canonical_pipe_contracts_are_registered() {
    use obs_build::validation::graph::Graph;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.d")).unwrap();
    let cases = [
        ("test_pipe_create2_exact_path", Some(20), true, true),
        ("test_pipe_no_handle_leak", Some(30), false, true),
        ("test_pipe_shell_argv0", Some(20), true, true),
        ("test_pipe_stdio_inherit", None, true, false),
    ];
    for (name, timeout, _, _) in cases {
        assert_pipe_contract_registration(&graph, &root, name, timeout);
    }

    graph
        .targets
        .retain(|name, _| cases.iter().any(|case| case.0 == name));
    graph
        .tests
        .retain(|name, _| cases.iter().any(|case| case.0 == name));
    for unix in [false, true] {
        for available in [false, true] {
            let facts = [
                ("UNIX".into(), unix),
                ("HAVE_LIBOBS".into(), available),
                ("OS_WINDOWS".into(), !unix),
            ]
            .into();
            let selected = graph.select(&facts).unwrap();
            for (name, _, needs_unix, needs_core_guard) in cases {
                let enabled = (!needs_unix || unix) && (!needs_core_guard || available);
                assert_eq!(
                    selected.targets.contains_key(name),
                    enabled,
                    "{name} Unix={unix} core={available}"
                );
                assert_eq!(selected.tests.contains_key(name), enabled, "{name}");
            }
        }
    }
}
