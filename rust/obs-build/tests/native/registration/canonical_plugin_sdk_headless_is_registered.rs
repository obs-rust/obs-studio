use super::*;
use obs_build::validation::graph::Graph;

const CASES: [&str; 2] = ["test_pipewire_frame_layout", "test_pulse_default_retry"];

fn requirement(name: &str) -> serde_json::Value {
    let (plugin, sdk) = if name == CASES[0] {
        ("HAVE_OBS_PIPEWIRE", "HAVE_PIPEWIRE_FRAME_LAYOUT_SDK")
    } else {
        ("HAVE_OBS_PULSEAUDIO", "HAVE_PULSE_DEFAULT_RETRY_SDK")
    };
    serde_json::json!({"all": ["OS_LINUX", plugin, sdk]})
}

#[test]
fn canonical_plugin_sdk_headless_is_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let raw: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap())
            .unwrap();
    for name in CASES {
        assert_eq!(raw["targets"][name], expected_target(name), "{name}");
        let mut expected = serde_json::json!({"target": name, "requires": requirement(name)});
        if name == CASES[1] {
            expected["timeout_seconds"] = 10.into();
        }
        assert_eq!(raw["tests"][name], expected, "{name}");
        let fragment =
            fs::read_to_string(root.join(format!("test/cmocka/tests.d/{name}.cmake"))).unwrap();
        assert!(!fragment.contains("add_executable("));
        assert!(!fragment.contains("add_test("));
    }
}

fn expected_target(name: &str) -> serde_json::Value {
    let plugin = if name == CASES[0] {
        "linux-pipewire"
    } else {
        "linux-pulseaudio"
    };
    let mut target = serde_json::json!({
        "sources": [format!("test/cmocka/{name}.c")],
        "includes": [".", format!("plugins/{plugin}")],
        "watch_inputs": [format!("plugins/{plugin}/build/{plugin}-nix.pch.h")],
        "libraries": ["OBS::libobs", if name == CASES[0] { "PipeWire::PipeWire" } else { "PulseAudio::PulseAudio" }, "OBS::native-test-cmocka"],
        "requires": requirement(name)
    });
    if name == CASES[0] {
        target["sources"].as_array_mut().unwrap().extend([
            "plugins/linux-pipewire/formats.c.h".into(),
            "plugins/linux-pipewire/frame-layout.c.h".into(),
        ]);
        target["includes"]
            .as_array_mut()
            .unwrap()
            .push("deps/glad/include".into());
        target["header_packages"] = serde_json::json!(["libdrm", "gio-2.0"]);
    } else {
        target["sources"].as_array_mut().unwrap().extend([
            "plugins/linux-pulseaudio/pulse-reconnect.c.h".into(),
            "plugins/linux-pulseaudio/pulse-timing.c.h".into(),
        ]);
        target["link_options"] = serde_json::json!([
            "LINKER:--wrap=obs_weak_source_addref,--wrap=obs_weak_source_release,--wrap=obs_weak_source_get_source",
            "LINKER:--wrap=obs_obj_get_data,--wrap=obs_source_release,--wrap=obs_queue_task",
            "LINKER:--wrap=obs_source_set_async_compensation"
        ]);
    }
    target
}

#[test]
fn canonical_plugin_sdk_headless_guards() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph
        .targets
        .retain(|name, _| CASES.contains(&name.as_str()));
    graph.tests.retain(|name, _| CASES.contains(&name.as_str()));
    assert_eq!(graph.targets.len(), CASES.len());
    for linux in [false, true] {
        for plugin in [false, true] {
            for sdk in [false, true] {
                let facts = [
                    ("OS_LINUX".into(), linux),
                    ("HAVE_OBS_PIPEWIRE".into(), plugin),
                    ("HAVE_OBS_PULSEAUDIO".into(), plugin),
                    ("HAVE_PIPEWIRE_FRAME_LAYOUT_SDK".into(), sdk),
                    ("HAVE_PULSE_DEFAULT_RETRY_SDK".into(), sdk),
                ]
                .into();
                let selected = graph.select(&facts).unwrap();
                let count = if linux && plugin && sdk {
                    CASES.len()
                } else {
                    0
                };
                assert_eq!(selected.targets.len(), count);
                assert_eq!(selected.tests.len(), count);
            }
        }
    }
}
