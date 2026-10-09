//! Tier 1: safe core of the `bmem` port (alignment math, accounting rules).
//! Ports the parts of `test/cmocka/test_bmem.c` that apply to the core; the
//! rest run against the real shims in the cmocka suite and `bmem_parity`.

use obs_c_oracle as _;
use obs_util::bmem::{ALIGNMENT, AllocCounter, aligned_addr, aligned_offset, raw_addr};

/// `alignment_constant_test`
#[test]
fn alignment_constant_test() {
    assert_eq!(ALIGNMENT, 32);
}

/// `bmalloc_alignment_test`: for any raw malloc address (here every residue
/// mod 32, at several bases) the returned address is 32-aligned and leaves at
/// least one byte for the stored offset.
#[test]
fn bmalloc_alignment_test() {
    for base in [0usize, 0x1000, 0x7fff_ffff_f000] {
        for r in 0..64usize {
            let raw = base + r;
            let diff = aligned_offset(raw);
            let out = aligned_addr(raw);
            assert_eq!(out % 32, 0, "raw {raw:#x}");
            assert_eq!(out, raw + diff);
            assert!((1..=32).contains(&diff), "diff {diff} for raw {raw:#x}");
            assert!(out > raw);
        }
    }
}

#[test]
fn already_aligned_address_gets_full_offset() {
    assert_eq!(aligned_offset(0), 32);
    assert_eq!(aligned_offset(32), 32);
    assert_eq!(aligned_offset(0x1000), 32);
    assert_eq!(aligned_addr(64), 96);
}

#[test]
fn offset_just_below_alignment_is_one() {
    assert_eq!(aligned_offset(31), 1);
    assert_eq!(aligned_offset(63), 1);
    assert_eq!(aligned_addr(31), 32);
}

#[test]
fn offset_matches_known_values() {
    assert_eq!(aligned_offset(1), 31);
    assert_eq!(aligned_offset(16), 16);
    assert_eq!(aligned_offset(17), 15);
    assert_eq!(aligned_offset(30), 2);
}

#[test]
fn offset_fits_in_a_byte_for_every_residue() {
    for addr in 0..256usize {
        let diff = aligned_offset(addr);
        assert!(u8::try_from(diff).is_ok());
        assert_eq!(raw_addr(aligned_addr(addr), diff as u8), addr);
    }
}

/// `alloc_count_test`: the counter rules, driven the way `bmalloc`,
/// `brealloc` and `bfree` drive it.
#[test]
fn alloc_count_test() {
    let c = AllocCounter::new();
    let base = c.get();
    assert_eq!(base, 0);

    // bmalloc(16): counts after success.
    c.inc();
    assert_eq!(c.get(), base + 1);

    // brealloc(NULL, 16): counts as an allocation.
    c.inc();
    assert_eq!(c.get(), base + 2);

    // brealloc(p, 4096) on a live block: no change (no counter call).
    assert_eq!(c.get(), base + 2);

    // bfree(p): decrements.
    c.dec();
    assert_eq!(c.get(), base + 1);

    // bfree(NULL): does not decrement (no counter call).
    assert_eq!(c.get(), base + 1);

    // bfree(q)
    c.dec();
    assert_eq!(c.get(), base);
}

#[test]
fn counter_default_is_zero_and_can_go_negative() {
    let c = AllocCounter::default();
    assert_eq!(c.get(), 0);
    c.dec();
    assert_eq!(c.get(), -1);
    c.inc();
    c.inc();
    assert_eq!(c.get(), 1);
}
