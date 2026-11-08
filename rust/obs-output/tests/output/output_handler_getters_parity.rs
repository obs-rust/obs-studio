//! Real-header layout and Tier 3 parity for #4026; no input exclusions.
//! Only oracle symbol prefixes in __func__ diagnostics are normalized.
use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use obs_output::ffi::output_handler_getters::{
    Fields, obs_output_get_proc_handler as proc, obs_output_get_signal_handler as signal,
};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type Getter = unsafe extern "C" fn(*const c_void) -> *mut c_void;
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);
unsafe extern "C" {
    fn oracle_output_handlers_new(signal: bool, proc: bool, same: bool, flags: u32) -> *mut c_void;
    fn oracle_output_handlers_tokens(output: *mut c_void, tokens: *mut *mut c_void);
    fn oracle_output_handlers_free(output: *mut c_void);
    fn oracle_output_handlers_unchanged(output: *const c_void) -> bool;
    fn oracle_output_handlers_addresses(output: *const c_void) -> bool;
    fn oracle_output_handlers_layout(layout: *mut usize);
    fn oracle_obs_output_get_signal_handler(output: *const c_void) -> *mut c_void;
    fn oracle_obs_output_get_proc_handler(output: *const c_void) -> *mut c_void;
    fn oracle_output_handlers_logs(signal: Getter, proc: Getter, get: GetLog, set: SetLog) -> bool;
}

struct Fixture(*mut c_void);
impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: owns the C allocation; handler tokens are owned by this fixture.
        unsafe { oracle_output_handlers_free(self.0) }
    }
}

#[test]
fn real_header_layout() {
    let mut layout = [0; 8];
    // SAFETY: the oracle writes exactly eight size/alignment/offset values.
    unsafe { oracle_output_handlers_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            offset_of!(Fields, signal),
            offset_of!(Fields, proc),
            size_of::<*mut c_void>(),
            align_of::<*mut c_void>(),
            size_of::<*mut c_void>(),
            align_of::<*mut c_void>()
        ]
    );
}

fn check(present_signal: bool, present_proc: bool, same: bool, flags: u32) {
    // SAFETY: the oracle owns its max_align_t-aligned handler tokens and fixture.
    let fixture =
        Fixture(unsafe { oracle_output_handlers_new(present_signal, present_proc, same, flags) });
    assert!(!fixture.0.is_null());
    let mut tokens = [core::ptr::null_mut(); 2];
    // SAFETY: this live fixture owns both tokens; the oracle writes exactly two addresses.
    unsafe { oracle_output_handlers_tokens(fixture.0, tokens.as_mut_ptr()) };
    let sp = if present_signal {
        tokens[0]
    } else {
        core::ptr::null_mut()
    };
    let pp = if present_proc {
        tokens[usize::from(!same)]
    } else {
        core::ptr::null_mut()
    };
    // SAFETY: the real-layout fixture is live; only the diagnostic test supplies
    // NULL to public getters, never to a member-address accessor.
    unsafe {
        assert!(oracle_output_handlers_addresses(fixture.0));
        for _ in 0..2 {
            assert_eq!(
                signal(fixture.0),
                oracle_obs_output_get_signal_handler(fixture.0)
            );
            assert_eq!(
                proc(fixture.0),
                oracle_obs_output_get_proc_handler(fixture.0)
            );
            assert_eq!(signal(fixture.0), sp);
            assert_eq!(proc(fixture.0), pp);
            assert!(oracle_output_handlers_unchanged(fixture.0));
        }
    }
}

#[test]
fn characterized_edges() {
    for s in [false, true] {
        for p in [false, true] {
            for same in [false, true] {
                for flags in [0, u32::MAX] {
                    check(s, p, same, flags);
                }
            }
        }
    }
}

#[test]
fn null_diagnostics_match_c() {
    if !super::isolated(concat!(module_path!(), "::null_diagnostics_match_c")) {
        return;
    }
    // SAFETY: the isolated child owns the log handler; callbacks use the base
    // shim's opaque handler-pointer ABI and the probe restores the handler.
    unsafe {
        assert!(oracle_output_handlers_logs(
            signal,
            proc,
            base_get_log_handler,
            base_set_log_handler
        ));
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn arbitrary_handlers_match_c(s in any::<bool>(), p in any::<bool>(), same in any::<bool>(), flags in any::<u32>()) {
        check(s, p, same, flags);
    }
}
