use super::*;

/// #1390: preserve both unchanged frontend-tools host helpers and their OS guard.
#[test]
fn canonical_frontend_tools_hosts_are_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let names = ["test_frontend_tools_host", "test_frontend_tools_host_macos"];
    for name in names {
        let target = graph
            .targets
            .get(name)
            .unwrap_or_else(|| panic!("canonical graph omits {name}"));
        assert!(
            matches!(target.sources.as_slice(), [Source::Path(path)] if path == &format!("test/cmocka/{name}.c"))
        );
        assert_eq!(target.includes, ["."]);
        assert!(
            matches!(target.libraries.as_slice(), [ConditionalValue::Name(name)] if name == "OBS::native-test-cmocka")
        );
        assert!(target.header_libraries.is_empty());
        let host = if name.ends_with("macos") {
            "macos"
        } else {
            "posix"
        };
        assert_eq!(
            target.watch_inputs,
            [
                format!("plugins/frontend-tools/frontend-tools-host-{host}.c.h"),
                "plugins/frontend-tools/frontend-tools-host.h".into(),
                format!("plugins/frontend-tools/build/frontend-tools-host-{host}.pch.h"),
            ]
        );
        let test = &graph.tests[name];
        assert_eq!(test.target, name);
        assert_eq!(test.command, ["${target}"]);
        assert_eq!(test.suite, "core");
        assert_eq!(test.timeout_seconds, None);
        assert_eq!(test.skip_return_code, None);
        assert!(test.optional_prefixes.is_empty());
        assert_eq!(test.qt_runtime, None);
    }
    let fragment =
        fs::read_to_string(root.join("test/cmocka/tests.d/test_frontend_tools_host.cmake"))
            .unwrap();
    assert!(!fragment.contains("add_executable("));
    assert!(!fragment.contains("add_test("));
    graph
        .targets
        .retain(|name, _| names.contains(&name.as_str()));
    graph.tests.retain(|name, _| names.contains(&name.as_str()));
    for (windows, linux, macos) in [
        (false, false, false),
        (false, false, true),
        (false, true, false),
        (false, true, true),
        (true, false, false),
        (true, false, true),
        (true, true, false),
        (true, true, true),
    ] {
        let facts = [
            ("OS_WINDOWS".into(), windows),
            ("OS_LINUX".into(), linux),
            ("OS_MACOS".into(), macos),
        ]
        .into();
        let selected = graph.select(&facts).unwrap();
        for name in names {
            assert_eq!(selected.targets.contains_key(name), !windows);
            assert_eq!(selected.tests.contains_key(name), !windows);
        }
    }
}
