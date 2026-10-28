//! The crash report shims allocate nothing.
//!
//! `obs-win-crash-handler.c` calls them inside the unhandled exception
//! filter, maybe with a damaged heap, so a Rust heap allocation there could
//! fail or recurse into the crash. Every shim runs over edge inputs (and
//! `pass_on` both with and without a debugger) under the counting allocator
//! of this binary (`alloc_count.rs`), which must see no allocation on this
//! thread. The safe core runs the same way.

use std::hint::black_box;

use obs_core::ffi::win_crash_report as shim;
use obs_core::win_crash_report as core_rules;

use super::alloc_count::allocs_during;

const VALUES: [u64; 8] = [
    0,
    1,
    0xFFFF_FFFF,
    0x1_0000_0000,
    0xF_FFFF_FFFF,
    0x7FF6_1234_0000,
    u64::MAX - 15,
    u64::MAX,
];
const SIZES: [u32; 5] = [0, 1, 16, 0x2000, u32::MAX];
const IDS: [u32; 4] = [0, 1, 0x1A2C, u32::MAX];
const FLAGS: [u32; 5] = [0, 0x200, 0x800, 0xA00, u32::MAX];
const LENS: [usize; 4] = [0, 1, 255, usize::MAX];

/// Calls every shim over the edge inputs, folding the results so none is
/// optimized away.
fn every_shim() -> u64 {
    let mut sum = 0u64;
    let mut add = |b: bool| sum = sum.wrapping_mul(3).wrapping_add(u64::from(b));
    for &addr in &VALUES {
        for &base in &VALUES {
            for &size in &SIZES {
                add(shim::obs_win_crash_report_contains(
                    black_box(addr),
                    base,
                    size,
                ));
            }
        }
    }
    for &a in &IDS {
        for &b in &IDS {
            add(shim::obs_win_crash_report_is_crashed(black_box(a), b));
            add(shim::obs_win_crash_report_other_process(black_box(a), b));
        }
    }
    for first in [false, true] {
        for crashed in [false, true] {
            add(shim::obs_win_crash_report_in_pass(
                black_box(first),
                crashed,
            ));
        }
    }
    for &flags in &FLAGS {
        add(shim::obs_win_crash_report_names_symbol(black_box(flags)));
    }
    for &len in &LENS {
        add(shim::obs_win_crash_report_name_prefix(black_box(len)));
    }
    let inside = [false, true];
    // SAFETY: NULL only with a debugger, when the flag is never read;
    // otherwise a live bool.
    unsafe {
        add(shim::obs_win_crash_report_pass_on(true, std::ptr::null()));
        for flag in &inside {
            add(shim::obs_win_crash_report_pass_on(black_box(false), flag));
            add(shim::obs_win_crash_report_pass_on(black_box(true), flag));
        }
    }
    for &v in &VALUES {
        sum = sum
            .wrapping_add(shim::obs_win_crash_report_mask_address(black_box(v)))
            .wrapping_add(shim::obs_win_crash_report_mask_param(black_box(v)));
    }
    sum
}

/// The safe core over the same inputs.
fn every_core_rule() -> u64 {
    let mut sum = 0u64;
    let mut add = |b: bool| sum = sum.wrapping_mul(3).wrapping_add(u64::from(b));
    for &addr in &VALUES {
        for &base in &VALUES {
            for &size in &SIZES {
                add(core_rules::contains(black_box(addr), base, size));
            }
        }
    }
    for &a in &IDS {
        for &b in &IDS {
            add(core_rules::is_crashed(black_box(a), b));
            add(core_rules::other_process(black_box(a), b));
        }
    }
    for first in [false, true] {
        for crashed in [false, true] {
            add(core_rules::in_pass(black_box(first), crashed));
        }
    }
    for &flags in &FLAGS {
        add(core_rules::names_symbol(black_box(flags)));
    }
    for &len in &LENS {
        add(core_rules::name_prefix(black_box(len)));
    }
    for debugger in [false, true] {
        for inside in [false, true] {
            add(core_rules::pass_on(black_box(debugger), || inside));
        }
    }
    for &v in &VALUES {
        sum = sum
            .wrapping_add(core_rules::mask_address(black_box(v)))
            .wrapping_add(core_rules::mask_param(black_box(v)));
    }
    sum
}

#[test]
fn shims_allocate_nothing() {
    // the counter itself sees an allocation: the check is live
    assert_eq!(allocs_during(|| drop(black_box(vec![0u8; 16]))), 1);

    let mut sum = 0;
    assert_eq!(allocs_during(|| sum = every_shim()), 0);
    assert_ne!(black_box(sum), 0);
}

#[test]
fn core_allocates_nothing() {
    let mut sum = 0;
    assert_eq!(allocs_during(|| sum = every_core_rule()), 0);
    assert_ne!(black_box(sum), 0);
}
