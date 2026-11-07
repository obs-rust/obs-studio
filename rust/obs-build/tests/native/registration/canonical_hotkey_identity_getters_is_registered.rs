use super::*;

/// #3924: characterize public getters through the actual library on every OS.
#[test]
fn canonical_hotkey_identity_getters_is_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let raw: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap())
            .unwrap();
    let name = "test_hotkey_identity_getters";
    assert_eq!(
        raw["targets"][name]["sources"],
        serde_json::json!(["test/cmocka/test_hotkey_identity_getters.c"])
    );
    assert_eq!(
        raw["targets"][name]["libraries"],
        serde_json::json!([
            "OBS::libobs", "OBS::caption", "Uthash::Uthash", "OBS::native-test-cmocka",
            {"name": "OBS::w32-pthreads", "requires": "OS_WINDOWS"}
        ])
    );
    assert_eq!(raw["tests"][name]["target"], name);
    assert_eq!(
        raw["tests"][name]["command"],
        serde_json::json!(["${target}"])
    );
    assert_eq!(raw["tests"][name]["suite"], "core");
    let fragment =
        fs::read_to_string(root.join(format!("test/cmocka/tests.d/{name}.cmake"))).unwrap();
    assert!(!fragment.contains("add_executable("));
    assert!(!fragment.contains("add_test("));
}
