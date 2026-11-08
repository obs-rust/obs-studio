//! Tier 2 real-header layout and Tier 3 atomic state and diagnostic parity.
//! No input exclusions or intentional differences.
use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use obs_output::ffi::encoder_state_queries::{
    Fields, obs_encoder_active as active, obs_encoder_paused as paused,
};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type Getter = unsafe extern "C" fn(*const c_void) -> bool;
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);
unsafe extern "C" {
    fn oracle_encoder_state_queries_new(
        active: bool,
        paused: bool,
        initialized: bool,
    ) -> *mut c_void;
    fn oracle_encoder_state_queries_free(output: *mut c_void);
    fn oracle_encoder_state_queries_unchanged(output: *const c_void) -> bool;
    fn oracle_encoder_state_queries_addresses(output: *const c_void) -> bool;
    fn oracle_encoder_state_queries_layout(layout: *mut usize);
    fn oracle_obs_encoder_active(output: *const c_void) -> bool;
    fn oracle_obs_encoder_paused(output: *const c_void) -> bool;
    fn oracle_encoder_state_queries_logs(
        active: Getter,
        paused: Getter,
        get: GetLog,
        set: SetLog,
    ) -> bool;
}

struct Fixture(*mut c_void);
impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: owns the C allocation; borrowed metadata is still live here.
        unsafe { oracle_encoder_state_queries_free(self.0) }
    }
}

#[test]
fn real_header_layout() {
    let mut layout = [0; 6];
    // SAFETY: the oracle writes exactly six size/offset values.
    unsafe { oracle_encoder_state_queries_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            offset_of!(Fields, active),
            offset_of!(Fields, paused),
            size_of::<bool>(),
            align_of::<bool>()
        ]
    );
}

fn check(a: bool, p: bool, initialized: bool) {
    // SAFETY: the oracle allocates and initializes a real C encoder fixture.
    let fixture = Fixture(unsafe { oracle_encoder_state_queries_new(a, p, initialized) });
    assert!(!fixture.0.is_null());
    // SAFETY: fixture remains live, and no NULL reaches a member-address accessor.
    unsafe {
        assert!(oracle_encoder_state_queries_addresses(fixture.0));
        for _ in 0..2 {
            assert_eq!(active(fixture.0), oracle_obs_encoder_active(fixture.0));
            assert_eq!(paused(fixture.0), oracle_obs_encoder_paused(fixture.0));
            assert_eq!(active(fixture.0), a);
            assert_eq!(paused(fixture.0), p);
            assert!(oracle_encoder_state_queries_unchanged(fixture.0));
        }
    }
}

#[test]
fn characterized_edges() {
    for active in [false, true] {
        for paused in [false, true] {
            for initialized in [false, true] {
                check(active, paused, initialized);
            }
        }
    }
}

#[test]
fn null_diagnostics_match_c() {
    if !super::isolated(concat!(module_path!(), "::null_diagnostics_match_c")) {
        return;
    }
    // SAFETY: the isolated child exclusively owns the log handler; callbacks
    // use the base shim's opaque handler-pointer ABI, and the probe restores it.
    unsafe {
        assert!(oracle_encoder_state_queries_logs(
            active,
            paused,
            base_get_log_handler,
            base_set_log_handler
        ));
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn independent_states_match_c(active in any::<bool>(), paused in any::<bool>(), initialized in any::<bool>()) {
        check(active, paused, initialized);
    }
}
