//! Exact original-C behavior and callback trace parity; no intentional differences.
use obs_c_oracle as _;
use obs_graphics::ffi::viewport::{
    gs_reset_viewport, gs_set_2d_mode, obs_graphics_device_max_texture_size,
};
use proptest::prelude::*;
use std::ffi::{CStr, c_char, c_void};
unsafe extern "C" {
    fn oracle_viewport_reset(
        present: bool,
        width: u32,
        height: u32,
        callback: bool,
        limit: u32,
        kind: i32,
    );
    fn oracle_gs_reset_viewport();
    fn oracle_gs_set_2d_mode();
    fn oracle_viewport_max() -> u32;
    fn oracle_viewport_context() -> *mut c_void;
    fn oracle_viewport_function() -> *const c_char;
    fn oracle_viewport_event(index: u32) -> u32;
    fn oracle_viewport_rect(index: u32) -> i32;
    fn oracle_viewport_projection(index: u32) -> u32;
}
fn compare(present: bool, width: u32, height: u32, callback: bool, limit: u32, kind: i32) {
    // SAFETY: the oracle owns this thread's real-header context and recording state;
    // each call completes before reset, and all indices are within fixed arrays.
    unsafe {
        for op in 0..3 {
            oracle_viewport_reset(present, width, height, callback, limit, kind);
            let expected = match op {
                0 => {
                    oracle_gs_reset_viewport();
                    0
                }
                1 => {
                    oracle_gs_set_2d_mode();
                    0
                }
                _ => oracle_viewport_max(),
            };
            let function = CStr::from_ptr(oracle_viewport_function())
                .to_bytes()
                .to_vec();
            let events: Vec<_> = (0..8).map(|i| oracle_viewport_event(i)).collect();
            let rect: Vec<_> = (0..4).map(|i| oracle_viewport_rect(i)).collect();
            let projection: Vec<_> = (0..6).map(|i| oracle_viewport_projection(i)).collect();
            oracle_viewport_reset(present, width, height, callback, limit, kind);
            let actual = match op {
                0 => {
                    gs_reset_viewport();
                    0
                }
                1 => {
                    gs_set_2d_mode();
                    0
                }
                _ => obs_graphics_device_max_texture_size(oracle_viewport_context()),
            };
            assert_eq!(actual, expected);
            assert_eq!(
                CStr::from_ptr(oracle_viewport_function()).to_bytes(),
                function
            );
            assert_eq!(
                (0..8).map(|i| oracle_viewport_event(i)).collect::<Vec<_>>(),
                events
            );
            assert_eq!(
                (0..4).map(|i| oracle_viewport_rect(i)).collect::<Vec<_>>(),
                rect
            );
            assert_eq!(
                (0..6)
                    .map(|i| oracle_viewport_projection(i))
                    .collect::<Vec<_>>(),
                projection
            );
        }
    }
}
#[test]
fn fixed_viewport_edges_match_c() {
    for present in [false, true] {
        for (w, h) in [
            (0, 0),
            (1, 7),
            (65535, 32769),
            (16_777_217, 16_777_219),
            (u32::MAX, u32::MAX),
        ] {
            for kind in [-1, 1, 2, 3, 99] {
                for callback in [false, true] {
                    for limit in [0, 7, 8192, u32::MAX] {
                        compare(present, w, h, callback, limit, kind);
                    }
                }
            }
        }
    }
}
proptest! {
 #![proptest_config(ProptestConfig {cases:obs_c_oracle::parity::cases(256),failure_persistence:None,..ProptestConfig::default()})]
 #[test]
 fn viewport_matches_c(present in any::<bool>(),width in any::<u32>(),height in any::<u32>(),callback in any::<bool>(),limit in any::<u32>(),kind in any::<i32>()) {
 compare(present,width,height,callback,limit,kind);
 }
}
