//! Real-header layout and exact differential parity, with no input exclusions.
use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use obs_output::ffi::output_pause_queries::{
    Fields, obs_output_get_pause_offset as offset, obs_output_paused as paused,
};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type Paused = unsafe extern "C" fn(*const c_void) -> bool;
type Offset = unsafe extern "C" fn(*mut c_void) -> u64;
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);
unsafe extern "C" {
    fn oracle_output_pause_queries_new(paused: bool, offset: u64) -> *mut c_void;
    fn oracle_output_pause_queries_free(output: *mut c_void);
    fn oracle_output_pause_queries_unchanged(output: *const c_void) -> bool;
    fn oracle_output_pause_queries_addresses(output: *mut c_void) -> bool;
    fn oracle_output_pause_queries_layout(layout: *mut usize);
    fn oracle_output_pause_queries_observe(getter: Offset) -> bool;
    fn oracle_obs_output_paused(output: *const c_void) -> bool;
    fn oracle_obs_output_get_pause_offset(output: *mut c_void) -> u64;
    fn oracle_output_pause_queries_logs(p: Paused, o: Offset, get: GetLog, set: SetLog) -> bool;
}

struct Fixture(*mut c_void);
impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: owns the C allocation and its initialized mutex.
        unsafe { oracle_output_pause_queries_free(self.0) }
    }
}

#[test]
fn real_header_layout() {
    let mut layout = [0; 9];
    // SAFETY: the oracle writes exactly nine size, alignment and offset values.
    unsafe { oracle_output_pause_queries_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            offset_of!(Fields, paused),
            offset_of!(Fields, offset),
            offset_of!(Fields, mutex),
            size_of::<bool>(),
            align_of::<bool>(),
            size_of::<u64>(),
            align_of::<u64>()
        ]
    );
}

fn check(p: bool, value: u64) {
    // SAFETY: C allocates a real output fixture and initializes its mutex.
    let fixture = Fixture(unsafe { oracle_output_pause_queries_new(p, value) });
    assert!(!fixture.0.is_null());
    // SAFETY: fixture is live, and no NULL reaches a member-address accessor.
    unsafe {
        assert!(oracle_output_pause_queries_addresses(fixture.0));
        for _ in 0..2 {
            assert_eq!(paused(fixture.0), oracle_obs_output_paused(fixture.0));
            assert_eq!(
                offset(fixture.0),
                oracle_obs_output_get_pause_offset(fixture.0)
            );
            assert_eq!(paused(fixture.0), p);
            assert_eq!(offset(fixture.0), value);
            assert!(oracle_output_pause_queries_unchanged(fixture.0));
        }
    }
}

#[test]
fn characterized_edges() {
    for p in [false, true] {
        for value in [0, 1, u32::MAX as u64, 1 << 63, u64::MAX] {
            check(p, value);
        }
    }
}

#[test]
fn offset_is_read_between_matching_lock_and_unlock() {
    // SAFETY: the oracle owns a live fixture. Its lock/unlock probes change the
    // offset to different sentinels, so an unlocked read cannot return 42.
    unsafe {
        assert!(oracle_output_pause_queries_observe(
            oracle_obs_output_get_pause_offset
        ));
        assert!(oracle_output_pause_queries_observe(offset));
    }
}

#[test]
fn null_diagnostics_match_c() {
    if !super::isolated(concat!(module_path!(), "::null_diagnostics_match_c")) {
        return;
    }
    // SAFETY: the isolated child exclusively owns the log handler; the C probe
    // calls the supplied production callbacks and restores the old handler.
    unsafe {
        assert!(oracle_output_pause_queries_logs(
            paused,
            offset,
            base_get_log_handler,
            base_set_log_handler
        ));
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn independent_states_match_c(p in any::<bool>(), value in any::<u64>()) {
        check(p, value);
    }
}
