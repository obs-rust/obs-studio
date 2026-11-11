use super::*;

/// #3712: group mutations and teardown must run through both native consumers.
#[test]
fn canonical_group_lifecycle_is_registered() {
    use obs_build::validation::graph::Graph;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.d")).unwrap();
    let cases = [
        ("test_group_empty_bounds", true, Some(120)),
        ("test_group_active_refs", true, Some(120)),
        ("test_group_recursion", true, Some(180)),
        ("test_group_insert_empty", false, None),
        ("test_group_child_drift", true, Some(120)),
        ("test_group_teardown_uaf", false, None),
    ];
    for (name, graphics, timeout) in cases {
        assert_group_lifecycle_registration(&root, &graph, name, graphics, timeout);
    }
    graph
        .targets
        .retain(|name, _| cases.iter().any(|case| case.0 == name));
    graph
        .tests
        .retain(|name, _| cases.iter().any(|case| case.0 == name));
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        for opengl in [true, false] {
            let selected = graph
                .select(
                    &[
                        ("OS_LINUX".into(), os == "OS_LINUX"),
                        ("OS_MACOS".into(), os == "OS_MACOS"),
                        ("OS_WINDOWS".into(), os == "OS_WINDOWS"),
                        ("HAVE_OBS_OPENGL".into(), opengl),
                    ]
                    .into(),
                )
                .unwrap();
            for (name, graphics, _) in cases {
                let expected = if graphics {
                    os == "OS_LINUX" && opengl
                } else {
                    name == "test_group_teardown_uaf" || os != "OS_WINDOWS"
                };
                assert_eq!(
                    selected.targets.contains_key(name),
                    expected,
                    "{os}/{opengl}/{name}"
                );
                assert_eq!(
                    selected.tests.contains_key(name),
                    expected,
                    "{os}/{opengl}/{name}"
                );
                if expected && !graphics {
                    assert_eq!(
                        selected.tests[name].optional_prefixes.is_empty(),
                        os != "OS_LINUX"
                    );
                }
            }
        }
    }
}
