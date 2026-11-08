//! Exact return values, historical NULL logs and full source-state parity.
use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use obs_source::ffi::source_state_getters::{self as rust, StateGetterFields};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_source_state_layout(layout: *mut usize);
    fn oracle_source_state_new(
        flags: u32,
        enabled: bool,
        version: u32,
        default_flags: u32,
        hidden: bool,
    ) -> *mut c_void;
    fn oracle_source_state_free(source: *mut c_void);
    fn oracle_source_state_unchanged(source: *const c_void) -> bool;
    fn oracle_obs_source_get_flags(source: *const c_void) -> u32;
    fn oracle_obs_source_enabled(source: *const c_void) -> bool;
    fn oracle_obs_source_get_last_obs_version(source: *const c_void) -> u32;
    fn oracle_source_state_null_log(
        flags: unsafe extern "C" fn(*const c_void) -> u32,
        enabled: unsafe extern "C" fn(*const c_void) -> bool,
        version: unsafe extern "C" fn(*const c_void) -> u32,
    ) -> bool;
}

#[test]
fn real_header_view_layout() {
    let mut c = [0; 9];
    // SAFETY: the oracle writes nine size_t values into this array.
    unsafe { oracle_source_state_layout(c.as_mut_ptr()) };
    assert_eq!(
        c,
        [
            size_of::<StateGetterFields>(),
            align_of::<StateGetterFields>(),
            offset_of!(StateGetterFields, flags),
            offset_of!(StateGetterFields, enabled),
            offset_of!(StateGetterFields, last_obs_version),
            size_of::<u32>(),
            align_of::<u32>(),
            size_of::<bool>(),
            align_of::<bool>()
        ]
    );
}

struct Source(*mut c_void);
impl Drop for Source {
    fn drop(&mut self) {
        // SAFETY: this exclusively owned oracle allocation is freed exactly once.
        unsafe { oracle_source_state_free(self.0) };
    }
}
fn check(flags: u32, enabled: bool, version: u32, default_flags: u32, hidden: bool) {
    // SAFETY: the oracle allocates and initializes the complete real C source.
    let source =
        Source(unsafe { oracle_source_state_new(flags, enabled, version, default_flags, hidden) });
    assert!(!source.0.is_null());
    // SAFETY: both implementations borrow the live, exclusively owned fixture.
    unsafe {
        for _ in 0..2 {
            assert_eq!(
                rust::obs_source_get_flags(source.0),
                oracle_obs_source_get_flags(source.0)
            );
            assert!(oracle_source_state_unchanged(source.0));
            assert_eq!(
                rust::obs_source_enabled(source.0),
                oracle_obs_source_enabled(source.0)
            );
            assert!(oracle_source_state_unchanged(source.0));
            assert_eq!(
                rust::obs_source_get_last_obs_version(source.0),
                oracle_obs_source_get_last_obs_version(source.0)
            );
            assert!(oracle_source_state_unchanged(source.0));
        }
    }
}

#[test]
fn characterized_edges() {
    let edges = [0, 1, 0x8000_0000, 0x7fff_ffff, u32::MAX, 0xa5a5_5a5a];
    for flags in edges {
        for version in edges {
            for enabled in [false, true] {
                check(flags, enabled, version, !flags, !enabled);
            }
        }
    }
}

#[test]
fn null_defaults_and_historical_logs() {
    // The logger is global: isolate in the existing Core state executable.
    if std::env::var_os("OBS_SOURCE_STATE_GETTERS_CHILD").is_none() {
        let selector = concat!(module_path!(), "::null_defaults_and_historical_logs")
            .split_once("::")
            .unwrap()
            .1;
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", selector, "--nocapture"])
            .env("OBS_SOURCE_STATE_GETTERS_CHILD", "1")
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    // SAFETY: the isolated probes save and restore the process-global logger.
    unsafe {
        assert!(oracle_source_state_null_log(
            oracle_obs_source_get_flags,
            oracle_obs_source_enabled,
            oracle_obs_source_get_last_obs_version
        ));
        assert!(oracle_source_state_null_log(
            rust::obs_source_get_flags,
            rust::obs_source_enabled,
            rust::obs_source_get_last_obs_version
        ));
    }
}

proptest! {
 #![proptest_config(ProptestConfig {
  cases: obs_c_oracle::parity::cases(64),
  failure_persistence: None,
  ..ProptestConfig::default()
 })]
 #[test]
 fn random_fields_match_c(flags in any::<u32>(), enabled in any::<bool>(), version in any::<u32>(), default_flags in any::<u32>(), hidden in any::<bool>()) {
  check(flags, enabled, version, default_flags, hidden);
 }
}
