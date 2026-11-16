//! Original-C differential tests. No intentional differences or exclusions.
use core::ffi::c_void;
use obs_source::ffi::source_media_getters::{
    obs_source_media_get_duration as duration, obs_source_media_get_state as state,
    obs_source_media_get_time as time,
};
use proptest::prelude::*;
type Time = unsafe extern "C" fn(*mut c_void) -> i64;
type State = unsafe extern "C" fn(*mut c_void) -> u32;
unsafe extern "C" {
    fn oracle_obs_source_media_get_duration(source: *mut c_void) -> i64;
    fn oracle_obs_source_media_get_time(source: *mut c_void) -> i64;
    fn oracle_obs_source_media_get_state(source: *mut c_void) -> u32;
    fn oracle_source_media_getters_run(
        d: Time,
        t: Time,
        s: State,
        valid: bool,
        flags: u32,
        mask: u32,
        duration: i64,
        time: i64,
        state: u32,
        reenter: bool,
        out: *mut i64,
    );
    fn oracle_source_media_getters_layout(out: *mut usize);
}
fn check(valid: bool, flags: u32, mask: u32, d: i64, t: i64, s: u32, reenter: bool) {
    let mut rust = [0; 16];
    let mut c = [0; 16];
    // SAFETY: C owns real-header stack fixtures and writes exactly sixteen observations.
    unsafe {
        oracle_source_media_getters_run(
            duration,
            time,
            state,
            valid,
            flags,
            mask,
            d,
            t,
            s,
            reenter,
            rust.as_mut_ptr(),
        );
        oracle_source_media_getters_run(
            oracle_obs_source_media_get_duration,
            oracle_obs_source_media_get_time,
            oracle_obs_source_media_get_state,
            valid,
            flags,
            mask,
            d,
            t,
            s,
            reenter,
            c.as_mut_ptr(),
        );
    }
    assert_eq!(rust, c);
    for chunk in rust.as_chunks::<8>().0 {
        assert_eq!(chunk[6..], [1, 0]);
    }
}
#[test]
fn real_header_layout() {
    let mut values = [0; 11];
    // SAFETY: C writes exactly eleven representation and enum measurements.
    unsafe { oracle_source_media_getters_layout(values.as_mut_ptr()) };
    assert_eq!(
        values,
        [
            size_of::<u32>(),
            align_of::<u32>(),
            1 << 13,
            0,
            1,
            2,
            3,
            4,
            5,
            6,
            7
        ]
    );
}
#[test]
fn characterized_edges() {
    for valid in [false, true] {
        for flags in [0, 1, 1 << 13, u32::MAX] {
            for mask in 0..8 {
                for (d, t) in [
                    (0, 1),
                    (-1, 0),
                    (i64::MIN, i64::MAX),
                    (4294967296, -4294967297),
                ] {
                    for s in 0..8 {
                        check(valid, flags, mask, d, t, s, false);
                    }
                }
            }
        }
    }
    check(true, 1 << 13, 7, -9, 42, 4, true);
}
proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(128)))]
    #[test]
    fn original_c_parity(valid in any::<bool>(), flags in any::<u32>(), mask in 0u32..8, d in any::<i64>(), t in any::<i64>(), s in 0u32..8) {
        check(valid, flags, mask, d, t, s, false);
    }
}

#[test]
fn null_diagnostics_match_c() {
    if !crate::output::isolated(concat!(module_path!(), "::null_diagnostics_match_c")) {
        return;
    }
    type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
    type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);
    unsafe extern "C" {
        fn oracle_source_media_getters_logs(
            d: Time,
            t: Time,
            s: State,
            get: GetLog,
            set: SetLog,
        ) -> bool;
    }
    // SAFETY: isolated child exclusively owns the log handler; oracle restores it.
    unsafe {
        assert!(oracle_source_media_getters_logs(
            duration,
            time,
            state,
            obs_util::ffi::base::base_get_log_handler,
            obs_util::ffi::base::base_set_log_handler
        ));
    }
}
