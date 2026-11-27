use super::*;

#[test]
fn canonical_source_output_capture_device_swap_is_registered() {
    use obs_build::validation::graph::{Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let name = "test_source_output_capture_device";
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    // Linux with PulseAudio only: it interposes libpulse so monitoring is
    // available without a sound server, as test_set_audio_monitoring_device.
    let selected = graph
        .select(
            &[
                ("OS_LINUX".into(), true),
                ("ENABLE_PULSEAUDIO".into(), true),
            ]
            .into(),
        )
        .unwrap();
    // The test compiles no copy of the C: it calls the exported function of
    // the real libobs (Rust in hybrid, the original C in the reference).
    assert!(
        matches!(selected.targets[name].sources.as_slice(), [Source::Path(path)]
        if path == "test/cmocka/test_source_output_capture_device.c")
    );
    assert!(selected.targets[name].definitions.is_empty());
    assert_eq!(selected.tests[name].target, name);
    for os in ["OS_MACOS", "OS_WINDOWS"] {
        let other = if os == "OS_MACOS" {
            "OS_WINDOWS"
        } else {
            "OS_MACOS"
        };
        let selected = graph
            .select(
                &[
                    (os.into(), true),
                    (other.into(), false),
                    ("OS_LINUX".into(), false),
                    ("ENABLE_PULSEAUDIO".into(), false),
                ]
                .into(),
            )
            .unwrap();
        assert!(!selected.targets.contains_key(name), "{os}");
    }
    let ported = fs::read_to_string(
        root.join("libobs/build/ported-source-sets/source-output-capture-device.json"),
    )
    .unwrap();
    assert!(ported.contains("build/libobs.ported-source-output-capture-device+.h.c"));
    let group = fs::read_to_string(
        root.join("libobs/build/libobs.ported-source-output-capture-device+.h.c"),
    )
    .unwrap();
    assert!(group.contains("#include \"libobs/obs-source-output-capture-device.c.h\""));
    let hybrid = fs::read_to_string(root.join("libobs/build/libobs.core-source+.h.c")).unwrap();
    assert!(!hybrid.contains("#include \"libobs/obs-source-output-capture-device.c.h\""));
    assert!(hybrid.contains("#include \"libobs/obs-source-output-capture-device-fields.c.h\""));
    let exports = fs::read_to_string(
        root.join("libobs/cmake/rust-exports.d/obs-source-source_output_capture_device.txt"),
    )
    .unwrap();
    assert_eq!(
        exports.lines().collect::<Vec<_>>(),
        ["obs_source_audio_output_capture_device_changed"]
    );
}
