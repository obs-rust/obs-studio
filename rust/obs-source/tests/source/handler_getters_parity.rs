//! Real-header layout and Tier 3 parity for #4073; no input exclusions.
use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use obs_source::ffi::source_handler_getters::{Fields, obs_source_get_proc_handler as proc};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type Getter = unsafe extern "C" fn(*const c_void) -> *mut c_void;
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);
unsafe extern "C" {
    fn oracle_source_handlers_new(signal: bool, proc: bool, same: bool, flags: u32) -> *mut c_void;
    fn oracle_source_handlers_tokens(source: *mut c_void, tokens: *mut *mut c_void);
    fn oracle_source_handlers_free(source: *mut c_void);
    fn oracle_source_handlers_unchanged(source: *const c_void) -> bool;
    fn oracle_source_handlers_addresses(source: *const c_void) -> bool;
    fn oracle_source_handlers_layout(layout: *mut usize);
    fn oracle_obs_source_get_proc_handler(source: *const c_void) -> *mut c_void;
    fn oracle_source_handlers_logs(proc: Getter, get: GetLog, set: SetLog) -> bool;
}

struct Fixture(*mut c_void);
impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: owns the C allocation; handler tokens are owned by this fixture.
        unsafe { oracle_source_handlers_free(self.0) }
    }
}

#[test]
fn real_header_layout() {
    let mut layout = [0; 5];
    // SAFETY: the oracle writes exactly five size/alignment/offset values.
    unsafe { oracle_source_handlers_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            offset_of!(Fields, proc),
            size_of::<*mut c_void>(),
            align_of::<*mut c_void>()
        ]
    );
}

fn check(present_signal: bool, present_proc: bool, same: bool, flags: u32) {
    // SAFETY: the oracle owns its long-double-aligned handler tokens and fixture.
    let fixture =
        Fixture(unsafe { oracle_source_handlers_new(present_signal, present_proc, same, flags) });
    assert!(!fixture.0.is_null());
    let mut tokens = [core::ptr::null_mut(); 2];
    // SAFETY: this live fixture owns both tokens; the oracle writes exactly two addresses.
    unsafe { oracle_source_handlers_tokens(fixture.0, tokens.as_mut_ptr()) };
    let pp = if present_proc {
        tokens[usize::from(!same)]
    } else {
        core::ptr::null_mut()
    };
    // SAFETY: the real-layout fixture is live; only the diagnostic test supplies
    // NULL to public getters, never to a member-address accessor.
    unsafe {
        assert!(oracle_source_handlers_addresses(fixture.0));
        for _ in 0..2 {
            assert_eq!(
                proc(fixture.0),
                oracle_obs_source_get_proc_handler(fixture.0)
            );
            assert_eq!(proc(fixture.0), pp);
            assert!(oracle_source_handlers_unchanged(fixture.0));
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
    if !crate::output::isolated(concat!(module_path!(), "::null_diagnostics_match_c")) {
        return;
    }
    // SAFETY: the isolated child owns the log handler; callbacks use the base
    // shim's opaque handler-pointer ABI and the probe restores the handler.
    unsafe {
        assert!(oracle_source_handlers_logs(
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
