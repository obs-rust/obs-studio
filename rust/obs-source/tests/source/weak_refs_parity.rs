//! Tier 3: the original C `obs_weak_source_addref`/`obs_weak_source_release`
//! and the Rust exports over real-header controls, with the real weak-count
//! adapter. Counts stay far from overflow (C UB). No intentional differences.
use core::ffi::{c_long, c_void};
use obs_source::ffi::source_weak_refs::{obs_weak_source_addref, obs_weak_source_release};
use obs_util::platform::CLongAtomic;
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_source_weak_refs_new(refs: c_long, weak: c_long, noise: u8) -> *mut c_void;
    fn oracle_source_weak_refs_weak(handle: *mut c_void) -> *mut c_void;
    fn oracle_source_weak_refs_counts(handle: *mut c_void, counts: *mut c_long);
    fn oracle_source_weak_refs_intact(handle: *mut c_void) -> bool;
    fn oracle_source_weak_refs_free(handle: *mut c_void, released: bool);
    fn oracle_source_weak_refs_layout(layout: *mut usize);
    fn oracle_obs_weak_source_addref(weak: *mut c_void);
    fn oracle_obs_weak_source_release(weak: *mut c_void);
}

struct Fixture {
    handle: *mut c_void,
    released: bool,
}

impl Fixture {
    fn new(refs: c_long, weak: c_long, noise: u8) -> Self {
        // SAFETY: bounded counts initialize an owned real-header fixture.
        let handle = unsafe { oracle_source_weak_refs_new(refs, weak, noise) };
        assert!(!handle.is_null());
        Self {
            handle,
            released: false,
        }
    }

    fn counts(&self) -> [c_long; 2] {
        let mut counts = [0; 2];
        // SAFETY: the control is live (not yet released); two entries written.
        unsafe { oracle_source_weak_refs_counts(self.handle, counts.as_mut_ptr()) };
        counts
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: owns the fixture; a released control was already bfreed.
        unsafe { oracle_source_weak_refs_free(self.handle, self.released) };
    }
}

#[test]
fn native_long_layout() {
    let mut layout = [0; 2];
    // SAFETY: the oracle writes exactly two entries.
    unsafe { oracle_source_weak_refs_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [size_of::<CLongAtomic>(), align_of::<CLongAtomic>()]
    );
}

/// Runs `ops` (even: addref, odd: release) on a Rust and a C fixture. Returns
/// the original's counts after each op and whether its last release freed.
fn check(refs: c_long, weak: c_long, noise: u8, ops: &[u8]) -> (Vec<[c_long; 2]>, bool) {
    let mut rust = Fixture::new(refs, weak, noise);
    let mut c = Fixture::new(refs, weak, noise);
    let mut seen = Vec::new();
    // SAFETY: both fixtures stay alive; the loop stops at the release that
    // frees, so no freed control is touched again.
    unsafe {
        let rw = oracle_source_weak_refs_weak(rust.handle);
        let cw = oracle_source_weak_refs_weak(c.handle);
        for op in ops {
            if op % 2 == 0 {
                obs_weak_source_addref(rw);
                oracle_obs_weak_source_addref(cw);
            } else {
                let frees = c.counts()[1] == 0;
                rust.released = frees;
                c.released = frees;
                obs_weak_source_release(rw);
                oracle_obs_weak_source_release(cw);
                if frees {
                    break;
                }
            }
            let counts = c.counts();
            assert_eq!(
                rust.counts(),
                counts,
                "refs={refs}, weak={weak}, ops={ops:?}"
            );
            assert!(oracle_source_weak_refs_intact(rust.handle));
            assert!(oracle_source_weak_refs_intact(c.handle));
            seen.push(counts);
        }
    }
    (seen, c.released)
}

#[test]
fn null_handles_match_original_c() {
    // SAFETY: NULL is explicitly a no-op for all four functions.
    unsafe {
        obs_weak_source_addref(core::ptr::null_mut());
        oracle_obs_weak_source_addref(core::ptr::null_mut());
        obs_weak_source_release(core::ptr::null_mut());
        oracle_obs_weak_source_release(core::ptr::null_mut());
    }
}

#[test]
fn characterized_edges() {
    // Pinned outcomes, so a mutation on both sides still fails.
    assert_eq!(
        check(7, 3, 0xa5, &[0, 1, 1]),
        (vec![[7, 4], [7, 3], [7, 2]], false)
    );
    assert_eq!(check(-1, 1, 0, &[1, 1, 0]), (vec![[-1, 0]], true));
    assert_eq!(check(5, 0, 0xff, &[0, 1, 1]), (vec![[5, 1], [5, 0]], true));
    // Below zero only falls further: only the exact 0 -> -1 step frees.
    assert_eq!(
        check(0, -1, 0x5a, &[1, 0, 0, 1]),
        (vec![[0, -2], [0, -1], [0, 0]], true)
    );
    for refs in [-2, -1, 0, 1, 7] {
        for weak in [-3, -1, 0, 1, 3] {
            for noise in [0, 0x5a, 0xff] {
                check(refs, weak, noise, &[0, 1, 0, 1, 1, 1, 1, 1, 1, 1]);
                check(refs, weak, noise, &[1, 1, 1, 1, 1]);
            }
        }
    }
}

/// Generated starting counts, typed as `c_long` so no target needs a conversion
/// (Windows `c_long` is `i32`, Linux and macOS `i64`).
const REFS: core::ops::Range<c_long> = -2..32;
const WEAK: core::ops::Range<c_long> = -4..16;

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn generated_sequences_match_original_c(
        refs in REFS,
        weak in WEAK,
        noise in any::<u8>(),
        ops in proptest::collection::vec(0_u8..2, 0..40),
    ) {
        check(refs, weak, noise, &ops);
    }
}
