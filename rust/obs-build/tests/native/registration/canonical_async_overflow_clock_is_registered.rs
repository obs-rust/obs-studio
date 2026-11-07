use super::*;

/// #3720: async cache overflow clocks must reach both native consumers.
#[test]
fn canonical_async_overflow_clock_is_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph.retain_suite("core");
    let name = "test_async_overflow_clock";
    let target = graph
        .targets
        .get(name)
        .expect("canonical graph omits async overflow clock coverage");
    assert!(matches!(target.sources.as_slice(), [Source::Path(path)]
        if path == "test/cmocka/test_async_overflow_clock.c"));
    let libraries: Vec<_> = target
        .libraries
        .iter()
        .map(|library| match library {
            ConditionalValue::Name(name) => name.as_str(),
            _ => panic!("async overflow linkage must be unconditional"),
        })
        .collect();
    assert_eq!(
        libraries,
        ["OBS::libobs", "OBS::caption", "OBS::native-test-cmocka"]
    );
    assert!(target.header_libraries.is_empty());
    let test = &graph.tests[name];
    assert_eq!(test.target, name);
    assert_eq!(test.command, ["${target}"]);
    assert_eq!(test.suite, "core");
    assert_eq!(test.working_directory, None);
    assert_eq!(test.timeout_seconds, None);
    assert_eq!(test.skip_return_code, None);
    assert!(test.optional_prefixes.is_empty());
    let fragment =
        fs::read_to_string(root.join("test/cmocka/tests.d/test_async_overflow_clock.cmake"))
            .unwrap();
    assert!(!fragment.contains("add_executable("));
    assert!(!fragment.contains("add_test("));
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph
            .select(
                &[
                    ("OS_LINUX".into(), os == "OS_LINUX"),
                    ("OS_MACOS".into(), os == "OS_MACOS"),
                    ("OS_WINDOWS".into(), os == "OS_WINDOWS"),
                ]
                .into(),
            )
            .unwrap();
        assert_eq!(
            selected.targets.contains_key(name),
            os == "OS_LINUX",
            "{os}"
        );
        assert_eq!(selected.tests.contains_key(name), os == "OS_LINUX", "{os}");
    }
}
