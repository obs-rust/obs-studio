//! Re-exec isolates the process-global ABI state from the rest of Core.
#![cfg(any(target_os = "linux", target_os = "freebsd"))]
#![allow(unsafe_code)]
use obs_core::ffi::nix_platform::*;

#[test]
fn public_abi_defaults_and_borrowed_pointer() {
    let selector = format!(
        "{}::public_abi_defaults_and_borrowed_pointer",
        module_path!().split_once("::").unwrap().1
    );
    if std::env::var_os("OBS_NIX_PLATFORM_ABI_CHILD").is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &selector, "--nocapture"])
            .env("OBS_NIX_PLATFORM_ABI_CHILD", "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
        return;
    }
    let mut byte = 7_u8;
    let display = (&raw mut byte).cast();
    // SAFETY: a fresh child process is the sole accessor of these globals.
    // The stack display is borrowed and never dereferenced by the API.
    unsafe {
        assert_eq!(obs_get_nix_platform(), 1);
        assert!(obs_get_nix_platform_display().is_null());
        obs_set_nix_platform_display(display);
        assert_eq!(obs_get_nix_platform_display(), display);
        for platform in [2, 1, u32::MAX, 17, 1] {
            obs_set_nix_platform(platform);
            assert_eq!(obs_get_nix_platform(), platform);
            assert_eq!(obs_get_nix_platform_display(), display);
        }
        obs_set_nix_platform_display(core::ptr::null_mut());
        assert!(obs_get_nix_platform_display().is_null());
        assert_eq!(obs_get_nix_platform(), 1);
    }
    assert_eq!(byte, 7);
}
