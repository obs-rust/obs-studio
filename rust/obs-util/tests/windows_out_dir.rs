//! Qualify the real build-script OUT_DIR without another category executable.
#[test]
#[cfg(all(windows, target_env = "msvc"))]
fn compiled_out_dir_reads_transported_build_output() {
    let Ok(root) = std::env::var("OBS_WINDOWS_PROOF_ROOT") else { return };
    let out = env!("OUT_DIR");
    assert!(out.starts_with(&format!("{root}/target/")), "compiled OUT_DIR root mismatch: {out}");
    let file = std::path::Path::new(out).join("plugin_seh.lib");
    assert_eq!(std::fs::read(file).unwrap(), include_bytes!(concat!(env!("OUT_DIR"), "/plugin_seh.lib")));
}
