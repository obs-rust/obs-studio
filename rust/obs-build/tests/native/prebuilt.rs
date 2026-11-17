//! Transport native fixture builds without adding a test executable.

#[test]
fn prebuilt_mode_rejects_missing_fixture_instead_of_compiling() {
    let directory = tempfile::tempdir().unwrap();
    assert!(read_manifest(directory.path(), "absent").is_err());
}

#[test]
fn prebuilt_mode_rejects_incomplete_or_wrong_host_target_evidence() {
    for (host, target, complete) in [
        ("aarch64-apple-darwin", "aarch64-apple-darwin", true),
        ("x86_64-unknown-linux-gnu", "x86_64-unknown-linux-gnu", true),
        ("x86_64-unknown-linux-gnu", "aarch64-apple-darwin", false),
    ] {
        let manifest = serde_json::json!({
            "version": 1, "host": host, "target": target,
            "complete": complete, "steps": [], "checks": []
        });
        assert!(validate_manifest(&manifest).is_err());
    }
}
