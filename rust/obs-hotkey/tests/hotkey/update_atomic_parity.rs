//! Exact trace parity with the original C body. Null callbacks are exercised
//! only on failed locking; invoking a null callback is outside the C contract.
use core::ffi::c_void;
use obs_hotkey::ffi::update_atomic::{Callback, obs_hotkey_update_atomic};
use proptest::prelude::*;
use std::cell::Cell;

unsafe extern "C" {
    fn oracle_obs_hotkey_update_atomic(callback: Callback, data: *mut c_void);
    fn oracle_hotkey_atomic_reset(initialized: bool, data: *mut c_void);
    fn oracle_hotkey_atomic_lock() -> bool;
    fn oracle_hotkey_atomic_unlock();
    fn oracle_hotkey_atomic_callback(data: *mut c_void) -> usize;
    fn oracle_hotkey_atomic_trace(out: *mut usize) -> usize;
}

thread_local! {
    static ACTIVE: Cell<bool> = const { Cell::new(false) };
}

// The existing save/load module owns the category's registry seam symbols.
// Select this instrumented C lock only on the thread running atomic parity.
pub(super) fn lock() -> Option<bool> {
    ACTIVE.with(Cell::get).then(|| {
        // SAFETY: this thread installed and reset the instrumented C registry.
        unsafe { oracle_hotkey_atomic_lock() }
    })
}

pub(super) fn unlock() -> bool {
    if !ACTIVE.with(Cell::get) {
        return false;
    }
    // SAFETY: the caller releases one successful recursive lock on this thread.
    unsafe { oracle_hotkey_atomic_unlock() };
    true
}

struct Data {
    rust: bool,
    remaining: u8,
    calls: usize,
    value: u64,
    depth_sum: usize,
}

unsafe extern "C" fn callback(data: *mut c_void) {
    // SAFETY: run supplies a live Data through every synchronous nested call.
    let state = unsafe { &mut *data.cast::<Data>() };
    state.calls += 1;
    // SAFETY: records identity and current depth in the thread-local C oracle.
    state.depth_sum += unsafe { oracle_hotkey_atomic_callback(data) };
    state.value = state
        .value
        .wrapping_mul(17)
        .wrapping_add(state.calls as u64);
    if state.remaining > 0 {
        state.remaining -= 1;
        let rust = state.rust;
        // End the mutable borrow before the recursive callback borrows Data.
        // SAFETY: data remains live, callback is valid, and the lock is recursive.
        unsafe {
            if rust {
                obs_hotkey_update_atomic(Some(callback), data);
            } else {
                oracle_obs_hotkey_update_atomic(Some(callback), data);
            }
        }
    }
}

fn run(rust: bool, initialized: bool, nesting: u8, value: u64) -> (Vec<usize>, usize, u64, usize) {
    let mut state = Data {
        rust,
        remaining: nesting,
        calls: 0,
        value,
        depth_sum: 0,
    };
    let data = (&raw mut state).cast();
    let mut trace = [0; 256];
    ACTIVE.with(|active| active.set(true));
    // SAFETY: all state is thread-local; data and the output array remain live.
    unsafe {
        oracle_hotkey_atomic_reset(initialized, data);
        if rust {
            obs_hotkey_update_atomic(Some(callback), data);
        } else {
            oracle_obs_hotkey_update_atomic(Some(callback), data);
        }
        let count = oracle_hotkey_atomic_trace(trace.as_mut_ptr());
        ACTIVE.with(|active| active.set(false));
        (
            trace[..count].to_vec(),
            state.calls,
            state.value,
            state.depth_sum,
        )
    }
}

fn compare(initialized: bool, nesting: u8, value: u64) {
    let rust = run(true, initialized, nesting, value);
    assert_eq!(rust, run(false, initialized, nesting, value));
    let calls = if initialized {
        usize::from(nesting) + 1
    } else {
        0
    };
    assert_eq!(rust.1, calls);
    assert_eq!(rust.3, calls * (calls + 1) / 2);
    if !initialized {
        assert_eq!(rust.0, [64]);
        assert_eq!(rust.2, value);
    }
}

#[test]
fn recursive_edges() {
    for initialized in [false, true] {
        for nesting in [0, 1, 8] {
            for value in [0, 1, u64::MAX] {
                compare(initialized, nesting, value);
            }
        }
    }
}

#[test]
fn failed_lock_accepts_null_callback_and_data() {
    ACTIVE.with(|active| active.set(true));
    let mut trace = [0; 256];
    // SAFETY: failed locking never invokes the null callback or accesses data.
    unsafe {
        oracle_hotkey_atomic_reset(false, core::ptr::null_mut());
        obs_hotkey_update_atomic(None, core::ptr::null_mut());
        assert_eq!(oracle_hotkey_atomic_trace(trace.as_mut_ptr()), 1);
        assert_eq!(trace[0], 64);
    }
    ACTIVE.with(|active| active.set(false));
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(128), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn differential(initialized in any::<bool>(), nesting in 0u8..9, value in any::<u64>()) {
        compare(initialized, nesting, value);
    }
}
