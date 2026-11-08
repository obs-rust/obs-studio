//! Exact stored values, NULL logs, lock lifetime and real-header view layout.
use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use obs_source::ffi::source_push_queries::{self as rust, PushQueryFields};
use proptest::prelude::*;
unsafe extern "C" {
    fn oracle_source_push_layout(layout: *mut usize);
    fn oracle_source_push_new(
        mute: bool,
        talk: bool,
        mute_delay: u64,
        talk_delay: u64,
        pressed: bool,
    ) -> *mut c_void;
    fn oracle_source_push_free(source: *mut c_void);
    fn oracle_source_push_unchanged(source: *const c_void) -> bool;
    fn oracle_obs_source_push_to_mute_enabled(source: *mut c_void) -> bool;
    fn oracle_obs_source_push_to_talk_enabled(source: *mut c_void) -> bool;
    fn oracle_obs_source_get_push_to_mute_delay(source: *mut c_void) -> u64;
    fn oracle_obs_source_get_push_to_talk_delay(source: *mut c_void) -> u64;
    fn oracle_source_push_null_log(
        mute: unsafe extern "C" fn(*mut c_void) -> bool,
        talk: unsafe extern "C" fn(*mut c_void) -> bool,
        mute_delay: unsafe extern "C" fn(*mut c_void) -> u64,
        talk_delay: unsafe extern "C" fn(*mut c_void) -> u64,
    ) -> bool;
}
#[test]
fn real_header_view_layout() {
    let mut c = [0; 11];
    // SAFETY: the oracle writes eleven size_t values to this array.
    unsafe { oracle_source_push_layout(c.as_mut_ptr()) };
    assert_eq!(
        c,
        [
            size_of::<PushQueryFields>(),
            align_of::<PushQueryFields>(),
            offset_of!(PushQueryFields, audio_mutex),
            offset_of!(PushQueryFields, mute_enabled),
            offset_of!(PushQueryFields, talk_enabled),
            offset_of!(PushQueryFields, mute_delay),
            offset_of!(PushQueryFields, talk_delay),
            size_of::<bool>(),
            align_of::<bool>(),
            size_of::<u64>(),
            align_of::<u64>()
        ]
    );
}
struct Source(*mut c_void);
impl Drop for Source {
    fn drop(&mut self) {
        // SAFETY: this fixture exclusively owns the oracle allocation and initialized mutex.
        unsafe { oracle_source_push_free(self.0) };
    }
}
fn check(mute: bool, talk: bool, mute_delay: u64, talk_delay: u64, pressed: bool) {
    // SAFETY: the oracle creates the full real C source with a live audio mutex.
    let source =
        Source(unsafe { oracle_source_push_new(mute, talk, mute_delay, talk_delay, pressed) });
    assert!(!source.0.is_null());
    // SAFETY: both implementations borrow this live source, and each releases its mutex.
    unsafe {
        for _ in 0..2 {
            assert_eq!(
                rust::obs_source_push_to_mute_enabled(source.0),
                oracle_obs_source_push_to_mute_enabled(source.0)
            );
            assert!(oracle_source_push_unchanged(source.0));
            assert_eq!(
                rust::obs_source_push_to_talk_enabled(source.0),
                oracle_obs_source_push_to_talk_enabled(source.0)
            );
            assert!(oracle_source_push_unchanged(source.0));
            assert_eq!(
                rust::obs_source_get_push_to_mute_delay(source.0),
                oracle_obs_source_get_push_to_mute_delay(source.0)
            );
            assert!(oracle_source_push_unchanged(source.0));
            assert_eq!(
                rust::obs_source_get_push_to_talk_delay(source.0),
                oracle_obs_source_get_push_to_talk_delay(source.0)
            );
            assert!(oracle_source_push_unchanged(source.0));
        }
    }
}
#[test]
fn characterized_edges() {
    let edges = [0, 1, 0x8000_0000_0000_0000, u64::MAX, 0xa5a5_5a5a_f00f_1234];
    for mute_delay in edges {
        for talk_delay in edges {
            for flags in 0..4 {
                check(
                    flags & 1 != 0,
                    flags & 2 != 0,
                    mute_delay,
                    talk_delay,
                    flags & 1 == 0,
                );
            }
        }
    }
}
#[test]
fn null_defaults_and_historical_logs() {
    if std::env::var_os("OBS_SOURCE_PUSH_QUERIES_CHILD").is_none() {
        let selector = concat!(module_path!(), "::null_defaults_and_historical_logs")
            .split_once("::")
            .unwrap()
            .1;
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", selector, "--nocapture"])
            .env("OBS_SOURCE_PUSH_QUERIES_CHILD", "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
        return;
    }
    // SAFETY: this child exclusively probes the global logger, which the oracle restores.
    unsafe {
        assert!(oracle_source_push_null_log(
            oracle_obs_source_push_to_mute_enabled,
            oracle_obs_source_push_to_talk_enabled,
            oracle_obs_source_get_push_to_mute_delay,
            oracle_obs_source_get_push_to_talk_delay
        ));
        assert!(oracle_source_push_null_log(
            rust::obs_source_push_to_mute_enabled,
            rust::obs_source_push_to_talk_enabled,
            rust::obs_source_get_push_to_mute_delay,
            rust::obs_source_get_push_to_talk_delay
        ));
    }
}
proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(64), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn random_fields_match_c(mute in any::<bool>(), talk in any::<bool>(), mute_delay in any::<u64>(), talk_delay in any::<u64>(), pressed in any::<bool>()) {
        check(mute, talk, mute_delay, talk_delay, pressed);
    }
}
