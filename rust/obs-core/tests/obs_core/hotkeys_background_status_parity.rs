//! Tier 3: the original extracted `obs_hotkeys_background_status_changed()`
//! versus the shim (and its `_request` entry) on one recording fixture core
//! (oracle/hotkeys_background_status.c), through the real-named adapters.
//! Compared: the trace of every signal (its handler, its exact name and its
//! NULL calldata) with the nesting of re-entries, and the core and handler
//! left behind, over any sequence of calls with or without a core and a
//! handler, and any script of callbacks that replace or clear the handler,
//! re-enter after a replacement, or remove the core.
//!
//! The C reads `obs` and the handler again for the argument; the shim reads
//! them once through its adapter. With no callback between those reads in
//! the C, the difference is not observable and the trace omits core reads.
//! No intentional differences. Mutants show the probe catches a cached
//! handler, a re-read after the signal, a wrong name, a signal without a
//! handler, a doubled signal and a suppressed re-entry.
use core::cell::Cell;
use core::ffi::{CStr, c_char, c_void};
use core::ptr::null_mut;
use obs_core::ffi::hotkeys_background_status::{
    obs_hotkeys_background_status_changed, obs_hotkeys_background_status_request,
};
use proptest::prelude::*;

type Notify = extern "C" fn();
unsafe extern "C" {
    fn oracle_hotkeys_background_status_probe(
        candidate: Notify,
        calls: *const u8,
        count: usize,
        script: *const u8,
        script_len: usize,
    ) -> u32;
    fn oracle_hotkeys_background_status_original(
        calls: *const u8,
        count: usize,
        script: *const u8,
        script_len: usize,
        trace: *mut c_char,
        trace_size: usize,
    );
    fn oracle_hotkeys_background_status_adapters() -> bool;
    fn obs_hotkeys_background_status_signals() -> *mut c_void;
    fn obs_hotkeys_background_status_signal(handler: *mut c_void, name: *const c_char);
}

/// A call with the core present.
const CORE: u8 = 0x80;
const REMOVE_CORE: u8 = 5;
const CLEAR_HANDLER: u8 = 6;
/// Install handler `n` (1..=4) without re-entering.
const fn replace(n: u8) -> u8 {
    6 + n
}

fn probe(candidate: Notify, calls: &[u8], script: &[u8]) -> u32 {
    // SAFETY: the oracle owns the fixture core every adapter reaches; both
    // slices outlive the call.
    unsafe {
        oracle_hotkeys_background_status_probe(
            candidate,
            calls.as_ptr(),
            calls.len(),
            script.as_ptr(),
            script.len(),
        )
    }
}

/// The original's trace: `|` starts a call, a digit is a signal to that
/// handler, `(` `)` bracket a re-entry.
fn original(calls: &[u8], script: &[u8]) -> String {
    let mut trace = [0 as c_char; 128];
    // SAFETY: the buffer is writable for its size; the oracle writes
    // NUL-terminated text.
    unsafe {
        oracle_hotkeys_background_status_original(
            calls.as_ptr(),
            calls.len(),
            script.as_ptr(),
            script.len(),
            trace.as_mut_ptr(),
            trace.len(),
        );
    }
    // SAFETY: NUL-terminated by the oracle.
    unsafe { CStr::from_ptr(trace.as_ptr()) }
        .to_str()
        .unwrap()
        .to_owned()
}

#[test]
fn real_header_adapters() {
    // SAFETY: the oracle checks the adapters on its own fixture.
    assert!(unsafe { oracle_hotkeys_background_status_adapters() });
}

#[test]
fn original_matches_the_characterization() {
    // No core, then a core without a handler: silent.
    assert_eq!(original(&[0, CORE], &[]), "||");
    // The handler current at each call: twice, replaced, cleared.
    assert_eq!(
        original(&[CORE | 1, CORE | 1, CORE | 2, CORE], &[]),
        "|1|1|2|"
    );
    // Re-entry after a replacement reaches the new handler.
    assert_eq!(original(&[CORE | 1], &[2]), "|1(2)");
    // Removing the core in the callback: that signal, then silence.
    assert_eq!(original(&[CORE | 2], &[REMOVE_CORE]), "|2");
    // A replacement without re-entry is seen only by the next call.
    assert_eq!(original(&[CORE | 1], &[replace(3)]), "|1");
}

