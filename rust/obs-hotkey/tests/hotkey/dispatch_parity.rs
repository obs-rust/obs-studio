//! Tier 3: the original extracted C dispatch versus the shim on one recording
//! core, through the same real-header adapters
//! (`obs-hotkey-dispatch-fields.c.h`). Compares the full trace: every core
//! read, the callback or router call with its pressed value, a wrong data,
//! id or hotkey argument, and any change to the core or the hotkey. No
//! intentional differences. Mutants show the probe catches each broken
//! contract.
use core::ffi::{CStr, c_char};
use core::mem::size_of;
use obs_hotkey::ffi::binding_press::obs_hotkey_t;
use obs_hotkey::ffi::dispatch::obs_hotkey_dispatch;
use proptest::prelude::*;

type DispatchFn = unsafe extern "C" fn(*mut obs_hotkey_t, bool);
unsafe extern "C" {
    fn oracle_hotkey_dispatch_probe(bits: u32, id: usize, candidate: DispatchFn) -> u32;
    fn oracle_hotkey_dispatch_trace(
        bits: u32,
        id: usize,
        candidate: Option<DispatchFn>,
        out: *mut c_char,
        size: usize,
    );
    fn oracle_hotkey_dispatch_layout(out: *mut usize);
    fn obs_hotkey_dispatch_rerouting() -> bool;
    fn obs_hotkey_dispatch_has_router() -> bool;
    fn obs_hotkey_dispatch_call(hotkey: *mut obs_hotkey_t, pressed: bool);
    fn obs_hotkey_dispatch_route(hotkey: *mut obs_hotkey_t, pressed: bool);
}

/// Bits: 1 rerouting on, 2 a router set, 4 a press, 8 a NULL hotkey.
const REROUTE: u32 = 1;
const ROUTER: u32 = 2;
const PRESS: u32 = 4;
const NULL_KEY: u32 = 8;

/// Every fixture the original runs safely: a NULL hotkey only where it is
/// never touched (rerouting on, no router).
fn safe_bits() -> impl Iterator<Item = u32> {
    (0..16).filter(|bits| bits & NULL_KEY == 0 || bits & (REROUTE | ROUTER) == REROUTE)
}

fn probe(bits: u32, id: usize, candidate: DispatchFn) -> u32 {
    // SAFETY: the oracle owns the core and the hotkey and passes the
    // candidate NULL only where the original does not touch it.
    unsafe { oracle_hotkey_dispatch_probe(bits, id, candidate) }
}

fn trace(bits: u32, id: usize, candidate: Option<DispatchFn>) -> String {
    let mut out = [0 as c_char; 64];
    // SAFETY: the oracle writes a terminated trace of at most 63 bytes.
    unsafe {
        oracle_hotkey_dispatch_trace(bits, id, candidate, out.as_mut_ptr(), out.len());
        CStr::from_ptr(out.as_ptr()).to_str().unwrap().to_owned()
    }
}

#[test]
fn real_header_layout() {
    let mut layout = [0; 3];
    // SAFETY: the oracle writes three size_t measurements.
    unsafe { oracle_hotkey_dispatch_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [size_of::<usize>(), size_of::<bool>(), size_of::<bool>()]
    );
}

/// The exact traces: `o` a core read, `C`/`c` the callback, `R`/`r` the
/// router, for a press/release.
#[test]
fn exact_traces() {
    let shim: Option<DispatchFn> = Some(obs_hotkey_dispatch);
    for candidate in [None, shim] {
        assert_eq!(trace(PRESS, 42, candidate), "oC");
        assert_eq!(trace(0, 42, candidate), "oc");
        // An installed router is ignored while rerouting is off.
        assert_eq!(trace(ROUTER | PRESS, 42, candidate), "oC");
        assert_eq!(trace(REROUTE | ROUTER | PRESS, 42, candidate), "ooooR");
        assert_eq!(trace(REROUTE | ROUTER, usize::MAX, candidate), "oooor");
        assert_eq!(trace(REROUTE | PRESS, 42, candidate), "oo");
        assert_eq!(trace(REROUTE | NULL_KEY, 42, candidate), "oo");
    }
}

#[test]
fn every_fixture_matches() {
    for bits in safe_bits() {
        for id in [0, 1, 42, usize::MAX - 1, usize::MAX] {
            assert_eq!(
                probe(bits, id, obs_hotkey_dispatch),
                0,
                "bits={bits:#x} id={id}"
            );
        }
    }
}

proptest! {
    #[test]
    fn random_ids_match(bits in 0u32..16, id in any::<usize>()) {
        prop_assume!(safe_bits().any(|safe| safe == bits));
        prop_assert_eq!(probe(bits, id, obs_hotkey_dispatch), 0);
    }
}

/// A broken dispatch over the same real adapters.
#[derive(Clone, Copy)]
enum Mutant {
    AlwaysCall,
    PreferRouter,
    CallInsteadOfRoute,
    CallWithoutRouter,
    FlipPressed,
    Twice,
    Drop,
}

thread_local! {
    static MUTANT: core::cell::Cell<Mutant> = const { core::cell::Cell::new(Mutant::Drop) };
}

unsafe extern "C" fn mutant(hotkey: *mut obs_hotkey_t, pressed: bool) {
    // SAFETY: the oracle's armed core and hotkey back every adapter; no
    // mutant calls the router unless it is set.
    unsafe {
        let reroute = obs_hotkey_dispatch_rerouting();
        match MUTANT.with(core::cell::Cell::get) {
            Mutant::AlwaysCall => obs_hotkey_dispatch_call(hotkey, pressed),
            Mutant::PreferRouter => {
                if obs_hotkey_dispatch_has_router() {
                    obs_hotkey_dispatch_route(hotkey, pressed);
                } else if !reroute {
                    obs_hotkey_dispatch_call(hotkey, pressed);
                }
            }
            Mutant::CallInsteadOfRoute => {
                if !reroute || obs_hotkey_dispatch_has_router() {
                    obs_hotkey_dispatch_call(hotkey, pressed);
                }
            }
            Mutant::CallWithoutRouter => {
                if !reroute {
                    obs_hotkey_dispatch_call(hotkey, pressed);
                } else if obs_hotkey_dispatch_has_router() {
                    obs_hotkey_dispatch_route(hotkey, pressed);
                } else if !hotkey.is_null() {
                    obs_hotkey_dispatch_call(hotkey, pressed);
                }
            }
            Mutant::FlipPressed => {
                if !reroute {
                    obs_hotkey_dispatch_call(hotkey, !pressed);
                } else if obs_hotkey_dispatch_has_router() {
                    obs_hotkey_dispatch_route(hotkey, !pressed);
                }
            }
            Mutant::Twice => {
                obs_hotkey_dispatch(hotkey, pressed);
                obs_hotkey_dispatch(hotkey, pressed);
            }
            Mutant::Drop => {}
        }
    }
}

#[test]
fn mutants_are_caught() {
    for m in [
        Mutant::AlwaysCall,
        Mutant::PreferRouter,
        Mutant::CallInsteadOfRoute,
        Mutant::CallWithoutRouter,
        Mutant::FlipPressed,
        Mutant::Twice,
        Mutant::Drop,
    ] {
        MUTANT.with(|c| c.set(m));
        // AlwaysCall would dereference a NULL hotkey; skip those fixtures
        // for it.
        let caught = safe_bits()
            .filter(|bits| bits & NULL_KEY == 0)
            .any(|bits| probe(bits, 42, mutant) != 0);
        assert!(caught, "a mutant survived");
    }
}
