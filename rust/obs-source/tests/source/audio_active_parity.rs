//! Exact parity with the original C. No intentional differences or exclusions.
use core::ffi::c_void;
use core::mem::{align_of, size_of};
use core::sync::atomic::AtomicBool;
use obs_source::ffi::source_audio_active::obs_source_audio_active;
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_source_audio_active_layout(layout: *mut usize);
    fn oracle_source_audio_active_new(bits: u32) -> *mut c_void;
    fn oracle_source_audio_active_free(source: *mut c_void);
    fn oracle_source_audio_active_unchanged(source: *const c_void) -> bool;
    fn oracle_obs_source_audio_active(source: *const c_void) -> bool;
    fn oracle_source_audio_active_null_log(
        getter: unsafe extern "C" fn(*const c_void) -> bool,
    ) -> bool;
}
struct Source(*mut c_void);
impl Drop for Source {
    fn drop(&mut self) {
        // SAFETY: free the exclusively owned fixture exactly once.
        unsafe { oracle_source_audio_active_free(self.0) };
    }
}
fn check(bits: u32) {
    // SAFETY: the oracle allocates a complete, initialized real C source.
    let source = Source(unsafe { oracle_source_audio_active_new(bits) });
    assert!(!source.0.is_null());
    // SAFETY: both queries borrow the live fixture; there are no concurrent writes.
    unsafe {
        for _ in 0..2 {
            assert_eq!(
                obs_source_audio_active(source.0),
                oracle_obs_source_audio_active(source.0)
            );
            assert!(oracle_source_audio_active_unchanged(source.0));
        }
    }
}
#[test]
fn characterized_edges() {
    for bits in 0..16 {
        check(bits);
    }
}
#[test]
fn real_header_atomic_layout() {
    let mut c = [0; 2];
    // SAFETY: oracle writes two size_t values to this array.
    unsafe { oracle_source_audio_active_layout(c.as_mut_ptr()) };
    assert_eq!(c, [size_of::<AtomicBool>(), align_of::<AtomicBool>()]);
    assert_eq!(c, [size_of::<bool>(), align_of::<bool>()]);
}
#[test]
fn null_defaults_and_historical_logs() {
    if std::env::var_os("OBS_SOURCE_AUDIO_ACTIVE_CHILD").is_none() {
        let selector = concat!(module_path!(), "::null_defaults_and_historical_logs")
            .split_once("::")
            .unwrap()
            .1;
        assert!(
            std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", selector, "--nocapture"])
                .env("OBS_SOURCE_AUDIO_ACTIVE_CHILD", "1")
                .status()
                .unwrap()
                .success()
        );
        return;
    }
    // SAFETY: the child probes restore the process-global logger; callbacks accept NULL.
    unsafe {
        assert!(oracle_source_audio_active_null_log(
            oracle_obs_source_audio_active
        ));
        assert!(oracle_source_audio_active_null_log(obs_source_audio_active));
    }
}
proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(64),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn random_fields_match_c(bits in any::<u32>()) { check(bits); }
}
