use super::*;

/// #3718: fake-device graphics C suites must reach both native consumers.
#[test]
fn canonical_fake_device_graphics_is_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.d")).unwrap();
    graph.retain_suite("core");
    let cases = [
        ("test_texture_render", None),
        ("test_graphics_immediate_render", Some("address,undefined")),
    ];
    for (name, sanitizers) in cases {
        let target = graph.targets.get(name).unwrap_or_else(|| {
            panic!("canonical graph omits fake-device graphics coverage: {name}")
        });
        assert!(matches!(target.sources.as_slice(), [Source::Path(path)]
            if path == &format!("test/cmocka/{name}.c")));
        let libraries: Vec<_> = target
            .libraries
            .iter()
            .map(|library| match library {
                ConditionalValue::Name(name) => name.as_str(),
                _ => panic!("fake-device graphics linkage must be unconditional"),
            })
            .collect();
        assert_eq!(libraries, ["OBS::libobs", "OBS::native-test-cmocka"]);
        assert_eq!(target.sanitizers.as_deref(), sanitizers);
        assert!(target.artifact_definitions.is_empty());
        let test = &graph.tests[name];
        assert_eq!(test.target, name);
        assert_eq!(test.command, ["${target}"]);
        assert_eq!(test.suite, "core");
        assert_eq!(test.working_directory, None);
        assert_eq!(test.timeout_seconds, None);
        assert_eq!(test.skip_return_code, None);
        assert!(test.optional_prefixes.is_empty());
        let fragment =
            fs::read_to_string(root.join(format!("test/cmocka/tests.d/{name}.cmake"))).unwrap();
        assert!(!fragment.contains("add_executable("), "{name}");
        assert!(!fragment.contains("add_test("), "{name}");
    }
    graph
        .targets
        .retain(|name, _| cases.iter().any(|case| case.0 == name));
    graph
        .tests
        .retain(|name, _| cases.iter().any(|case| case.0 == name));
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph.select(&[(os.into(), true)].into()).unwrap();
        for (name, _) in cases {
            assert!(selected.targets.contains_key(name), "{os}/{name}");
            assert!(selected.tests.contains_key(name), "{os}/{name}");
        }
    }
}
