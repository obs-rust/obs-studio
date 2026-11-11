use super::*;

/// #3707: keep the unchanged shutdown-after-failed-startup regression discoverable.
#[test]
fn canonical_shutdown_after_failed_startup_is_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.d")).unwrap();
    graph.retain_suite("core");
    let name = "test_shutdown_after_failed_startup";
    let target = graph
        .targets
        .get(name)
        .expect("canonical graph omits shutdown after failed startup compatibility coverage");
    assert!(matches!(target.sources.as_slice(), [Source::Path(path)]
        if path == "test/cmocka/test_shutdown_after_failed_startup.c"));
    let libraries: Vec<_> = target
        .libraries
        .iter()
        .map(|library| match library {
            ConditionalValue::Name(name) => name.as_str(),
            _ => panic!("failed-startup shutdown linkage must be unconditional"),
        })
        .collect();
    assert_eq!(libraries, ["OBS::libobs", "OBS::native-test-cmocka"]);
    let test = &graph.tests[name];
    assert_eq!(test.target, name);
    assert_eq!(test.command, ["${target}"]);
    assert_eq!(test.working_directory.as_deref(), Some("${runtime}"));
    assert_eq!(test.timeout_seconds, Some(60));
    assert_eq!(test.skip_return_code, None);
    assert!(test.optional_prefixes.is_empty());
    let fragment = fs::read_to_string(
        root.join("test/cmocka/tests.d/test_shutdown_after_failed_startup.cmake"),
    )
    .unwrap();
    assert!(!fragment.contains("add_executable("));
    assert!(!fragment.contains("add_test("));
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for (os, expected) in [
        ("OS_LINUX", true),
        ("OS_MACOS", false),
        ("OS_WINDOWS", false),
    ] {
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
        assert_eq!(selected.targets.contains_key(name), expected, "{os}");
        assert_eq!(selected.tests.contains_key(name), expected, "{os}");
    }
}
