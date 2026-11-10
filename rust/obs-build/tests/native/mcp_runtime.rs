//! MCP runtime requests use the existing native builder and shared test binary.
#[test]
fn mcp_runtime_request_is_a_supported_native_build_command() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("request.json");
    std::fs::write(
        &path,
        serde_json::to_vec(&serde_json::json!({
            "root": directory.path(), "core": directory.path().join("libobs.so.30"),
            "output": directory.path().join("runtime")
        }))
        .unwrap(),
    )
    .unwrap();
    let result = obs_build::cli::parse([
        "mcp-runtime".into(),
        "--request".into(),
        path.into_os_string(),
    ]);
    assert!(
        result.is_ok(),
        "MCP must build graphics and probe plugins through the native graph"
    );
}
