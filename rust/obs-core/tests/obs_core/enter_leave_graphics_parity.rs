//! Tier 3: the original extracted `obs_enter_graphics()` and
//! `obs_leave_graphics()` versus the shims on one recording fixture core
//! (oracle/enter_leave_graphics.c), through the real-named adapters. A script
//! of calls and graphics changes runs on both; compared: the trace of every
//! `gs_enter_context` (with the graphics it got) and `gs_leave_context`, and
//! that no call changed the core. The core's reads are not traced: the
//! original's enter reads the graphics field twice (the check, then the
//! call) and the shim once, which differs only under a concurrent change. Scripts: no
//! graphics, nested entries, a leave without an entry, the graphics set,
//! cleared or swapped between calls, and any mix of these.
//!
//! Excluded: an uninitialized core, a dead graphics subsystem and the
//! graphics layer itself, outside these functions (unchanged Tier 2 runs the
//! real one). No intentional differences. Mutants show the probe catches each
//! broken step.
use core::ffi::{CStr, c_char};
use obs_core::ffi::enter_leave_graphics::{obs_enter_graphics, obs_leave_graphics};
use proptest::prelude::*;
use std::ffi::CString;

type Call = unsafe extern "C" fn();
unsafe extern "C" {
    fn oracle_enter_leave_graphics_probe(enter: Call, leave: Call, script: *const c_char) -> u32;
    fn oracle_enter_leave_graphics_original(
        script: *const c_char,
        trace: *mut c_char,
        trace_size: usize,
    ) -> bool;
    fn oracle_enter_leave_graphics_adapters() -> bool;
    fn obs_enter_leave_graphics_get() -> *mut core::ffi::c_void;
    fn obs_enter_leave_graphics_enter(graphics: *mut core::ffi::c_void);
    fn obs_enter_leave_graphics_leave();
}

fn probe(enter: Call, leave: Call, script: &str) -> u32 {
    let script = CString::new(script).unwrap();
    // SAFETY: the oracle owns the fixture core every adapter reaches; the
    // script outlives the call.
    unsafe { oracle_enter_leave_graphics_probe(enter, leave, script.as_ptr()) }
}

fn shims(script: &str) -> u32 {
    probe(obs_enter_graphics, obs_leave_graphics, script)
}

/// The original's trace.
fn original(script: &str) -> String {
    let script = CString::new(script).unwrap();
    let mut trace = [0 as c_char; 256];
    // SAFETY: the buffer is writable for its size; the oracle writes
    // NUL-terminated text.
    assert!(unsafe {
        oracle_enter_leave_graphics_original(script.as_ptr(), trace.as_mut_ptr(), trace.len())
    });
    // SAFETY: NUL-terminated by the oracle.
    unsafe { CStr::from_ptr(trace.as_ptr()) }
        .to_str()
        .unwrap()
        .to_owned()
}

#[test]
fn real_adapters_read_the_real_fields() {
    // SAFETY: the oracle arms and clears its own fixture.
    assert!(unsafe { oracle_enter_leave_graphics_adapters() });
}

#[test]
fn original_c_is_the_characterized_forwarding() {
    // No graphics: nothing forwarded.
    assert_eq!(original("EELL"), "");
    // With graphics: enter forwards the core's graphics, leave forwards.
    assert_eq!(original("AEL"), "eAl");
    assert_eq!(original("BEEL"), "eBeBl");
    // A leave without an entry is still forwarded (the layer reports it).
    assert_eq!(original("AL"), "l");
    // The read is per call: a graphics cleared after the entry skips the
    // leave, and a swapped one is the one entered.
    assert_eq!(original("AE0L"), "eA");
    assert_eq!(original("AEBEL"), "eAeBl");
}

#[test]
fn characterized_edges() {
    for script in [
        "", "E", "L", "EL", "AE", "AL", "AEL", "AEELLL", "AE0L", "0EAL", "AEBEL", "BLLEE",
    ] {
        assert_eq!(shims(script), 0, "{script:?}");
    }
}

/// Enter that ignores the graphics field and always forwards.
unsafe extern "C" fn enter_unchecked() {
    // SAFETY: the oracle's fixture adapters.
    unsafe { obs_enter_leave_graphics_enter(obs_enter_leave_graphics_get()) }
}

/// Leave that ignores the graphics field and always forwards.
unsafe extern "C" fn leave_unchecked() {
    // SAFETY: the oracle's fixture adapters.
    unsafe { obs_enter_leave_graphics_leave() }
}

/// Enter that never forwards.
unsafe extern "C" fn enter_nothing() {
    // SAFETY: the oracle's fixture adapter.
    let _ = unsafe { obs_enter_leave_graphics_get() };
}

#[test]
fn mutants_are_caught() {
    assert_ne!(probe(enter_unchecked, obs_leave_graphics, "EL"), 0);
    assert_ne!(probe(obs_enter_graphics, leave_unchecked, "L"), 0);
    assert_ne!(probe(enter_nothing, obs_leave_graphics, "AEL"), 0);
    // Swapped entry points.
    assert_ne!(probe(obs_leave_graphics, obs_enter_graphics, "AEL"), 0);
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn any_script_matches_original_c(script in "[0ABEL]{0,40}") {
        prop_assert_eq!(shims(&script), 0);
    }
}
