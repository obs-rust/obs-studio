//! Tier 3: `obs_core::win_crash_report` against the original C.
//!
//! `obs-c-oracle` compiles the unchanged `libobs/obs-win-crash-report.c.h`
//! with its nine functions renamed `oracle_*` (it takes no Windows type, so
//! every host builds it). Every test runs the Rust shim, the C and the safe
//! core on the same input and compares the results exactly, the C's `bool`
//! as the byte it returns.
//!
//! - the module rule over every combination of edge bases (0, 2^32, 2^36,
//!   2^63, the last 2^32 below 2^64, ...), edge sizes (0, 1, 15, 16, 32,
//!   2^31, `u32::MAX`, ...) and addresses at and around each module's base
//!   and end, and over the edges themselves;
//! - the thread rules over every pair of edge ids and both passes, and
//!   `write_thread_traces()`'s two passes over every snapshot of up to
//!   three threads from three ids and two processes, for every current
//!   thread and process;
//! - the frame line over every flag value of the low 16 bits under four high
//!   patterns, every single bit and every pair of bits;
//! - both masks over every power of two and its neighbours;
//! - the name prefix over 0 to 4096 and the ends of `size_t`;
//! - the filter over both inputs, with NULL for the flag whenever a
//!   debugger is present (neither side reads it then);
//! - proptests (20,000 cases each) over all of it.
//!
//! No input is skipped on any host: no division, no 128-bit multiply, and
//! the end of a module wraps in unsigned arithmetic on both sides. No
//! intentional differences.

use std::ptr;

use obs_c_oracle::win_crash_report as oracle;
use obs_core::ffi::win_crash_report as shim;
use obs_core::win_crash_report as core_rules;
use proptest::prelude::*;

/// Values a base, an address or a frame value often sits at.
const EDGES: &[u64] = &[
    0,
    1,
    2,
    0x0040_0000,
    0x7FFF_FFFF,
    0xFFFF_FFFE,
    0xFFFF_FFFF,
    0x1_0000_0000,
    0x1_0000_0001,
    0xF_FFFF_FFFF,
    0x10_0000_0000,
    0x7FF6_1234_0000,
    0x7FFF_FFFF_FFFF_FFFF,
    0x8000_0000_0000_0000,
    u64::MAX - 0xFFFF_FFFF,
    u64::MAX - 32,
    u64::MAX - 16,
    u64::MAX - 15,
    u64::MAX - 1,
    u64::MAX,
];

const SIZES: &[u32] = &[
    0,
    1,
    2,
    15,
    16,
    17,
    32,
    0x2000,
    0x7FFF_FFFF,
    0x8000_0000,
    u32::MAX - 1,
    u32::MAX,
];

const IDS: &[u32] = &[
    0,
    1,
    2,
    3,
    0x1A2C,
    0x7FFF_FFFF,
    0x8000_0000,
    u32::MAX - 1,
    u32::MAX,
];

fn contains_all(addr: u64, base: u64, size: u32) {
    let s = shim::obs_win_crash_report_contains(addr, base, size);
    // SAFETY: plain values.
    let c = unsafe { oracle::oracle_obs_win_crash_report_contains(addr, base, size) };
    let r = core_rules::contains(addr, base, size);
    assert_eq!(
        (u8::from(s), u8::from(r)),
        (c, c),
        "contains({addr:#x}, {base:#x}, {size:#x})"
    );
}

