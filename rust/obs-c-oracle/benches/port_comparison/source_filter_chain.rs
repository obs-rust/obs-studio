//! obs-source: the filter chain rules (`libobs/obs-source-filter-chain.c.h`),
//! which `move_filter_dir()` runs for every filter drag (one walk step and
//! one kind compare per filter it passes, then one target per filter), and
//! `obs_source_restore_filters()` for every filter undo (one name compare
//! per current filter, per backed-up one). Each measurement folds its
//! results into a checksum that both sides must match:
//! - `moves`: every filter of a 16-filter chain (sync and async mixed)
//!   moved up, down, to the top and to the bottom, each move with the
//!   retarget of the whole chain (64 moves; the four movements of the enum,
//!   where the two sides agree);
//! - `restore`: the restore loop of 16 backed-up filters against 16 current
//!   ones (names sharing prefixes, one unnamed, four not found), with the
//!   restore step of each.
//!
//! Measured when it was added (x86_64 Linux, GCC, not pinned to a core, a
//! loaded machine; median): moves 2.44 us Rust, 2.56 us C per 64 moves
//! (0.95x); restore 914 ns Rust, 685 ns C per 16 filters (1.33x: the shim
//! measures both names before it compares them, where `strcmp()` stops at
//! the first difference). Under 15 ns a filter either way, once per drag
//! or undo.

use std::ffi::{CString, c_char, c_int};
use std::hint::black_box;
use std::ptr;

use criterion::{Criterion, Throughput};
use obs_c_oracle::source_filter_chain as c;
use obs_source::ffi::source_filter_chain as rs;

const FILTERS: usize = 16;

/// Whether each filter of the chain is async.
const ASYNC: [bool; FILTERS] = [
    false, true, false, false, true, true, false, true, false, false, false, true, false, true,
    true, false,
];

const CURRENT: [&str; FILTERS] = [
    "Color Correction",
    "Color Correction 2",
    "Crop/Pad",
    "Sharpen",
    "Chroma Key",
    "Chroma Key 2",
    "",
    "Scroll",
    "Mask",
    "Image Mask/Blend",
    "Gain",
    "Compressor",
    "Limiter",
    "Noise Gate",
    "Noise Suppression",
    "Video Delay",
];

const BACKUP: [&str; FILTERS] = [
    "Noise Gate",
    "Color Correction 2",
    "Render Delay",
    "Crop/Pad",
    "Chroma Key",
    "Color Correction",
    "Scroll",
    "Expander",
    "",
    "Gain",
    "Mask",
    "Upward Compressor",
    "Limiter",
    "Sharpen",
    "Luma Key",
    "Video Delay",
];

struct Side {
    move_route: fn(c_int) -> c_int,
    walk_step: fn(c_int, usize, usize, *mut usize) -> bool,
    same_kind: fn(u32, u32) -> bool,
    edge_move: fn(c_int, usize, usize, *mut usize) -> bool,
    target_of: fn(usize, usize) -> c_int,
    restore_matches: fn(*const c_char, *const c_char) -> bool,
    restore_step: fn(bool) -> c_int,
}

// SAFETY (every unsafe closure): the out-parameters are live locals of
// the callers below, and the names live NUL-terminated strings or NULL.
const RUST: Side = Side {
    move_route: |m| rs::obs_source_filter_chain_move_route(m),
    walk_step: |r, c, n, o| unsafe { rs::obs_source_filter_chain_walk_step(r, c, n, o) },
    same_kind: |a, b| rs::obs_source_filter_chain_same_kind(a, b),
    edge_move: |r, i, n, o| unsafe { rs::obs_source_filter_chain_edge_move(r, i, n, o) },
    target_of: |i, n| rs::obs_source_filter_chain_target_of(i, n),
    restore_matches: |a, b| unsafe { rs::obs_source_filter_chain_restore_matches(a, b) },
    restore_step: |f| rs::obs_source_filter_chain_restore_step(f),
};

