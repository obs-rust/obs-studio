//! Tier 3: the Rust `bmem` shims behave exactly like the original C, compiled
//! as an oracle.
//!
//! No intentional differences from C. Only the fresh-allocation pointers are
//! checked for 32-byte alignment: `brealloc` of a live block keeps the old
//! offset and so (as in C) does not promise alignment when the block moves.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use core::ffi::c_void;

use obs_c_oracle::bmem as c;
use obs_util::ffi::bmem as rs;
use proptest::prelude::*;

#[derive(Debug, Clone)]
enum Op {
    Malloc(usize),
    ReallocNull(usize),
    Realloc(prop::sample::Index, usize),
    Free(prop::sample::Index),
    Memdup(usize),
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (1..=4096usize).prop_map(Op::Malloc),
        (1..=4096usize).prop_map(Op::ReallocNull),
        (any::<prop::sample::Index>(), 1..=4096usize).prop_map(|(i, n)| Op::Realloc(i, n)),
        any::<prop::sample::Index>().prop_map(Op::Free),
        (1..=4096usize).prop_map(Op::Memdup),
    ]
}

struct Slot {
    ours: *mut c_void,
    theirs: *mut c_void,
    expected: Vec<u8>,
}

fn pattern(seed: usize, len: usize) -> Vec<u8> {
    (0..len)
        .map(|i| (i.wrapping_mul(31).wrapping_add(seed)) as u8)
        .collect()
}

unsafe fn fill(p: *mut c_void, bytes: &[u8]) {
    // SAFETY: the caller guarantees `p` has `bytes.len()` writable bytes.
    unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), p.cast::<u8>(), bytes.len()) };
}

unsafe fn read<'a>(p: *mut c_void, len: usize) -> &'a [u8] {
    // SAFETY: the caller guarantees `p` has `len` readable bytes.
    unsafe { core::slice::from_raw_parts(p.cast::<u8>(), len) }
}

fn counts() -> (i64, i64) {
    // SAFETY: both counters are always readable.
    unsafe { (rs::bnum_allocs() as i64, c::oracle_bnum_allocs() as i64) }
}

#[test]
fn alignment_matches_c() {
    // SAFETY: no preconditions.
    let (ours, theirs) = unsafe { (rs::base_get_alignment(), c::oracle_base_get_alignment()) };
    assert_eq!(ours, 32);
    assert_eq!(ours, theirs);
}

proptest! {
    #[test]
    fn matches_c_oracle(ops in prop::collection::vec(op(), 1..64), seed in any::<usize>()) {
        let mut slots: Vec<Slot> = Vec::new();

        for (step, op) in ops.into_iter().enumerate() {
            let (rs_before, c_before) = counts();
            let seed = seed.wrapping_add(step);

            match op {
                Op::Malloc(n) => {
                    // SAFETY: `n >= 1`; each block gets `n` bytes written.
                    let (ours, theirs) = unsafe { (rs::bmalloc(n), c::oracle_bmalloc(n)) };
                    prop_assert!(!ours.is_null() && !theirs.is_null());
                    prop_assert_eq!(ours as usize % 32, 0);
                    prop_assert_eq!(theirs as usize % 32, 0);
                    let expected = pattern(seed, n);
                    // SAFETY: both blocks hold `n` bytes.
                    unsafe { fill(ours, &expected); fill(theirs, &expected); }
                    slots.push(Slot { ours, theirs, expected });
                }
                Op::ReallocNull(n) => {
                    // SAFETY: NULL with `n >= 1` is an allocation.
                    let (ours, theirs) = unsafe {
                        (rs::brealloc(core::ptr::null_mut(), n), c::oracle_brealloc(core::ptr::null_mut(), n))
                    };
                    prop_assert!(!ours.is_null() && !theirs.is_null());
                    prop_assert_eq!(ours as usize % 32, 0);
                    prop_assert_eq!(theirs as usize % 32, 0);
                    let expected = pattern(seed, n);
                    // SAFETY: both blocks hold `n` bytes.
                    unsafe { fill(ours, &expected); fill(theirs, &expected); }
                    slots.push(Slot { ours, theirs, expected });
                }
                Op::Realloc(i, n) => {
                    if !slots.is_empty() {
                        let idx = i.index(slots.len());
                        let slot = &mut slots[idx];
                        // SAFETY: the slot's blocks are live; `n >= 1`.
                        let (ours, theirs) = unsafe {
                            (rs::brealloc(slot.ours, n), c::oracle_brealloc(slot.theirs, n))
                        };
                        prop_assert!(!ours.is_null() && !theirs.is_null());
                        let keep = slot.expected.len().min(n);
                        // SAFETY: both blocks hold at least `keep` bytes.
                        unsafe {
                            prop_assert_eq!(read(ours, keep), &slot.expected[..keep]);
                            prop_assert_eq!(read(theirs, keep), &slot.expected[..keep]);
                        }
                        let mut expected = slot.expected[..keep].to_vec();
                        expected.extend(pattern(seed, n - keep));
                        // SAFETY: both blocks hold `n` bytes.
                        unsafe { fill(ours, &expected); fill(theirs, &expected); }
                        *slot = Slot { ours, theirs, expected };
                    }
                }
                Op::Free(i) => {
                    if !slots.is_empty() {
                        let slot = slots.swap_remove(i.index(slots.len()));
                        // SAFETY: the slot's blocks are live.
                        unsafe { rs::bfree(slot.ours); c::oracle_bfree(slot.theirs); }
                    }
                    let (n_rs, n_c) = counts();
                    // SAFETY: freeing NULL is allowed and must not change the counters.
                    unsafe { rs::bfree(core::ptr::null_mut()); c::oracle_bfree(core::ptr::null_mut()); }
                    prop_assert_eq!(counts(), (n_rs, n_c));
                }
                Op::Memdup(n) => {
                    let src = pattern(seed, n);
                    // SAFETY: `src` has `n >= 1` readable bytes.
                    let (ours, theirs) = unsafe {
                        (rs::bmemdup(src.as_ptr().cast(), n), c::oracle_bmemdup(src.as_ptr().cast(), n))
                    };
                    prop_assert!(!ours.is_null() && !theirs.is_null());
                    prop_assert_eq!(ours as usize % 32, 0);
                    prop_assert_eq!(theirs as usize % 32, 0);
                    // SAFETY: both blocks hold `n` bytes.
                    unsafe {
                        prop_assert_eq!(read(ours, n), &src[..]);
                        prop_assert_eq!(read(theirs, n), &src[..]);
                    }
                    slots.push(Slot { ours, theirs, expected: src });
                }
            }

            let (rs_after, c_after) = counts();
            prop_assert_eq!(rs_after - rs_before, c_after - c_before);
        }

        for slot in slots {
            // SAFETY: the blocks are live.
            unsafe { rs::bfree(slot.ours); c::oracle_bfree(slot.theirs); }
        }
    }
}