#[test]
fn every_module_edge() {
    let mut cases = 0u32;
    for &base in EDGES {
        for &size in SIZES {
            let end = base.wrapping_add(u64::from(size));
            for addr in [
                base.wrapping_sub(1),
                base,
                base.wrapping_add(1),
                end.wrapping_sub(1),
                end,
                end.wrapping_add(1),
            ]
            .into_iter()
            .chain(EDGES.iter().copied())
            {
                contains_all(addr, base, size);
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 20 * 12 * 26);
}

fn threads_all(a: u32, b: u32, first: bool, crashed: bool) {
    // SAFETY (each): plain values.
    let (c_crashed, c_pass, c_other) = unsafe {
        (
            oracle::oracle_obs_win_crash_report_is_crashed(a, b),
            oracle::oracle_obs_win_crash_report_in_pass(first, crashed),
            oracle::oracle_obs_win_crash_report_other_process(a, b),
        )
    };
    assert_eq!(
        u8::from(shim::obs_win_crash_report_is_crashed(a, b)),
        c_crashed
    );
    assert_eq!(u8::from(core_rules::is_crashed(a, b)), c_crashed);
    assert_eq!(
        u8::from(shim::obs_win_crash_report_in_pass(first, crashed)),
        c_pass
    );
    assert_eq!(u8::from(core_rules::in_pass(first, crashed)), c_pass);
    assert_eq!(
        u8::from(shim::obs_win_crash_report_other_process(a, b)),
        c_other
    );
    assert_eq!(u8::from(core_rules::other_process(a, b)), c_other);
}

#[test]
fn every_thread_rule() {
    for &a in IDS {
        for &b in IDS {
            for first in [false, true] {
                for crashed in [false, true] {
                    threads_all(a, b, first, crashed);
                }
            }
        }
    }
}

/// The three thread rules, from one side.
struct Side {
    is_crashed: fn(u32, u32) -> u8,
    in_pass: fn(bool, bool) -> u8,
    other_process: fn(u32, u32) -> u8,
}

const RUST: Side = Side {
    is_crashed: |a, b| u8::from(shim::obs_win_crash_report_is_crashed(a, b)),
    in_pass: |f, c| u8::from(shim::obs_win_crash_report_in_pass(f, c)),
    other_process: |a, b| u8::from(shim::obs_win_crash_report_other_process(a, b)),
};

// SAFETY (each): plain values.
const C: Side = Side {
    is_crashed: |a, b| unsafe { oracle::oracle_obs_win_crash_report_is_crashed(a, b) },
    in_pass: |f, c| unsafe { oracle::oracle_obs_win_crash_report_in_pass(f, c) },
    other_process: |a, b| unsafe { oracle::oracle_obs_win_crash_report_other_process(a, b) },
};

/// `write_thread_traces()`'s two passes, with `write_thread_trace()`'s
/// checks in their order: the traced ids and their "(Crashed)" labels.
fn trace_threads(side: &Side, threads: &[(u32, u32)], tid: u32, pid: u32) -> Vec<(u32, bool)> {
    let mut out = Vec::new();
    for first_thread in [true, false] {
        for &(id, owner) in threads {
            let crash_thread = (side.is_crashed)(id, tid) != 0;
            if (side.in_pass)(first_thread, crash_thread) == 0 {
                continue;
            }
            if (side.other_process)(owner, pid) != 0 {
                continue;
            }
            out.push((id, crash_thread));
        }
    }
    out
}

#[test]
fn every_small_snapshot() {
    let entries: Vec<(u32, u32)> = (1..=3)
        .flat_map(|id| (1..=2).map(move |owner| (id, owner)))
        .collect();
    let mut snapshots: Vec<Vec<(u32, u32)>> = vec![Vec::new()];
    for len in 1..=3 {
        let mut grown = Vec::new();
        for s in snapshots.iter().filter(|s| s.len() == len - 1) {
            for &e in &entries {
                let mut t = s.clone();
                t.push(e);
                grown.push(t);
            }
        }
        snapshots.extend(grown);
    }
    assert_eq!(snapshots.len(), 1 + 6 + 36 + 216);
    for snapshot in &snapshots {
        for tid in 1..=4 {
            for pid in 1..=2 {
                let rust = trace_threads(&RUST, snapshot, tid, pid);
                assert_eq!(
                    rust,
                    trace_threads(&C, snapshot, tid, pid),
                    "{snapshot:?} {tid} {pid}"
                );
                // the crashed thread, when traced, comes first
                if let Some(i) = rust.iter().position(|&(_, crashed)| crashed) {
                    assert!(rust[..i].iter().all(|&(_, crashed)| crashed));
                }
            }
        }
    }
}

fn names_symbol_all(flags: u32) {
    let s = shim::obs_win_crash_report_names_symbol(flags);
    // SAFETY: a plain value.
    let c = unsafe { oracle::oracle_obs_win_crash_report_names_symbol(flags) };
    let r = core_rules::names_symbol(flags);
    assert_eq!(
        (u8::from(s), u8::from(r)),
        (c, c),
        "names_symbol({flags:#x})"
    );
}

#[test]
fn every_flag_pattern() {
    for high in [0u32, 0xFFFF_0000, 0x5A5A_0000, 0xA5A5_0000] {
        for low in 0..=0xFFFFu32 {
            names_symbol_all(high | low);
        }
    }
    for i in 0..32 {
        for j in 0..32 {
            names_symbol_all((1u32 << i) | (1u32 << j));
            names_symbol_all(!((1u32 << i) | (1u32 << j)));
        }
    }
}

fn masks_all(value: u64) {
    // SAFETY (each): plain values.
    let (address, param) = unsafe {
        (
            oracle::oracle_obs_win_crash_report_mask_address(value),
            oracle::oracle_obs_win_crash_report_mask_param(value),
        )
    };
    assert_eq!(shim::obs_win_crash_report_mask_address(value), address);
    assert_eq!(core_rules::mask_address(value), address);
    assert_eq!(shim::obs_win_crash_report_mask_param(value), param);
    assert_eq!(core_rules::mask_param(value), param);
}

#[test]
fn every_mask_edge() {
    for &v in EDGES {
        masks_all(v);
        masks_all(!v);
    }
    for i in 0..64 {
        let p = 1u64 << i;
        for v in [p - 1, p, p + 1, !p, p | 0xF_FFFF_FFFF] {
            masks_all(v);
        }
    }
}

fn name_prefix_all(len: usize) {
    // SAFETY: a plain value.
    let c = unsafe { oracle::oracle_obs_win_crash_report_name_prefix(len) };
    assert_eq!(u8::from(shim::obs_win_crash_report_name_prefix(len)), c);
    assert_eq!(u8::from(core_rules::name_prefix(len)), c);
}

#[test]
fn every_name_length() {
    for len in 0..=4096 {
        name_prefix_all(len);
    }
    for len in [usize::MAX - 1, usize::MAX, usize::MAX / 2, 1 << 31, 1 << 32] {
        name_prefix_all(len);
    }
}

fn pass_on_all(debugger: bool, inside: Option<bool>) {
    let p = inside.as_ref().map_or(ptr::null(), ptr::from_ref);
    // SAFETY: `p` points to a live bool, or is NULL only with a debugger,
    // when neither side reads it.
    let (s, c) = unsafe {
        (
            shim::obs_win_crash_report_pass_on(debugger, p),
            oracle::oracle_obs_win_crash_report_pass_on(debugger, p),
        )
    };
    assert_eq!(u8::from(s), c, "pass_on({debugger}, {inside:?})");
    if let Some(inside) = inside {
        assert_eq!(u8::from(core_rules::pass_on(debugger, || inside)), c);
    }
}

#[test]
fn every_handler_state() {
    // a debugger: the flag is never read, so NULL is safe on both sides
    pass_on_all(true, None);
    for debugger in [false, true] {
        for inside in [false, true] {
            pass_on_all(debugger, Some(inside));
        }
    }
}

/// A module and an address near its base or end, or anywhere.
fn module_strategy() -> impl Strategy<Value = (u64, u64, u32)> {
    (
        any::<u64>(),
        any::<u32>(),
        0u8..8,
        any::<i8>(),
        any::<u64>(),
    )
        .prop_map(|(base, size, near, delta, anywhere)| {
            let end = base.wrapping_add(u64::from(size));
            let d = u64::from(delta.unsigned_abs());
            let addr = match near {
                0 => base.wrapping_add(d),
                1 => base.wrapping_sub(d),
                2 => end.wrapping_add(d),
                3 => end.wrapping_sub(d),
                4 => u64::MAX - (anywhere % 0x1_0000_0000),
                _ => anywhere,
            };
            let base = if near == 5 {
                u64::MAX - (anywhere % 0x1_0000_0000)
            } else {
                base
            };
            (addr, base, size)
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(20_000)))]

    #[test]
    fn proptest_modules((addr, base, size) in module_strategy()) {
        contains_all(addr, base, size);
    }

    #[test]
    fn proptest_frames(flags in any::<u32>(), value in any::<u64>(), len in any::<usize>()) {
        names_symbol_all(flags);
        masks_all(value);
        name_prefix_all(len);
    }

    #[test]
    fn proptest_threads(
        snapshot in prop::collection::vec((0u32..6, 0u32..3), 0..8),
        tid in 0u32..6,
        pid in 0u32..3,
        a in any::<u32>(),
        b in any::<u32>(),
        first in any::<bool>(),
        crashed in any::<bool>(),
        debugger in any::<bool>(),
        inside in any::<bool>(),
    ) {
        prop_assert_eq!(trace_threads(&RUST, &snapshot, tid, pid), trace_threads(&C, &snapshot, tid, pid));
        threads_all(a, b, first, crashed);
        pass_on_all(debugger, Some(inside));
    }
}