// SAFETY (every closure): as above; the rest are plain values in and out.
const C: Side = Side {
    move_route: |m| unsafe { c::oracle_obs_source_filter_chain_move_route(m) },
    walk_step: |r, cur, n, o| unsafe { c::oracle_obs_source_filter_chain_walk_step(r, cur, n, o) },
    same_kind: |a, b| unsafe { c::oracle_obs_source_filter_chain_same_kind(a, b) },
    edge_move: |r, i, n, o| unsafe { c::oracle_obs_source_filter_chain_edge_move(r, i, n, o) },
    target_of: |i, n| unsafe { c::oracle_obs_source_filter_chain_target_of(i, n) },
    restore_matches: |a, b| unsafe { c::oracle_obs_source_filter_chain_restore_matches(a, b) },
    restore_step: |f| unsafe { c::oracle_obs_source_filter_chain_restore_step(f) },
};

fn fold(sum: u64, value: usize) -> u64 {
    sum.wrapping_mul(31).wrapping_add(value as u64)
}

fn flags(i: usize) -> u32 {
    if ASYNC[i] { 0b101 } else { 0b001 }
}

/// `move_filter_dir()` of every filter with every movement of the enum,
/// and `reorder_filter_targets()` after each.
fn moves(side: &Side) -> u64 {
    let mut sum = 0;
    for idx in 0..FILTERS {
        for movement in 0..4 {
            let route = (side.move_route)(black_box(movement));
            let mut to = usize::MAX;
            if route == rs::OBS_SOURCE_FILTER_CHAIN_ROUTE_UP
                || route == rs::OBS_SOURCE_FILTER_CHAIN_ROUTE_DOWN
            {
                let mut cur = idx;
                let mut next = 0;
                while (side.walk_step)(route, cur, FILTERS, &raw mut next) {
                    if (side.same_kind)(flags(idx), flags(next)) {
                        to = next;
                        break;
                    }
                    cur = next;
                }
            } else {
                let mut edge = 0;
                if (side.edge_move)(route, idx, FILTERS, &raw mut edge) {
                    to = edge;
                }
            }
            sum = fold(sum, to);
            if to != usize::MAX {
                for i in 0..FILTERS {
                    sum = fold(sum, (side.target_of)(i, FILTERS) as usize);
                }
            }
        }
    }
    sum
}

/// The backup loop of `obs_source_restore_filters()`.
fn restore(side: &Side, backup: &[CString], current: &[Option<CString>]) -> u64 {
    let mut sum = 0;
    for name in backup {
        let mut found = usize::MAX;
        for (j, cur) in current.iter().enumerate() {
            let cur = cur.as_ref().map_or(ptr::null(), |c| c.as_ptr());
            if (side.restore_matches)(cur, name.as_ptr()) {
                found = j;
                break;
            }
        }
        sum = fold(sum, found);
        sum = fold(sum, (side.restore_step)(found != usize::MAX) as usize);
    }
    sum
}

pub(crate) fn benchmarks(criterion: &mut Criterion) {
    let backup: Vec<CString> = BACKUP
        .iter()
        .map(|n| CString::new(*n).expect("no NUL"))
        .collect();
    let current: Vec<Option<CString>> = CURRENT
        .iter()
        .map(|n| (!n.is_empty()).then(|| CString::new(*n).expect("no NUL")))
        .collect();

    assert_eq!(moves(&RUST), moves(&C), "benchmark fixture differs: moves");
    // the restore steps differ for the four filters not found (the port
    // loads them private), so only the matches are compared
    let matches_only = |side: &Side| {
        let mut sum = 0;
        for name in &backup {
            for (j, cur) in current.iter().enumerate() {
                let cur = cur.as_ref().map_or(ptr::null(), |c| c.as_ptr());
                if (side.restore_matches)(cur, name.as_ptr()) {
                    sum = fold(sum, j);
                    break;
                }
            }
        }
        sum
    };
    assert_eq!(
        matches_only(&RUST),
        matches_only(&C),
        "benchmark fixture differs: restore"
    );

    let mut group = criterion.benchmark_group("source_filter_chain/moves");
    group.throughput(Throughput::Elements(4 * FILTERS as u64));
    group.bench_function("rust", |b| b.iter(|| black_box(moves(&RUST))));
    group.bench_function("c", |b| b.iter(|| black_box(moves(&C))));
    group.finish();

    let mut group = criterion.benchmark_group("source_filter_chain/restore");
    group.throughput(Throughput::Elements(FILTERS as u64));
    group.bench_function("rust", |b| {
        b.iter(|| black_box(restore(&RUST, &backup, &current)))
    });
    group.bench_function("c", |b| {
        b.iter(|| black_box(restore(&C, &backup, &current)))
    });
    group.finish();
}
