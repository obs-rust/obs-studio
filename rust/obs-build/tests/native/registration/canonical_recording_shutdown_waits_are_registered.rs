use super::*;

const CASES: [(&str, u32); 2] = [
    ("test_recording_shutdown_inactive", 15),
    ("test_recording_shutdown_wait", 60),
];

#[test]
fn canonical_recording_shutdown_waits_are_registered() {
    use obs_build::validation::graph::Graph;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let raw: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap())
            .unwrap();
    for (name, timeout) in CASES {
        let target = graph
            .targets
            .get(name)
            .unwrap_or_else(|| panic!("canonical graph omits {name}"));
        assert_eq!(
            raw["targets"][name]["sources"],
            serde_json::json!([format!("test/cmocka/{name}.cpp")])
        );
        assert_eq!(target.includes, ["frontend/widgets"]);
        assert_eq!(
            raw["targets"][name]["libraries"],
            serde_json::json!(["OBS::libobs", "Qt6::Core", "OBS::native-test-cmocka"])
        );
        assert!(target.definitions.is_empty());
        assert!(target.compile_options.is_empty());
        assert!(target.link_options.is_empty());
        let test = &graph.tests[name];
        assert_eq!(test.target, name);
        assert_eq!(test.suite, "qt");
        assert_eq!(test.command, ["@EXECUTABLE@"]);
        assert_eq!(test.timeout_seconds, Some(timeout));
        assert!(test.skip_return_code.is_none());
        assert!(test.environment.is_empty());
        assert!(test.resource_locks.is_empty());
    }
    graph
        .targets
        .retain(|name, _| CASES.iter().any(|case| case.0 == name));
    graph
        .tests
        .retain(|name, _| CASES.iter().any(|case| case.0 == name));
    for host in [
        "OS_LINUX",
        "OS_FREEBSD",
        "OS_OPENBSD",
        "OS_WINDOWS",
        "OS_MACOS",
    ] {
        for qt in [false, true] {
            let mut facts: std::collections::BTreeMap<String, bool> = [
                "OS_LINUX",
                "OS_FREEBSD",
                "OS_OPENBSD",
                "OS_WINDOWS",
                "OS_MACOS",
            ]
            .into_iter()
            .map(|os| (os.into(), os == host))
            .collect();
            facts.insert("HAVE_QT6_CORE".into(), qt);
            let selected = graph.select(&facts).unwrap();
            let count = if host == "OS_LINUX" && qt { 2 } else { 0 };
            assert_eq!(selected.targets.len(), count, "{host} Qt={qt}");
            assert_eq!(selected.tests.len(), count, "{host} Qt={qt}");
        }
    }
}
