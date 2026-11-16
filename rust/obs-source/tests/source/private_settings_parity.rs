//! Tier 3: the original C getter, the production adapter and the Rust ABI over
//! real-header source fixtures with live data reference counts. No intentional
//! behavior differences or excluded inputs; no mirrored layouts.
use core::ffi::c_void;
use obs_data::ffi::obs_data::{
    obs_data_create, obs_data_get_int, obs_data_release, obs_data_set_int, ref_count_for_tests,
};
use obs_source::ffi::source_private_settings::obs_source_get_private_settings;
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type Getter = unsafe extern "C" fn(*mut c_void) -> *mut c_void;
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);
unsafe extern "C" {
    fn oracle_source_private_settings_new(
        private_settings: *mut c_void,
        settings: *mut c_void,
        flags: u32,
    ) -> *mut c_void;
    fn oracle_source_private_settings_unchanged(source: *const c_void) -> bool;
    fn oracle_source_private_settings_read(source: *const c_void) -> *mut c_void;
    fn oracle_source_private_settings_free(source: *mut c_void);
    fn oracle_obs_source_get_private_settings(source: *mut c_void) -> *mut c_void;
    fn oracle_source_private_settings_logs(getter: Getter, get: GetLog, set: SetLog) -> bool;
}

struct Fixture {
    source: *mut c_void,
    private_settings: *mut c_void,
    settings: *mut c_void,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: the fixture and the initial reference of each object are owned here.
        unsafe {
            oracle_source_private_settings_free(self.source);
            obs_data_release(self.private_settings);
            obs_data_release(self.settings);
        }
    }
}

fn refs(data: *mut c_void) -> usize {
    // SAFETY: NULL or a live object owned by the fixture.
    unsafe { ref_count_for_tests(data) }
}

fn check(present: bool, flags: u32, value: i64, extra_refs: u8) {
    // SAFETY: each fixture owns a source with the real C layout, a live
    // private-settings object or NULL and live ordinary settings. Each getter
    // adds one reference, released before the next call.
    unsafe {
        let private_settings = if present {
            obs_data_create()
        } else {
            core::ptr::null_mut()
        };
        let settings = obs_data_create();
        assert!(!settings.is_null() && (!present || !private_settings.is_null()));
        obs_data_set_int(private_settings, c"value".as_ptr(), value);
        obs_data_set_int(settings, c"value".as_ptr(), 91);
        let fixture = Fixture {
            source: oracle_source_private_settings_new(private_settings, settings, flags),
            private_settings,
            settings,
        };
        assert!(!fixture.source.is_null());
        // The production adapter reads exactly the stored field.
        assert_eq!(
            oracle_source_private_settings_read(fixture.source),
            private_settings
        );
        let mut held = Vec::new();
        for _ in 0..extra_refs {
            held.push(obs_source_get_private_settings(fixture.source));
        }
        assert_eq!(
            refs(private_settings),
            usize::from(present) * (1 + usize::from(extra_refs))
        );
        for getter in [
            obs_source_get_private_settings as Getter,
            oracle_obs_source_get_private_settings as Getter,
        ] {
            let before = refs(private_settings);
            let result = getter(fixture.source);
            assert_eq!(result, private_settings);
            assert_eq!(refs(private_settings), before + usize::from(present));
            assert_eq!(refs(settings), 1);
            assert_eq!(
                obs_data_get_int(result, c"value".as_ptr()),
                if present { value } else { 0 }
            );
            assert!(oracle_source_private_settings_unchanged(fixture.source));
            obs_data_release(result);
            assert_eq!(refs(private_settings), before);
        }
        for data in held {
            obs_data_release(data);
        }
        assert_eq!(refs(private_settings), usize::from(present));
        assert_eq!(obs_data_get_int(settings, c"value".as_ptr()), 91);
    }
}

#[test]
fn real_header_identity_reference_and_state() {
    for present in [false, true] {
        for flags in [0, u32::MAX] {
            for value in [i64::MIN, 0, i64::MAX] {
                for extra_refs in [0, 1, 4] {
                    check(present, flags, value, extra_refs);
                }
            }
        }
    }
}

#[test]
fn null_diagnostics_match_c() {
    if !crate::output::isolated(concat!(module_path!(), "::null_diagnostics_match_c")) {
        return;
    }
    // SAFETY: the isolated child owns the process-global log handler; C restores it.
    unsafe {
        assert!(oracle_source_private_settings_logs(
            obs_source_get_private_settings,
            base_get_log_handler,
            base_set_log_handler
        ));
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(128), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn arbitrary_private_settings_match_c(present in any::<bool>(), flags in any::<u32>(), value in any::<i64>(), extra_refs in 0u8..8) {
        check(present, flags, value, extra_refs);
    }
}
