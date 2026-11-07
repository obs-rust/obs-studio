use super::*;

/// #1390: preserve the standalone fade and host tests, including exact OS guards.
#[test]
fn canonical_fade_and_host_tests_are_registered() {
    use obs_build::validation::graph::Graph;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let raw: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap())
            .unwrap();
    let names = [
        "test_fade_to_color_mix",
        "test_ffmpeg_mux_helper_host",
        "test_frontend_obs_main_host",
        "test_frontend_obsbasic_host",
        "test_frontend_settings_host",
    ];
    let mut graph = Graph::parse(&raw.to_string()).unwrap();
    for name in names {
        let target = raw["targets"]
            .get(name)
            .unwrap_or_else(|| panic!("canonical graph omits {name}"));
        let guard = if name == names[0] {
            serde_json::json!(true)
        } else if name == names[1] {
            serde_json::json!({"not": "OS_WINDOWS"})
        } else {
            serde_json::json!("OS_LINUX")
        };
        assert_eq!(target["requires"], guard);
        assert_eq!(raw["tests"][name]["requires"], guard);
        assert_eq!(
            target["sources"],
            serde_json::json!([format!("test/cmocka/{name}.c")])
        );
        assert_eq!(target["libraries"][0], "OBS::native-test-cmocka");
        let test = &graph.tests[name];
        assert_eq!(test.target, name);
        assert_eq!(test.command, ["${target}"]);
        assert_eq!(test.timeout_seconds, None);
        assert!(test.optional_prefixes.is_empty());
        assert_eq!(test.qt_runtime, None);
        let fragment =
            fs::read_to_string(root.join(format!("test/cmocka/tests.d/{name}.cmake"))).unwrap();
        assert!(!fragment.contains("add_executable("));
        assert!(!fragment.contains("add_test("));
    }
    assert_eq!(
        raw["targets"][names[0]]["includes"],
        serde_json::json!(["plugins/obs-transitions"])
    );
    assert_eq!(
        raw["targets"][names[0]]["libraries"],
        serde_json::json!([
            "OBS::native-test-cmocka", {"name": "m", "requires": {"not": "MSVC"}}
        ])
    );
    graph
        .targets
        .retain(|name, _| names.contains(&name.as_str()));
    graph.tests.retain(|name, _| names.contains(&name.as_str()));
    for (linux, windows) in [(true, false), (false, false), (false, true)] {
        let selected = graph
            .select(
                &[
                    ("OS_LINUX".into(), linux),
                    ("OS_WINDOWS".into(), windows),
                    ("MSVC".into(), windows),
                ]
                .into(),
            )
            .unwrap();
        for name in names {
            let expected =
                name == names[0] || (name == names[1] && !windows) || (name != names[1] && linux);
            assert_eq!(
                selected.targets.contains_key(name),
                expected,
                "{name}: linux={linux}, windows={windows}"
            );
            assert_eq!(
                selected.tests.contains_key(name),
                expected,
                "{name}: linux={linux}, windows={windows}"
            );
        }
    }
}
