//! Tier 3: original C getter, production adapter and real-header source fixtures.
//! No intentional behavior differences or excluded inputs; no mirrored layouts.
use core::ffi::c_void;
use obs_data::ffi::obs_data::{
    obs_data_create, obs_data_get_int, obs_data_release, obs_data_set_int, ref_count_for_tests,
};
use obs_source::ffi::source_settings_getter::obs_source_get_settings;
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type Getter = unsafe extern "C" fn(*const c_void) -> *mut c_void;
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);
unsafe extern "C" {
    fn oracle_source_settings_new(settings: *mut c_void, flags: u32) -> *mut c_void;
    fn oracle_source_settings_unchanged(source: *const c_void) -> bool;
    fn oracle_source_settings_free(source: *mut c_void);
    fn oracle_obs_source_get_settings(source: *const c_void) -> *mut c_void;
    fn oracle_source_settings_logs(getter: Getter, get: GetLog, set: SetLog) -> bool;
}

struct Fixture {
    source: *mut c_void,
    settings: *mut c_void,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: fixture and its initial settings reference are owned by this guard.
        unsafe {
            oracle_source_settings_free(self.source);
            obs_data_release(self.settings);
        }
    }
}

fn check(present: bool, flags: u32, value: i64, extra_refs: u8) {
    // SAFETY: each fixture owns a source with the real C layout and a live settings
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
        obs_data_set_int(settings, c"crop".as_ptr(), value);
        let fixture = Fixture {
            source: oracle_source_settings_new(settings, flags),
            settings,
        };
        assert!(!fixture.source.is_null());
        let mut held = Vec::new();
        for _ in 0..extra_refs {
            held.push(obs_source_get_settings(fixture.source));
        }
        for getter in [
            obs_source_get_settings as Getter,
            oracle_obs_source_get_settings as Getter,
        ] {
            let before = ref_count_for_tests(settings);
            let result = getter(fixture.source);
            assert_eq!(result, settings);
            assert_eq!(ref_count_for_tests(settings), before + usize::from(present));
            assert_eq!(
                obs_data_get_int(result, c"crop".as_ptr()),
                if present { value } else { 0 }
            );
            assert!(oracle_source_settings_unchanged(fixture.source));
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
        assert!(oracle_source_settings_logs(
            obs_source_get_settings,
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