const CALLS: [u8; 5] = [0, 1, CORE, CORE | 1, CORE | 2];
const ACTIONS: [u8; 9] = [
    0,
    1,
    2,
    REMOVE_CORE,
    CLEAR_HANDLER,
    replace(1),
    replace(2),
    replace(3),
    3,
];

fn sequences(alphabet: &[u8], max: usize) -> Vec<Vec<u8>> {
    let mut all = vec![Vec::new()];
    let mut layer = vec![Vec::new()];
    for _ in 0..max {
        layer = layer
            .iter()
            .flat_map(|prefix: &Vec<u8>| {
                alphabet.iter().map(move |&value| {
                    let mut next = prefix.clone();
                    next.push(value);
                    next
                })
            })
            .collect();
        all.extend(layer.iter().cloned());
    }
    all
}

#[test]
fn characterized_edges() {
    let scripts = sequences(&ACTIONS, 2);
    for calls in sequences(&CALLS, 3) {
        for script in &scripts {
            for (name, candidate) in [
                ("shim", obs_hotkeys_background_status_changed as Notify),
                ("request", obs_hotkeys_background_status_request),
            ] {
                assert_eq!(
                    probe(candidate, &calls, script),
                    0,
                    "{name} {calls:?} {script:?}"
                );
            }
        }
    }
}

fn call() -> impl Strategy<Value = u8> {
    (any::<bool>(), 0..5u8).prop_map(|(core, handler)| if core { CORE | handler } else { handler })
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(512), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn any_script_matches_c(calls in prop::collection::vec(call(), 0..8),
        script in prop::collection::vec(0..=10u8, 0..=16)) {
        prop_assert_eq!(probe(obs_hotkeys_background_status_changed, &calls, &script), 0);
    }
}

// ---------------------------------------------------------------------------
// mutants, over the same adapters

fn signals() -> *mut c_void {
    // SAFETY: mutants run only inside the probe, on its fixture core.
    unsafe { obs_hotkeys_background_status_signals() }
}

fn signal(handler: *mut c_void, name: &CStr) {
    // SAFETY: as in `signals`; the name is NUL-terminated.
    unsafe { obs_hotkeys_background_status_signal(handler, name.as_ptr()) }
}

const NAME: &CStr = c"hotkey_background_status_changed";

extern "C" fn unbroken() {
    let handler = signals();
    if !handler.is_null() {
        signal(handler, NAME);
    }
}

thread_local! {
    static CACHE: Cell<*mut c_void> = const { Cell::new(null_mut()) };
    static BUSY: Cell<bool> = const { Cell::new(false) };
}

extern "C" fn cached_handler() {
    let mut handler = CACHE.get();
    if handler.is_null() {
        handler = signals();
        CACHE.set(handler);
    }
    if !handler.is_null() {
        signal(handler, NAME);
    }
}

extern "C" fn reread_after_signal() {
    let handler = signals();
    if !handler.is_null() {
        signal(handler, NAME);
        let again = signals();
        if !again.is_null() && again != handler {
            signal(again, NAME);
        }
    }
}

extern "C" fn wrong_name() {
    let handler = signals();
    if !handler.is_null() {
        signal(handler, c"hotkey_background_status_change");
    }
}

extern "C" fn without_handler() {
    signal(signals(), NAME);
}

extern "C" fn doubled() {
    let handler = signals();
    if !handler.is_null() {
        signal(handler, NAME);
        signal(handler, NAME);
    }
}

extern "C" fn no_reentry() {
    if BUSY.get() {
        return;
    }
    BUSY.set(true);
    unbroken();
    BUSY.set(false);
}

#[test]
fn mutants_are_caught() {
    let scripts = sequences(&ACTIONS, 2);
    let sweep = |candidate: Notify| {
        let mut diff = 0;
        for calls in sequences(&CALLS, 2) {
            for script in &scripts {
                diff |= probe(candidate, &calls, script);
            }
        }
        diff
    };
    // The unbroken model of the steps passes the same sweep.
    assert_eq!(sweep(unbroken), 0);
    for (name, mutant) in [
        ("cached_handler", cached_handler as Notify),
        ("reread_after_signal", reread_after_signal),
        ("wrong_name", wrong_name),
        ("without_handler", without_handler),
        ("doubled", doubled),
        ("no_reentry", no_reentry),
    ] {
        assert_ne!(sweep(mutant), 0, "{name} survived");
    }
}
