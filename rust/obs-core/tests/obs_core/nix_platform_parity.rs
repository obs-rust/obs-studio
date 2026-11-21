//! The C oracle is used by this single test only: no concurrent global writes.
#![allow(unsafe_code)]
use core::ffi::{c_uint, c_void};
use obs_c_oracle as _;
#[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "openbsd"))]
use obs_core::ffi::nix_platform::*;
use obs_core::platform::nix_platform::PlatformState;
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_obs_set_nix_platform(platform: c_uint);
    fn oracle_obs_get_nix_platform() -> c_uint;
    fn oracle_obs_set_nix_platform_display(display: *mut c_void);
    fn oracle_obs_get_nix_platform_display() -> *mut c_void;
}

fn run_child_process() -> bool {
    if std::env::var_os("OBS_NIX_PLATFORM_PARITY_CHILD").is_none() {
        let selector = format!(
            "{}::operation_sequences_match_original_c",
            module_path!().split_once("::").unwrap().1
        );
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &selector, "--nocapture"])
            .env("OBS_NIX_PLATFORM_PARITY_CHILD", "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
        return true;
    }
    false
}

#[test]
fn operation_sequences_match_original_c() {
    if run_child_process() {
        return;
    }
    // SAFETY: this test alone accesses the oracle's globals; pointers stay live,
    // and neither implementation dereferences or owns the borrowed display.
    unsafe {
        assert_eq!(oracle_obs_get_nix_platform(), 1);
        assert!(oracle_obs_get_nix_platform_display().is_null());
        #[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "openbsd"))]
        {
            assert_eq!(obs_get_nix_platform(), 1);
            assert!(obs_get_nix_platform_display().is_null());
        }
    }
    let mut runner = proptest::test_runner::TestRunner::new(proptest::test_runner::Config {
        cases: obs_c_oracle::parity::cases(128),
        ..Default::default()
    });
    runner
        .run(
            &prop::collection::vec((any::<bool>(), any::<u32>()), 0..64),
            |ops| {
                let mut state = PlatformState::new();
                let mut bytes = [7_u8, 9];
                let displays = [
                    core::ptr::null_mut(),
                    (&raw mut bytes[0]).cast(),
                    (&raw mut bytes[1]).cast(),
                ];
                // SAFETY: this test exclusively owns the oracle state and all displays.
                unsafe {
                    oracle_obs_set_nix_platform(1);
                    oracle_obs_set_nix_platform_display(core::ptr::null_mut());
                    #[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "openbsd"))]
                    {
                        obs_nix_platform_set(1);
                        obs_set_nix_platform_display(core::ptr::null_mut());
                    }
                    for (set_platform, value) in [
                        (true, 2),
                        (false, 1),
                        (true, 1),
                        (false, 2),
                        (false, 0),
                        (true, u32::MAX),
                    ]
                    .into_iter()
                    .chain(ops)
                    {
                        if set_platform {
                            let value = value.max(1); // INVALID is a debug assertion, tested separately.
                            state.set_platform(value);
                            oracle_obs_set_nix_platform(value);
                            #[cfg(any(
                                target_os = "linux",
                                target_os = "freebsd",
                                target_os = "openbsd"
                            ))]
                            obs_nix_platform_set(value);
                        } else {
                            let display = displays[(value % 3) as usize];
                            state.set_display(display);
                            oracle_obs_set_nix_platform_display(display);
                            #[cfg(any(
                                target_os = "linux",
                                target_os = "freebsd",
                                target_os = "openbsd"
                            ))]
                            obs_set_nix_platform_display(display);
                        }
                        prop_assert_eq!(state.platform(), oracle_obs_get_nix_platform());
                        prop_assert_eq!(state.display(), oracle_obs_get_nix_platform_display());
                        #[cfg(any(
                            target_os = "linux",
                            target_os = "freebsd",
                            target_os = "openbsd"
                        ))]
                        {
                            prop_assert_eq!(obs_get_nix_platform(), oracle_obs_get_nix_platform());
                            prop_assert_eq!(
                                obs_get_nix_platform_display(),
                                oracle_obs_get_nix_platform_display()
                            );
                        }
                    }
                    oracle_obs_set_nix_platform_display(core::ptr::null_mut());
                    #[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "openbsd"))]
                    obs_set_nix_platform_display(core::ptr::null_mut());
                }
                prop_assert_eq!(bytes, [7, 9]);
                Ok(())
            },
        )
        .unwrap();
}
