use super::*;

const ORIGINAL: &str = "libobs/obs-hotkeys-background-status-changed.c.h";
const LINK_SUPPORT: &str = "test/cmocka/hotkeys-background-status-link-support.c";
const REQUEST: &str = "obs_hotkeys_background_status_changed=obs_hotkeys_background_status_request";

/// Every standalone hotkey world that compiled the original now links the
/// real adapter and reaches the Rust notification through its request entry.
#[test]
fn canonical_hotkeys_background_status_consumers_are_switched() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let graph: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(root.join("test/cmocka/build/test-graph.json")).unwrap(),
    )
    .unwrap();
    let mut consumers = Vec::new();
    for (name, target) in graph["targets"].as_object().unwrap() {
        let sources = target["sources"].as_array().unwrap();
        if name == "test_hotkeys_background_status_changed" {
            continue;
        }
        assert!(!sources.contains(&serde_json::json!(ORIGINAL)), "{name}");
        if let Some(groups) = target["unity_groups"].as_object() {
            for group in groups.values() {
                assert!(
                    !group
                        .as_array()
                        .unwrap()
                        .contains(&serde_json::json!(ORIGINAL)),
                    "{name}"
                );
            }
        }
        if sources.contains(&serde_json::json!(LINK_SUPPORT)) {
            assert!(
                target["libraries"]
                    .as_array()
                    .unwrap()
                    .contains(&serde_json::json!("libobs_rust-static")),
                "{name}"
            );
            assert!(
                target["definitions"]
                    .as_array()
                    .unwrap()
                    .contains(&serde_json::json!(REQUEST)),
                "{name}"
            );
            assert_eq!(
                target["unity_groups"]["hotkeys_background_status"],
                serde_json::json!([LINK_SUPPORT]),
                "{name}"
            );
            consumers.push(name.as_str());
        }
    }
    assert_eq!(
        consumers,
        [
            "test_hotkey_background_enum",
            "test_hotkey_background_press",
            "test_hotkey_background_trigger",
            "test_hotkey_binding_press_steps",
            "test_hotkey_getters_characterization",
            "test_hotkey_load",
            "test_hotkey_pair_steps",
            "test_hotkey_press_steps",
            "test_hotkey_unregister_entry",
            "test_portal_shortcuts",
            "test_wayland_hotkeys",
        ]
    );
    let support = fs::read_to_string(
        root.join("test/cmocka/tests.d/hotkeys_background_status_link_support.cmake"),
    )
    .unwrap();
    for consumer in consumers {
        assert!(support.lines().any(|line| line.trim() == consumer));
    }
    assert!(support.contains(REQUEST));
    let link = fs::read_to_string(root.join(LINK_SUPPORT)).unwrap();
    assert!(link.contains("#include \"libobs/obs-hotkeys-background-status-changed-fields.c.h\""));
}

/// The unchanged characterization keeps testing the original; its external
/// twin runs the same assertions on the Rust shim through the real adapters.
#[test]
fn canonical_hotkeys_background_status_swap_is_registered() {
    use obs_build::validation::graph::{Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let c_test = "test_hotkeys_background_status_changed";
    let rust_test = "test_hotkeys_background_status_changed_rust";
    graph
        .targets
        .retain(|key, _| key == c_test || key == rust_test);
    graph
        .tests
        .retain(|key, _| key == c_test || key == rust_test);
    let test_source = "test/cmocka/test_hotkeys_background_status_changed.c";
    let record = "test/cmocka/hotkeys-background-status-record-support.c";
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        for available in [false, true] {
            let facts = [
                (os.into(), true),
                ("HAVE_LIBOBS_RUST_STATIC".into(), available),
            ]
            .into();
            let selected = graph.select(&facts).unwrap();
            assert!(matches!(selected.targets[c_test].sources.as_slice(),
                [Source::Path(a)] if a == test_source));
            assert!(selected.targets[c_test].definitions.is_empty());
            assert_eq!(selected.tests[c_test].target, c_test);
            assert_eq!(selected.targets.contains_key(rust_test), available);
            if !available {
                continue;
            }
            let target = &selected.targets[rust_test];
            assert!(matches!(target.sources.as_slice(),
                [Source::Path(a), Source::Path(b)] if a == test_source && b == record));
            assert_eq!(target.unity_groups["status_changed"], [test_source, record]);
            assert_eq!(target.definitions, ["HOTKEY_BACKGROUND_STATUS_EXTERNAL"]);
            assert_eq!(selected.tests[rust_test].target, rust_test);
        }
    }
    files_are_wired(&root, test_source, record);
}

/// The record support, the ported group, the hybrid group and the hidden list.
fn files_are_wired(root: &Path, test_source: &str, record: &str) {
    let fields = "#include \"libobs/obs-hotkeys-background-status-changed-fields.c.h\"";
    let record_text = fs::read_to_string(root.join(record)).unwrap();
    assert!(record_text.contains("#define obs current_core"));
    assert!(record_text.contains("#define signal_handler_signal record_signal"));
    assert!(record_text.contains(fields));
    let test_text = fs::read_to_string(root.join(test_source)).unwrap();
    assert!(test_text.contains("#ifndef HOTKEY_BACKGROUND_STATUS_EXTERNAL"));
    let ported = fs::read_to_string(
        root.join("libobs/build/ported-source-sets/hotkeys-background-status-changed.json"),
    )
    .unwrap();
    assert!(ported.contains("build/libobs.ported-hotkeys-background-status-changed+.h.c"));
    let group = fs::read_to_string(
        root.join("libobs/build/libobs.ported-hotkeys-background-status-changed+.h.c"),
    )
    .unwrap();
    let original = format!("#include \"{ORIGINAL}\"");
    assert!(group.contains(&original));
    let hybrid = fs::read_to_string(root.join("libobs/build/libobs.core+.h.c")).unwrap();
    assert!(!hybrid.contains(&original));
    // The C-owned core, handler and signal dispatch stay in the core group.
    assert!(hybrid.contains(fields));
    let hidden = fs::read_to_string(
        root.join("libobs/cmake/rust-hidden.d/obs-core-hotkeys_background_status.txt"),
    )
    .unwrap();
    let names: Vec<_> = hidden
        .lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
        .collect();
    assert_eq!(
        names,
        [
            "obs_hotkeys_background_status_changed",
            "obs_hotkeys_background_status_request"
        ]
    );
}
