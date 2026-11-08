use super::*;

#[test]
fn camera_wrap_boundary() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let raw: serde_json::Value = serde_json::from_slice(
        &fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap(),
    ).unwrap();
    let target = &raw["targets"]["test_pipewire_camera_registry_race"];
    let groups = &target["unity_groups"];
    assert_eq!(groups["pipewire-implementation"], serde_json::json!(["plugins/linux-pipewire/pipewire.c.h"]));
    let callers = groups["camera-caller"].as_array().unwrap();
    assert!(callers.iter().any(|s| s == "test/cmocka/test_pipewire_camera_registry_race.c"));
    assert!(callers.iter().any(|s| s == "plugins/linux-pipewire/camera-portal.c.h"));
    assert!(!callers.iter().any(|s| s == "plugins/linux-pipewire/pipewire.c.h"));
    assert_eq!(target["pch"], "cmake/pch/obs-c-std.h");
}

#[test]
fn native_pulse_server_in_tier2_closure() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let packages = fs::read_to_string(root.join("rust/tools/linux-validate/apt-packages.txt")).unwrap();
    assert!(packages.lines().any(|line| line == "pulseaudio"));
    let runtime = fs::read_to_string(root.join("rust/tools/linux-validate/runtime-packages.py")).unwrap();
    assert!(runtime.contains("'pulseaudio',"));
}
