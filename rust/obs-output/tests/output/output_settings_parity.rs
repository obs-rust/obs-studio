//! Tier 3: original C, real private layout, pointer identity and retained ownership.
use core::ffi::{c_char, c_void};
use core::ptr;
use obs_output::ffi::output_settings::obs_output_get_settings as settings;
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type Getter = unsafe extern "C" fn(*const c_void) -> *mut c_void;
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);
unsafe extern "C" {
    fn obs_data_create() -> *mut c_void;
    fn obs_data_release(data: *mut c_void);
    fn obs_data_set_int(data: *mut c_void, name: *const c_char, value: i64);
    fn obs_data_get_int(data: *mut c_void, name: *const c_char) -> i64;
    fn oracle_output_settings_new(data: *mut c_void, flags: u32) -> *mut c_void;
    fn oracle_output_settings_free(output: *mut c_void);
    fn oracle_output_settings_addresses(output: *const c_void) -> bool;
    fn oracle_obs_output_get_settings(output: *const c_void) -> *mut c_void;
    fn oracle_output_settings_logs(getter: Getter, get: GetLog, set: SetLog) -> bool;
}

fn check(value: i64, flags: u32, present: bool) {
    // SAFETY: all handles are allocated by their matching constructors. Each
    // getter grants a reference, released after verifying it outlives the output.
    unsafe {
        let data = if present {
            obs_data_create()
        } else {
            ptr::null_mut()
        };
        obs_data_set_int(data, c"value".as_ptr(), value);
        let output = oracle_output_settings_new(data, flags);
        assert!(!output.is_null());
        assert!(oracle_output_settings_addresses(output));
        let r = settings(output);
        let c = oracle_obs_output_get_settings(output);
        assert_eq!((r, c), (data, data));
        let again = settings(output);
        assert_eq!(again, data);
        oracle_output_settings_free(output);
        // Drop the output-owned reference, then exercise each caller-owned one.
        obs_data_release(data);
        if present {
            assert_eq!(obs_data_get_int(r, c"value".as_ptr()), value);
            obs_data_release(r);
            obs_data_set_int(c, c"value".as_ptr(), value.wrapping_add(1));
            assert_eq!(
                obs_data_get_int(again, c"value".as_ptr()),
                value.wrapping_add(1)
            );
            obs_data_release(c);
            obs_data_release(again);
        }
    }
}

#[test]
fn characterized_edges_and_retained_lifetime() {
    for value in [i64::MIN, 0, i64::MAX] {
        for flags in [0, u32::MAX] {
            for present in [false, true] {
                check(value, flags, present);
            }
        }
    }
}

#[test]
fn null_diagnostics_match_c() {
    if !super::isolated(concat!(module_path!(), "::null_diagnostics_match_c")) {
        return;
    }
    // SAFETY: the isolated process owns and restores the global log handler.
    unsafe {
        assert!(oracle_output_settings_logs(
            settings,
            base_get_log_handler,
            base_set_log_handler
        ));
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(128), failure_persistence: None, .. ProptestConfig::default() })]
    #[test]
    fn settings_matches_c(value in any::<i64>(), flags in any::<u32>(), present in any::<bool>()) {
        check(value, flags, present);
    }
}
