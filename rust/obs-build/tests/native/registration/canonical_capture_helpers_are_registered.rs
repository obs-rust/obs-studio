use super::*;

/// #1390: portable capture decisions retain real libobs and cmocka linkage.
#[test]
fn canonical_capture_helpers_are_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let raw: serde_json::Value = obs_build::validation::graph::Graph::read_value(
        &root.join("test/cmocka/build/test-graph.d"),
    )
    .unwrap();
    for name in [
        "test_coreaudio_retry",
        "test_coreaudio_zero_channels",
        "test_display_reinit",
    ] {
        let target = raw["targets"]
            .get(name)
            .expect("missing portable capture helper");
        let mut sources = vec![format!("test/cmocka/{name}.c")];
        if name == "test_display_reinit" {
            sources.push("plugins/mac-capture/display-reinit.c.h".into());
        }
        assert_eq!(target["sources"], serde_json::json!(sources));
        assert_eq!(target["requires"], true);
        let mut libraries = vec![
            serde_json::json!("OBS::libobs"),
            serde_json::json!("OBS::native-test-cmocka"),
        ];
        if name == "test_coreaudio_retry" {
            libraries
                .push(serde_json::json!({"name": "OBS::w32-pthreads", "requires": "OS_WINDOWS"}));
        }
        assert_eq!(target["libraries"], serde_json::json!(libraries));
        assert_eq!(
            target["includes"],
            serde_json::json!(["plugins/mac-capture"])
        );
        assert_eq!(
            target["watch_inputs"],
            serde_json::json!(["plugins/mac-capture"])
        );
        let test = &raw["tests"][name];
        assert_eq!(test, &serde_json::json!({"target": name, "requires": true}));
        let fragment =
            fs::read_to_string(root.join(format!("test/cmocka/tests.d/{name}.cmake"))).unwrap();
        assert!(!fragment.contains("add_executable("));
        assert!(!fragment.contains("add_test("));
    }
    let mut graph = obs_build::validation::graph::Graph::parse(&raw.to_string()).unwrap();
    let names = [
        "test_coreaudio_retry",
        "test_coreaudio_zero_channels",
        "test_display_reinit",
    ];
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
        let selected = graph.select(&[(os.into(), true)].into()).unwrap();
        assert_eq!(selected.targets.len(), 3);
        assert_eq!(selected.tests.len(), 3);
    }
}
