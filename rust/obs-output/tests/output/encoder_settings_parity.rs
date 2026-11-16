//! Tier 3: original C getter, production adapter and real-header encoder fixtures.
//! No intentional behavior differences or excluded inputs; no mirrored layouts.
use core::ffi::c_void;
use obs_data::ffi::obs_data::{
    obs_data_create, obs_data_get_int, obs_data_release, obs_data_set_int, ref_count_for_tests,
};
use obs_output::ffi::encoder_settings::obs_encoder_get_settings;
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type Getter = unsafe extern "C" fn(*const c_void) -> *mut c_void;
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);
unsafe extern "C" {
    fn oracle_encoder_settings_new(settings: *mut c_void, flags: u32) -> *mut c_void;
    fn oracle_encoder_settings_unchanged(encoder: *const c_void) -> bool;
    fn oracle_encoder_settings_free(encoder: *mut c_void);
    fn oracle_obs_encoder_get_settings(encoder: *const c_void) -> *mut c_void;
    fn oracle_encoder_settings_logs(getter: Getter, get: GetLog, set: SetLog) -> bool;
}

struct Fixture {
    encoder: *mut c_void,
    settings: *mut c_void,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: fixture and its initial settings reference are owned by this guard.
        unsafe {
            oracle_encoder_settings_free(self.encoder);
            obs_data_release(self.settings);
        }
    }
}

fn check(present: bool, flags: u32, value: i64, extra_refs: u8) {
    // SAFETY: each fixture owns a encoder with the real C layout and a live settings
    // object or NULL. Each getter adds one reference, released before the next call.
    unsafe {
        let settings = if present {
            obs_data_create()
        } else {
            core::ptr::null_mut()
        };
        if present {
            assert!(!settings.is_null());
        }
        obs_data_set_int(settings, c"bitrate".as_ptr(), value);
        let fixture = Fixture {
            encoder: oracle_encoder_settings_new(settings, flags),
            settings,
        };
        assert!(!fixture.encoder.is_null());
        let mut held = Vec::new();
        for _ in 0..extra_refs {
            held.push(obs_encoder_get_settings(fixture.encoder));
        }
        for getter in [
            obs_encoder_get_settings as Getter,
            oracle_obs_encoder_get_settings as Getter,
        ] {
            let before = ref_count_for_tests(settings);
            let result = getter(fixture.encoder);
            assert_eq!(result, settings);
            assert_eq!(ref_count_for_tests(settings), before + usize::from(present));
            assert_eq!(
                obs_data_get_int(result, c"bitrate".as_ptr()),
                if present { value } else { 0 }
            );
            assert!(oracle_encoder_settings_unchanged(fixture.encoder));
            obs_data_release(result);
            assert_eq!(ref_count_for_tests(settings), before);
        }
        for settings in held {
            obs_data_release(settings);
        }
        assert_eq!(ref_count_for_tests(settings), usize::from(present));
    }
}

#[test]
fn real_header_identity_reference_and_state() {
    for present in [false, true] {
        for flags in [0, u32::MAX] {
            for value in [i64::MIN, 0, i64::MAX] {
                check(present, flags, value, 3);
            }
        }
    }
}

#[test]
fn null_diagnostics_match_c() {
    if !crate::output::isolated(concat!(module_path!(), "::null_diagnostics_match_c")) {
        return;
    }
    // SAFETY: isolated child owns the process-global log handler; C restores it.
    unsafe {
        assert!(oracle_encoder_settings_logs(
            obs_encoder_get_settings,
            base_get_log_handler,
            base_set_log_handler
        ));
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(128), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn arbitrary_settings_match_c(present in any::<bool>(), flags in any::<u32>(), value in any::<i64>(), extra_refs in 0u8..8) {
        check(present, flags, value, extra_refs);
    }
}

#[test]
fn owned_settings_survive_encoder_fixture_and_original_reference() {
    // SAFETY: the fixture borrows the initial reference; each getter gives us
    // one owned reference, which remains live after freeing the fixture.
    unsafe {
        let settings = obs_data_create();
        obs_data_set_int(settings, c"bitrate".as_ptr(), 2500);
        let encoder = oracle_encoder_settings_new(settings, 0);
        assert!(!encoder.is_null());
        let rust_owned = obs_encoder_get_settings(encoder);
        let c_owned = oracle_obs_encoder_get_settings(encoder);
        assert_eq!(rust_owned, settings);
        assert_eq!(c_owned, settings);
        assert_eq!(ref_count_for_tests(settings), 3);
        oracle_encoder_settings_free(encoder);
        obs_data_release(settings);
        assert_eq!(ref_count_for_tests(rust_owned), 2);
        assert_eq!(obs_data_get_int(rust_owned, c"bitrate".as_ptr()), 2500);
        obs_data_release(c_owned);
        assert_eq!(ref_count_for_tests(rust_owned), 1);
        assert_eq!(obs_data_get_int(rust_owned, c"bitrate".as_ptr()), 2500);
        obs_data_release(rust_owned);
    }
}
