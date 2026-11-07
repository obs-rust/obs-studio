use super::*;

#[test]
fn canonical_pulse_monitor_format_is_registered() {
    use obs_build::validation::graph::Graph;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let raw: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap())
            .unwrap();
    let name = "test_pulse_monitor_format";
    let guard =
        serde_json::json!({"all": [{"any": ["OS_LINUX", "OS_FREEBSD"]}, "ENABLE_PULSEAUDIO"]});
    assert_eq!(
        raw["targets"][name],
        serde_json::json!({
            "sources": ["test/cmocka/test_pulse_monitor_format.c", "libobs/audio-monitoring/pulse/pulseaudio-monitor-format.c.h"],
            "includes": ["libobs/audio-monitoring/pulse"],
            "watch_inputs": ["libobs/build/libobs-ported-pulse-format.pch.h", "libobs/audio-monitoring/pulse/pulseaudio-monitor-format.h"],
            "libraries": ["OBS::libobs", "PulseAudio::PulseAudio", "OBS::native-test-cmocka"],
            "sanitizers": "address,undefined", "requires": guard
        })
    );
    assert_eq!(
        raw["tests"][name],
        serde_json::json!({"target": name, "requires": guard})
    );
    let fragment =
        fs::read_to_string(root.join("test/cmocka/tests.d/test_pulse_monitor_format.cmake"))
            .unwrap();
    assert!(!fragment.contains("add_executable("));
    assert!(!fragment.contains("add_test("));
    let mut graph = Graph::parse(&raw.to_string()).unwrap();
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in [
        "OS_LINUX",
        "OS_FREEBSD",
        "OS_OPENBSD",
        "OS_MACOS",
        "OS_WINDOWS",
    ] {
        for enabled in [false, true] {
            let facts = [
                ("OS_LINUX".into(), os == "OS_LINUX"),
                ("OS_FREEBSD".into(), os == "OS_FREEBSD"),
                ("ENABLE_PULSEAUDIO".into(), enabled),
            ]
            .into();
            let selected = graph.select(&facts).unwrap();
            let expected = enabled && ["OS_LINUX", "OS_FREEBSD"].contains(&os);
            assert_eq!(
                selected.targets.contains_key(name),
                expected,
                "{os}/{enabled}"
            );
            assert_eq!(
                selected.tests.contains_key(name),
                expected,
                "{os}/{enabled}"
            );
        }
    }
}
