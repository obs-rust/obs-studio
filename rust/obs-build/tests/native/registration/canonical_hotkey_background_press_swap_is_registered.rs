use super::*;

#[test]
fn canonical_hotkey_background_press_swap_is_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let graph: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(root.join("test/cmocka/build/test-graph.json")).unwrap(),
    )
    .unwrap();
    let mut consumers = Vec::new();
    for (name, target) in graph["targets"].as_object().unwrap() {
        let sources = target["sources"].as_array().unwrap();
        assert!(!sources.contains(&serde_json::json!("libobs/obs-hotkey-background-press.c.h")));
        if sources.contains(&serde_json::json!("libobs/obs-hotkey.c.h")) {
            assert!(
                sources.contains(&serde_json::json!(
                    "test/cmocka/hotkey-background-press-link-support.c"
                )),
                "{name}"
            );
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
                    .contains(&serde_json::json!(
                        "obs_hotkey_enable_background_press=obs_hotkey_background_press_request"
                    )),
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
            "test_portal_shortcuts",
            "test_wayland_hotkeys",
        ]
    );
    let support = fs::read_to_string(
        root.join("test/cmocka/tests.d/hotkey_background_press_link_support.cmake"),
    )
    .unwrap();
    for consumer in consumers {
        assert!(support.lines().any(|line| line.trim() == consumer));
    }
    let ported = fs::read_to_string(
        root.join("libobs/build/ported-source-sets/hotkey-background-press.json"),
    )
    .unwrap();
    assert!(ported.contains("build/libobs.ported-hotkey-background-press+.h.c"));
    let group =
        fs::read_to_string(root.join("libobs/build/libobs.ported-hotkey-background-press+.h.c"))
            .unwrap();
    assert!(group.contains("#include \"libobs/obs-hotkey-background-press.c.h\""));
    let hybrid = fs::read_to_string(root.join("libobs/build/libobs.core+.h.c")).unwrap();
    assert!(!hybrid.contains("#include \"libobs/obs-hotkey-background-press.c.h\""));
    assert!(hybrid.contains("#include \"libobs/obs-hotkey-background-press-fields.c.h\""));
}
