use super::*;

/// #1390: portable capture decisions retain real libobs and cmocka linkage.
#[test]
fn canonical_capture_helpers_are_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let raw: serde_json::Value = serde_json::from_slice(
        &fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap(),
    ).unwrap();
    for name in ["test_coreaudio_retry", "test_coreaudio_zero_channels", "test_display_reinit"] {
        let target = raw["targets"].get(name).expect("missing portable capture helper");
        let mut sources = vec![format!("test/cmocka/{name}.c")];
        if name == "test_display_reinit" {
            sources.push("plugins/mac-capture/display-reinit.c.h".into());
        }
        assert_eq!(target["sources"], serde_json::json!(sources));
        assert_eq!(target["requires"], true);
        assert_eq!(target["libraries"], serde_json::json!(["OBS::libobs", "OBS::native-test-cmocka"]));
        assert_eq!(target["include_dirs"], serde_json::json!(["plugins/mac-capture"]));
        assert_eq!(target["watch_inputs"], serde_json::json!(["plugins/mac-capture"]));
        let test = &raw["tests"][name];
        assert_eq!(test, &serde_json::json!({"target": name, "requires": true}));
        let fragment = fs::read_to_string(root.join(format!("test/cmocka/tests.d/{name}.cmake"))).unwrap();
        assert!(!fragment.contains("add_executable("));
        assert!(!fragment.contains("add_test("));
    }
}
