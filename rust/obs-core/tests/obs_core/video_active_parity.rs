//! Tier 3: the original extracted `obs_video_active()` versus the shim on one
//! recording fixture core (oracle/video_active.c), through the real-named
//! mutex, list and counter adapters. Compared: the answer; the trace of
//! every core read, lock, unlock and counter load with the mix it belongs to
//! (so the order, the early stop at the first active mix and a GPU count left
//! unread after an active raw count all show), with its checks that every
//! load runs under the lock; the mutex released; and the untouched bytes of
//! the list and every mix. Lists: empty, one, many and duplicate entries,
//! with counts negative, zero, positive and at the `long` extremes.
//!
//! Excluded: an uninitialized core, a NULL or dead listed mix and a
//! concurrent list change, outside the original's contract. No intentional
//! differences. Mutants show the probe catches each broken step.
use core::ffi::{CStr, c_char, c_long};
use obs_core::ffi::video_active::obs_video_active;
use proptest::prelude::*;

const POOL: usize = 8;

type Active = unsafe extern "C" fn() -> bool;
unsafe extern "C" {
    fn oracle_video_active_probe(
        candidate: Active,
        list: *const u8,
        count: usize,
        raw: *const c_long,
        gpu: *const c_long,
    ) -> u32;
    fn oracle_video_active_original(
        list: *const u8,
        count: usize,
        raw: *const c_long,
        gpu: *const c_long,
        trace: *mut c_char,
        trace_size: usize,
    ) -> bool;
    fn oracle_video_active_adapters() -> bool;
}

fn probe(candidate: Active, list: &[u8], raw: &[c_long; POOL], gpu: &[c_long; POOL]) -> u32 {
    // SAFETY: the oracle owns the fixture core and mixes every adapter
    // reaches; the list and counts outlive the call.
    unsafe {
        oracle_video_active_probe(
            candidate,
            list.as_ptr(),
            list.len(),
            raw.as_ptr(),
            gpu.as_ptr(),
        )
    }
}

/// The original's answer and trace.
fn original(list: &[u8], raw: &[c_long; POOL], gpu: &[c_long; POOL]) -> (bool, String) {
    let mut trace = [0 as c_char; 160];
    // SAFETY: the buffer is writable for its size; the oracle writes
    // NUL-terminated text.
    let answer = unsafe {
        oracle_video_active_original(
            list.as_ptr(),
            list.len(),
            raw.as_ptr(),
            gpu.as_ptr(),
            trace.as_mut_ptr(),
            trace.len(),
        )
    };
    // SAFETY: NUL-terminated by the oracle.
    let text = unsafe { CStr::from_ptr(trace.as_ptr()) }
        .to_str()
        .unwrap()
        .to_owned();
    (answer, text)
}

fn counts(pairs: &[(usize, c_long, c_long)]) -> ([c_long; POOL], [c_long; POOL]) {
    let (mut raw, mut gpu) = ([0; POOL], [0; POOL]);
    for &(mix, r, g) in pairs {
        raw[mix] = r;
        gpu[mix] = g;
    }
    (raw, gpu)
}

#[test]
fn real_adapters_read_the_real_fields() {
    // SAFETY: the oracle arms and frees its own fixture.
    assert!(unsafe { oracle_video_active_adapters() });
}

#[test]
fn original_c_is_the_characterized_walk() {
    let (raw, gpu) = counts(&[(0, 0, 0), (1, 0, 2), (2, 5, 5), (3, -1, 0), (4, 0, -1)]);
    // An empty list reads only its count.
    assert_eq!(original(&[], &raw, &gpu), (false, "oLooU".into()));
    // Every mix of an inactive list, in order, raw before GPU.
    assert_eq!(
        original(&[0, 3, 4], &raw, &gpu),
        (false, "oLoor0g0or3g3or4g4oU".into())
    );
    // The walk stops at the first active mix; an active raw count leaves
    // the GPU count unread.
    assert_eq!(
        original(&[0, 1, 2], &raw, &gpu),
        (true, "oLoor0g0or1g1oU".into())
    );
    assert_eq!(original(&[2, 1], &raw, &gpu), (true, "oLoor2oU".into()));
}

#[test]
fn characterized_edges() {
    let shim: Active = obs_video_active;
    let extremes = [c_long::MIN, -1, 0, 1, c_long::MAX];
    for &r in &extremes {
        for &g in &extremes {
            let (raw, gpu) = counts(&[(0, 0, 0), (5, r, g), (6, 1, 1)]);
            for list in [&[][..], &[5], &[0, 5], &[5, 6], &[0, 0, 5, 6], &[5, 5]] {
                assert_eq!(probe(shim, list, &raw, &gpu), 0, "{list:?} {r} {g}");
            }
        }
    }
}

fn count() -> impl Strategy<Value = c_long> {
    prop_oneof![
        Just(c_long::MIN),
        Just(c_long::MAX),
        -2..3 as c_long,
        any::<c_long>(),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn any_list_matches_original_c(
        list in proptest::collection::vec(0..POOL as u8, 0..=16),
        raw in proptest::array::uniform8(count()),
        gpu in proptest::array::uniform8(count()),
    ) {
        prop_assert_eq!(probe(obs_video_active, &list, &raw, &gpu), 0);
    }
}
