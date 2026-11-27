//! Tier 3: original C removal and the Rust shim over real C structures,
//! retaining real reference counting and signal callbacks on both sides.
//! NULL canvas and invalid controls are C UB, excluded. No intentional differences.
use core::ffi::c_void;
use obs_core::ffi::canvas_remove::obs_canvas_remove;
use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};

unsafe extern "C" {
    fn oracle_obs_canvas_remove(canvas: *mut c_void);
    fn oracle_canvas_remove_run(
        candidate: unsafe extern "C" fn(*mut c_void),
        flags: u32,
        private: bool,
        expired: bool,
        mutate: bool,
        callback_flags: u32,
        repeats: u32,
        trace: *mut usize,
    );
}

fn compare(
    flags: u32,
    private: bool,
    expired: bool,
    mutate: bool,
    callback_flags: u32,
    repeats: u32,
) {
    let mut c = [0; 29];
    let mut rust = [0; 29];
    // SAFETY: C owns live real-layout canvases, controls and handlers, invokes
    // each implementation synchronously, writes exactly 29 words and frees
    // every handler. Counts cannot reach destruction or overflow in this probe.
    unsafe {
        oracle_canvas_remove_run(
            oracle_obs_canvas_remove,
            flags,
            private,
            expired,
            mutate,
            callback_flags,
            repeats,
            c.as_mut_ptr(),
        );
        oracle_canvas_remove_run(
            obs_canvas_remove,
            flags,
            private,
            expired,
            mutate,
            callback_flags,
            repeats,
            rust.as_mut_ptr(),
        );
    }
    assert_eq!(
        rust, c,
        "flags={flags:x} private={private} expired={expired} mutate={mutate} callback={callback_flags:x} repeats={repeats}"
    );
    assert_eq!(
        rust[26],
        if expired { usize::MAX } else { 2 },
        "balanced strong count"
    );
    assert_eq!(rust[27], 3, "unchanged weak count");
    assert_eq!(
        rust[28], 1,
        "all other canvas bytes and control target unchanged"
    );
    for row in rust[1..1 + rust[0] * 4].as_chunks::<4>().0 {
        assert_eq!(row[1], 1, "callback canvas identity");
        assert_eq!(row[3], 3, "promotion held across callbacks");
    }
}

#[test]
fn original_canvas_remove_matches_rust() {
    const CHILD: &str = "OBS_CANVAS_REMOVE_PARITY_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let selector = format!(
            "{}::original_canvas_remove_matches_rust",
            module_path!().split_once("::").unwrap().1
        );
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &selector, "--nocapture"])
            .env(CHILD, "1")
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    for flags in [0, 1, 2, 8, 32, 0x8000_0000, 0x8000_0020, u32::MAX] {
        for private in [false, true] {
            for expired in [false, true] {
                for mutate in [false, true] {
                    for callback_flags in [0, 1, 0x8000_0000, u32::MAX] {
                        for repeats in [1, 2, 3] {
                            compare(flags, private, expired, mutate, callback_flags, repeats);
                        }
                    }
                }
            }
        }
    }
    let mut runner = TestRunner::new(Config {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..Config::default()
    });
    runner
        .run(
            &(
                any::<u32>(),
                any::<bool>(),
                any::<bool>(),
                any::<bool>(),
                any::<u32>(),
                1u32..=3,
            ),
            |(flags, private, expired, mutate, callback_flags, repeats)| {
                compare(flags, private, expired, mutate, callback_flags, repeats);
                Ok(())
            },
        )
        .unwrap();
}
