//! Tier 3 parity: the shims of `obs_source::ffi::source_filter_chain` (and
//! the safe core under them) against the original
//! `libobs/obs-source-filter-chain.c.h`, compiled as
//! `oracle_obs_source_filter_chain_*` (obs-c-oracle,
//! oracle/source_filter_chain.c).
//!
//! Every call runs the shim and the C and compares the results, the
//! out-parameters included; the safe core is checked against the shim
//! where the types differ.
//!
//! Runs: the header's values and enum sizes; exhaustive over every pair of
//! 49 flag words (every video, audio and async combination, alone and with
//! other bits), every bool input, every count, index and route from -2 to 7
//! over lists of up to 20 and near the ends of `usize`, every movement from
//! -1000 to 1000 and the ends of `int`, and every pair of 17 names (empty,
//! case, prefixes, trailing spaces, invalid UTF-8) with an unnamed filter; a
//! model of `move_filter_dir()` with its walks and the retarget of the
//! whole chain, the shims on one side and the C on the other, over every
//! chain of up to 7 filters with every async pattern, every index and every
//! movement from -1 to 5 (10,766 moves); a model of the restore loop; and
//! proptests over any flag words, counts, indices, movements and name
//! lists.
//!
//! Intentional differences (#2981), and the only inputs where the shim and
//! the C differ:
//! 1. `obs_source_filter_chain_ref_step(false)`: the C gives
//!    `OBS_SOURCE_FILTER_CHAIN_REF_RETURN` (`obs_source_filter_add()`
//!    returns with `filter_mutex` locked), the shim
//!    `OBS_SOURCE_FILTER_CHAIN_REF_UNLOCK_RETURN` (it unlocks first);
//! 2. `obs_source_filter_chain_add_allowed(false, true)`, a filter that has a
//!    parent and is not listed: the C takes it (it joins a second chain), the
//!    shim refuses it;
//! 3. `obs_source_filter_chain_move_route()` for a movement outside
//!    `enum obs_order_movement`: the C gives
//!    `OBS_SOURCE_FILTER_CHAIN_ROUTE_RETARGET` (every filter retargeted and
//!    a move reported, so `reorder_filters` is signalled), the shim
//!    `OBS_SOURCE_FILTER_CHAIN_ROUTE_STAY` (nothing moved or reported);
//! 4. `obs_source_filter_chain_restore_step(false)`: the C gives
//!    `OBS_SOURCE_FILTER_CHAIN_RESTORE_LOAD` (`obs_load_source()`, public
//!    unless the type is a registered filter), the shim
//!    `OBS_SOURCE_FILTER_CHAIN_RESTORE_LOAD_PRIVATE`
//!    (`obs_load_private_source()`).
//!
//! `obs_source_filter_chain_restore_matches()` with a NULL backed-up name
//! is not compared: the C passes it to `strcmp()` (undefined), and its
//! caller never does (`obs_data_get_string()` never returns NULL); the shim
//! answers false.
//!
//! `test_source_filter_chain_steps` runs the C test against the C,
//! `test_source_filter_chain_steps_rust` against these shims; the
//! real-libobs filter tests run the callers through the hybrid libobs,
//! where these shims replace the C.

use std::ffi::{CString, c_char, c_int};
use std::ptr;

use obs_c_oracle::source_filter_chain::{
    oracle_obs_source_filter_chain_add_allowed, oracle_obs_source_filter_chain_add_target,
    oracle_obs_source_filter_chain_checks_compatible, oracle_obs_source_filter_chain_compatible,
    oracle_obs_source_filter_chain_edge_move, oracle_obs_source_filter_chain_move_route,
    oracle_obs_source_filter_chain_ref_step, oracle_obs_source_filter_chain_remove_retarget,
    oracle_obs_source_filter_chain_restore_matches, oracle_obs_source_filter_chain_restore_step,
    oracle_obs_source_filter_chain_same_kind, oracle_obs_source_filter_chain_target_of,
    oracle_obs_source_filter_chain_walk_step, oracle_source_filter_chain_layout,
};
use obs_source::ffi::source_filter_chain::{
    OBS_SOURCE_FILTER_CHAIN_REF_ADD, OBS_SOURCE_FILTER_CHAIN_REF_RETURN,
    OBS_SOURCE_FILTER_CHAIN_REF_UNLOCK_RETURN, OBS_SOURCE_FILTER_CHAIN_RESTORE_LOAD,
    OBS_SOURCE_FILTER_CHAIN_RESTORE_LOAD_PRIVATE, OBS_SOURCE_FILTER_CHAIN_RESTORE_REUSE,
    OBS_SOURCE_FILTER_CHAIN_ROUTE_BOTTOM, OBS_SOURCE_FILTER_CHAIN_ROUTE_DOWN,
    OBS_SOURCE_FILTER_CHAIN_ROUTE_RETARGET, OBS_SOURCE_FILTER_CHAIN_ROUTE_STAY,
    OBS_SOURCE_FILTER_CHAIN_ROUTE_TOP, OBS_SOURCE_FILTER_CHAIN_ROUTE_UP,
    OBS_SOURCE_FILTER_CHAIN_TARGET_FILTER, OBS_SOURCE_FILTER_CHAIN_TARGET_SOURCE,
    obs_source_filter_chain_add_allowed, obs_source_filter_chain_add_target,
    obs_source_filter_chain_checks_compatible, obs_source_filter_chain_compatible,
    obs_source_filter_chain_edge_move, obs_source_filter_chain_move_route,
    obs_source_filter_chain_ref_step, obs_source_filter_chain_remove_retarget,
    obs_source_filter_chain_restore_matches, obs_source_filter_chain_restore_step,
    obs_source_filter_chain_same_kind, obs_source_filter_chain_target_of,
    obs_source_filter_chain_walk_step,
};
use obs_source::source_filter_chain::{
    self as core, ASYNC, AUDIO, MOVE_BOTTOM, MOVE_DOWN, MOVE_TOP, MOVE_UP, VIDEO,
};
use proptest::prelude::*;

const BOOLS: [bool; 2] = [false, true];

/// A value the functions never store, to see that an out-parameter was
/// left alone.
const UNSET: usize = 0x5eed;

fn layout(which: usize) -> usize {
    // SAFETY: the oracle reads nothing and returns a constant.
    unsafe { oracle_source_filter_chain_layout(which) }
}

fn as_usize(value: c_int) -> usize {
    usize::try_from(value).expect("a non-negative header value")
}

/// The flag words compared: every combination of the video, audio and
/// async bits, alone, with every other bit, and with a few others.
fn flag_words() -> Vec<u32> {
    let mut out = Vec::new();
    for low in 0..8u32 {
        for high in [0, 0xffff_fff8, 1 << 3, 1 << 7, 1 << 31, 0x1234_5670] {
            out.push(low | high);
        }
    }
    out.push(u32::MAX);
    out
}

/// The `size_t` values compared for counts and indices.
fn sizes() -> Vec<usize> {
    let mut out: Vec<usize> = (0..=20).collect();
    out.extend([1000, usize::MAX - 2, usize::MAX - 1, usize::MAX]);
    out
}

/// The `int` values compared for movements.
fn movements() -> Vec<c_int> {
    let mut out: Vec<c_int> = (-1000..=1000).collect();
    out.extend([c_int::MIN, c_int::MIN + 1, c_int::MAX - 1, c_int::MAX]);
    out
}

fn shim_retarget(idx: usize) -> (bool, usize) {
    let mut prev = UNSET;
    // SAFETY: `prev` is a live local.
    let r = unsafe { obs_source_filter_chain_remove_retarget(idx, &raw mut prev) };
    (r, prev)
}

fn c_retarget(idx: usize) -> (bool, usize) {
    let mut prev = UNSET;
    // SAFETY: `prev` is a live local.
    let r = unsafe { oracle_obs_source_filter_chain_remove_retarget(idx, &raw mut prev) };
    (r, prev)
}

fn shim_walk(route: c_int, cur: usize, count: usize) -> (bool, usize) {
    let mut next = UNSET;
    // SAFETY: `next` is a live local.
    let r = unsafe { obs_source_filter_chain_walk_step(route, cur, count, &raw mut next) };
    (r, next)
}

fn c_walk(route: c_int, cur: usize, count: usize) -> (bool, usize) {
    let mut next = UNSET;
    // SAFETY: `next` is a live local.
    let r = unsafe { oracle_obs_source_filter_chain_walk_step(route, cur, count, &raw mut next) };
    (r, next)
}

fn shim_edge(route: c_int, idx: usize, count: usize) -> (bool, usize) {
    let mut to = UNSET;
    // SAFETY: `to` is a live local.
    let r = unsafe { obs_source_filter_chain_edge_move(route, idx, count, &raw mut to) };
    (r, to)
}

fn c_edge(route: c_int, idx: usize, count: usize) -> (bool, usize) {
    let mut to = UNSET;
    // SAFETY: `to` is a live local.
    let r = unsafe { oracle_obs_source_filter_chain_edge_move(route, idx, count, &raw mut to) };
    (r, to)
}

fn shim_matches(cur: Option<&CString>, name: &CString) -> bool {
    let cur = cur.map_or(ptr::null(), |c| c.as_ptr());
    // SAFETY: NULL or live NUL-terminated strings.
    unsafe { obs_source_filter_chain_restore_matches(cur, name.as_ptr()) }
}

fn c_matches(cur: Option<&CString>, name: &CString) -> bool {
    let cur: *const c_char = cur.map_or(ptr::null(), |c| c.as_ptr());
    // SAFETY: NULL or a live NUL-terminated string, and a live one.
    unsafe { oracle_obs_source_filter_chain_restore_matches(cur, name.as_ptr()) }
}

/// Every C answer equals the shim's, but for the intentional differences,
/// which are checked to be exactly those.
fn check_movement(movement: c_int) {
    // SAFETY: plain values in and out.
    let (shim, c) = unsafe {
        (
            obs_source_filter_chain_move_route(movement),
            oracle_obs_source_filter_chain_move_route(movement),
        )
    };
    if (MOVE_UP..=MOVE_BOTTOM).contains(&movement) {
        assert_eq!(shim, c, "movement {movement}");
    } else {
        // intentional difference 3
        assert_eq!(
            c, OBS_SOURCE_FILTER_CHAIN_ROUTE_RETARGET,
            "movement {movement}"
        );
        assert_eq!(
            shim, OBS_SOURCE_FILTER_CHAIN_ROUTE_STAY,
            "movement {movement}"
        );
    }
}

#[test]
fn header_values_match() {
    let values = [
        usize::try_from(VIDEO).expect("small"),
        usize::try_from(AUDIO).expect("small"),
        usize::try_from(ASYNC).expect("small"),
        as_usize(MOVE_UP),
        as_usize(MOVE_DOWN),
        as_usize(MOVE_TOP),
        as_usize(MOVE_BOTTOM),
        as_usize(OBS_SOURCE_FILTER_CHAIN_REF_ADD),
        as_usize(OBS_SOURCE_FILTER_CHAIN_REF_RETURN),
        as_usize(OBS_SOURCE_FILTER_CHAIN_REF_UNLOCK_RETURN),
        as_usize(OBS_SOURCE_FILTER_CHAIN_TARGET_SOURCE),
        as_usize(OBS_SOURCE_FILTER_CHAIN_TARGET_FILTER),
        as_usize(OBS_SOURCE_FILTER_CHAIN_ROUTE_UP),
        as_usize(OBS_SOURCE_FILTER_CHAIN_ROUTE_DOWN),
        as_usize(OBS_SOURCE_FILTER_CHAIN_ROUTE_TOP),
        as_usize(OBS_SOURCE_FILTER_CHAIN_ROUTE_BOTTOM),
        as_usize(OBS_SOURCE_FILTER_CHAIN_ROUTE_RETARGET),
        as_usize(OBS_SOURCE_FILTER_CHAIN_ROUTE_STAY),
        as_usize(OBS_SOURCE_FILTER_CHAIN_RESTORE_REUSE),
        as_usize(OBS_SOURCE_FILTER_CHAIN_RESTORE_LOAD),
        as_usize(OBS_SOURCE_FILTER_CHAIN_RESTORE_LOAD_PRIVATE),
        size_of::<c_int>(),
        size_of::<c_int>(),
        size_of::<c_int>(),
        size_of::<c_int>(),
        size_of::<bool>(),
        size_of::<usize>(),
    ];
    for (i, value) in values.iter().enumerate() {
        assert_eq!(layout(i), *value, "layout value {i}");
    }
    assert_eq!(layout(values.len()), 0);
}

#[test]
fn flags_match_exhaustively() {
    let words = flag_words();
    for &s in &words {
        for &f in &words {
            // SAFETY: plain values in and out.
            unsafe {
                assert_eq!(
                    obs_source_filter_chain_compatible(s, f),
                    oracle_obs_source_filter_chain_compatible(s, f),
                    "compatible({s:#x}, {f:#x})"
                );
                assert_eq!(
                    obs_source_filter_chain_same_kind(s, f),
                    oracle_obs_source_filter_chain_same_kind(s, f),
                    "same_kind({s:#x}, {f:#x})"
                );
            }
            assert_eq!(
                core::compatible(s, f),
                obs_source_filter_chain_compatible(s, f)
            );
        }
    }
    // every single bit, as source and as filter
    for bit in 0..32 {
        let b = 1u32 << bit;
        for (s, f) in [(b, b), (0, b), (b, 0), (!b, b), (b, !b)] {
            // SAFETY: plain values in and out.
            unsafe {
                assert_eq!(
                    obs_source_filter_chain_compatible(s, f),
                    oracle_obs_source_filter_chain_compatible(s, f)
                );
                assert_eq!(
                    obs_source_filter_chain_same_kind(s, f),
                    oracle_obs_source_filter_chain_same_kind(s, f)
                );
            }
        }
    }
}

#[test]
fn add_gates_match_but_for_the_fixes() {
    for listed in BOOLS {
        for has_parent in BOOLS {
            // SAFETY: plain values in and out.
            let (shim, c) = unsafe {
                (
                    obs_source_filter_chain_add_allowed(listed, has_parent),
                    oracle_obs_source_filter_chain_add_allowed(listed, has_parent),
                )
            };
            if !listed && has_parent {
                // intentional difference 2
                assert!(c);
                assert!(!shim);
            } else {
                assert_eq!(shim, c, "add_allowed({listed}, {has_parent})");
            }
        }
    }
    for b in BOOLS {
        // SAFETY: plain values in and out.
        unsafe {
            assert_eq!(
                obs_source_filter_chain_checks_compatible(b),
                oracle_obs_source_filter_chain_checks_compatible(b)
            );
        }
    }

    // SAFETY: plain values in and out.
    unsafe {
        assert_eq!(
            obs_source_filter_chain_ref_step(true),
            oracle_obs_source_filter_chain_ref_step(true)
        );
        // intentional difference 1
        assert_eq!(
            oracle_obs_source_filter_chain_ref_step(false),
            OBS_SOURCE_FILTER_CHAIN_REF_RETURN
        );
        assert_eq!(
            obs_source_filter_chain_ref_step(false),
            OBS_SOURCE_FILTER_CHAIN_REF_UNLOCK_RETURN
        );

        assert_eq!(
            obs_source_filter_chain_restore_step(true),
            oracle_obs_source_filter_chain_restore_step(true)
        );
        // intentional difference 4
        assert_eq!(
            oracle_obs_source_filter_chain_restore_step(false),
            OBS_SOURCE_FILTER_CHAIN_RESTORE_LOAD
        );
        assert_eq!(
            obs_source_filter_chain_restore_step(false),
            OBS_SOURCE_FILTER_CHAIN_RESTORE_LOAD_PRIVATE
        );
    }
}

#[test]
fn positions_match_exhaustively() {
    let sizes = sizes();
    for &a in &sizes {
        // SAFETY: plain values in and out.
        unsafe {
            assert_eq!(
                obs_source_filter_chain_add_target(a),
                oracle_obs_source_filter_chain_add_target(a),
                "add_target({a})"
            );
        }
        assert_eq!(shim_retarget(a), c_retarget(a), "remove_retarget({a})");

        for &b in &sizes {
            // SAFETY: plain values in and out.
            unsafe {
                assert_eq!(
                    obs_source_filter_chain_target_of(a, b),
                    oracle_obs_source_filter_chain_target_of(a, b),
                    "target_of({a}, {b})"
                );
            }
            for route in -2..=7 {
                assert_eq!(
                    shim_walk(route, a, b),
                    c_walk(route, a, b),
                    "walk_step({route}, {a}, {b})"
                );
                assert_eq!(
                    shim_edge(route, a, b),
                    c_edge(route, a, b),
                    "edge_move({route}, {a}, {b})"
                );
            }
        }
    }
}

#[test]
fn movements_match_but_for_the_fix() {
    for movement in movements() {
        check_movement(movement);
    }
}

#[test]
fn names_match_exhaustively() {
    let names: Vec<CString> = [
        &b""[..],
        b"A",
        b"a",
        b"A ",
        b" A",
        b"AB",
        b"Color",
        b"Color 2",
        b"Color Correction",
        b"color",
        b"\xff",
        b"\xff\xfe",
        b"\xc3\xa9",
        b"e\xcc\x81",
        b"\x01",
        b"Crop/Pad",
        b"Crop/Pad\x7f",
    ]
    .iter()
    .map(|n| CString::new(*n).expect("no NUL"))
    .collect();

    for name in &names {
        assert_eq!(shim_matches(None, name), c_matches(None, name));
        assert!(!shim_matches(None, name));
        for cur in &names {
            assert_eq!(
                shim_matches(Some(cur), name),
                c_matches(Some(cur), name),
                "restore_matches({cur:?}, {name:?})"
            );
        }
    }

    // the shim's answer for a NULL name, which the C never gets
    let cur = CString::new("A").expect("no NUL");
    // SAFETY: a live string and NULL.
    unsafe {
        assert!(!obs_source_filter_chain_restore_matches(
            cur.as_ptr(),
            ptr::null()
        ));
        assert!(!obs_source_filter_chain_restore_matches(
            ptr::null(),
            ptr::null()
        ));
    }
}

// ---------------------------------------------------------------------------
// move_filter_dir(), with each side's functions

struct Side {
    route: fn(c_int) -> c_int,
    walk: fn(c_int, usize, usize) -> (bool, usize),
    same_kind: fn(u32, u32) -> bool,
    edge: fn(c_int, usize, usize) -> (bool, usize),
    target_of: fn(usize, usize) -> c_int,
}

fn shim_route(m: c_int) -> c_int {
    obs_source_filter_chain_move_route(m)
}
fn c_route(m: c_int) -> c_int {
    // SAFETY: a plain value in and out.
    unsafe { oracle_obs_source_filter_chain_move_route(m) }
}
fn shim_same(a: u32, b: u32) -> bool {
    obs_source_filter_chain_same_kind(a, b)
}
fn c_same(a: u32, b: u32) -> bool {
    // SAFETY: plain values in and out.
    unsafe { oracle_obs_source_filter_chain_same_kind(a, b) }
}
fn shim_target(i: usize, n: usize) -> c_int {
    obs_source_filter_chain_target_of(i, n)
}
fn c_target(i: usize, n: usize) -> c_int {
    // SAFETY: plain values in and out.
    unsafe { oracle_obs_source_filter_chain_target_of(i, n) }
}

const SHIM: Side = Side {
    route: shim_route,
    walk: shim_walk,
    same_kind: shim_same,
    edge: shim_edge,
    target_of: shim_target,
};
const C: Side = Side {
    route: c_route,
    walk: c_walk,
    same_kind: c_same,
    edge: c_edge,
    target_of: c_target,
};

/// The order and the targets (`None` for the source) after a move, and
/// whether a move was reported.
#[derive(Debug, PartialEq, Eq)]
struct Moved {
    reported: bool,
    order: Vec<usize>,
    targets: Vec<Option<usize>>,
}

/// `find_next_filter()` and `find_prev_filter()`: recursive in the C, a
/// loop here, one walk step and one kind compare per filter.
fn walk(side: &Side, route: c_int, flags: &[u32], order: &[usize], idx: usize) -> Option<usize> {
    let mut cur = idx;
    loop {
        let (more, next) = (side.walk)(route, cur, order.len());
        if !more {
            return None;
        }
        if (side.same_kind)(flags[order[idx]], flags[order[next]]) {
            return Some(next);
        }
        cur = next;
    }
}

/// `move_filter_dir()` and `reorder_filter_targets()` as obs-source.c.h
/// calls them, for the filter at `idx`. Stale targets (all `Some(99)`)
/// stay when nothing is retargeted.
fn move_filter(side: &Side, flags: &[u32], idx: usize, movement: c_int) -> Moved {
    let mut order: Vec<usize> = (0..flags.len()).collect();
    let mut targets = vec![Some(99); flags.len()];
    let route = (side.route)(movement);

    let to = if route == OBS_SOURCE_FILTER_CHAIN_ROUTE_UP
        || route == OBS_SOURCE_FILTER_CHAIN_ROUTE_DOWN
    {
        walk(side, route, flags, &order, idx)
    } else if route == OBS_SOURCE_FILTER_CHAIN_ROUTE_TOP
        || route == OBS_SOURCE_FILTER_CHAIN_ROUTE_BOTTOM
    {
        let (moves, to) = (side.edge)(route, idx, order.len());
        moves.then_some(to)
    } else if route == OBS_SOURCE_FILTER_CHAIN_ROUTE_STAY {
        None
    } else {
        Some(idx)
    };
    let Some(to) = to else {
        return Moved {
            reported: false,
            order,
            targets,
        };
    };

    let item = order.remove(idx);
    order.insert(to, item);
    let n = order.len();
    for (i, t) in targets.iter_mut().enumerate() {
        *t = if (side.target_of)(i, n) == OBS_SOURCE_FILTER_CHAIN_TARGET_SOURCE {
            None
        } else {
            Some(order[i + 1])
        };
    }
    Moved {
        reported: true,
        order,
        targets,
    }
}

fn compare_move(flags: &[u32], idx: usize, movement: c_int) -> usize {
    let shim = move_filter(&SHIM, flags, idx, movement);
    let c = move_filter(&C, flags, idx, movement);
    if (MOVE_UP..=MOVE_BOTTOM).contains(&movement) {
        assert_eq!(shim, c, "flags {flags:?} idx {idx} movement {movement}");
        return 1;
    }
    // intentional difference 3: the C retargets the unchanged chain and
    // reports a move; the shim does neither
    let unchanged: Vec<usize> = (0..flags.len()).collect();
    assert!(c.reported);
    assert_eq!(c.order, unchanged);
    assert!(!shim.reported);
    assert_eq!(shim.order, unchanged);
    assert!(shim.targets.iter().all(|t| *t == Some(99)));
    0
}

#[test]
fn moves_match_over_every_small_chain() {
    let mut compared = 0;
    let mut differ = 0;
    for n in 1..=7usize {
        for pattern in 0..(1u32 << n) {
            let flags: Vec<u32> = (0..n)
                .map(|i| {
                    if pattern >> i & 1 == 1 {
                        ASYNC | VIDEO
                    } else {
                        VIDEO
                    }
                })
                .collect();
            for idx in 0..n {
                for movement in -1..=5 {
                    let same = compare_move(&flags, idx, movement);
                    compared += same;
                    differ += 1 - same;
                }
            }
        }
    }
    assert_eq!(compared + differ, 10_766);
    assert_eq!(compared, 6_152);
}

// ---------------------------------------------------------------------------
// The restore loop, with each side's functions

/// For each backed-up name, the index of the current filter the loop keeps
/// and the restore step it then takes.
fn restore_plan(
    matches: fn(Option<&CString>, &CString) -> bool,
    step: fn(bool) -> c_int,
    backup: &[CString],
    current: &[Option<CString>],
) -> Vec<(Option<usize>, c_int)> {
    backup
        .iter()
        .map(|name| {
            let found = current.iter().position(|cur| matches(cur.as_ref(), name));
            (found, step(found.is_some()))
        })
        .collect()
}

fn shim_step(found: bool) -> c_int {
    obs_source_filter_chain_restore_step(found)
}
fn c_step(found: bool) -> c_int {
    // SAFETY: a plain value in and out.
    unsafe { oracle_obs_source_filter_chain_restore_step(found) }
}

fn compare_restore(backup: &[CString], current: &[Option<CString>]) {
    let shim = restore_plan(shim_matches, shim_step, backup, current);
    let c = restore_plan(c_matches, c_step, backup, current);
    for ((s_found, s_step), (c_found, c_step)) in shim.iter().zip(&c) {
        assert_eq!(s_found, c_found);
        if c_found.is_some() {
            assert_eq!(s_step, c_step);
        } else {
            // intentional difference 4
            assert_eq!(*c_step, OBS_SOURCE_FILTER_CHAIN_RESTORE_LOAD);
            assert_eq!(*s_step, OBS_SOURCE_FILTER_CHAIN_RESTORE_LOAD_PRIVATE);
        }
    }
}

fn names(list: &[&str]) -> Vec<CString> {
    list.iter()
        .map(|n| CString::new(*n).expect("no NUL"))
        .collect()
}

#[test]
fn restore_loops_match() {
    let current: Vec<Option<CString>> = vec![
        Some(CString::new("Color").expect("no NUL")),
        None,
        Some(CString::new("Crop").expect("no NUL")),
        Some(CString::new("Crop").expect("no NUL")),
        Some(CString::new("").expect("no NUL")),
    ];
    compare_restore(
        &names(&["Crop", "Color", "Mask", "", "Crop", "crop"]),
        &current,
    );
    compare_restore(&names(&["Crop"]), &[]);
    compare_restore(&[], &current);
}

fn name_strategy() -> impl Strategy<Value = CString> {
    prop::collection::vec(
        prop_oneof![Just(b'A'), Just(b'a'), Just(b' '), Just(0xffu8)],
        0..3,
    )
    .prop_map(|v| CString::new(v).expect("no NUL"))
}

proptest! {
    #[test]
    fn prop_flags_match(s in any::<u32>(), f in any::<u32>()) {
        // SAFETY: plain values in and out.
        unsafe {
            prop_assert_eq!(obs_source_filter_chain_compatible(s, f), oracle_obs_source_filter_chain_compatible(s, f));
            prop_assert_eq!(obs_source_filter_chain_same_kind(s, f), oracle_obs_source_filter_chain_same_kind(s, f));
        }
    }

    #[test]
    fn prop_positions_match(a in any::<usize>(), b in any::<usize>(), route in any::<c_int>()) {
        // SAFETY: plain values in and out.
        unsafe {
            prop_assert_eq!(obs_source_filter_chain_add_target(a), oracle_obs_source_filter_chain_add_target(a));
            prop_assert_eq!(obs_source_filter_chain_target_of(a, b), oracle_obs_source_filter_chain_target_of(a, b));
        }
        prop_assert_eq!(shim_retarget(a), c_retarget(a));
        prop_assert_eq!(shim_walk(route, a, b), c_walk(route, a, b));
        prop_assert_eq!(shim_edge(route, a, b), c_edge(route, a, b));
    }

    #[test]
    fn prop_movements_match(movement in any::<c_int>()) {
        check_movement(movement);
    }

    #[test]
    fn prop_moves_match(
        flags in prop::collection::vec(any::<u32>(), 1..=12),
        pick in any::<prop::sample::Index>(),
        movement in -3i32..=6,
    ) {
        compare_move(&flags, pick.index(flags.len()), movement);
    }

    #[test]
    fn prop_restore_loops_match(
        backup in prop::collection::vec(name_strategy(), 0..8),
        current in prop::collection::vec(prop::option::of(name_strategy()), 0..8),
    ) {
        compare_restore(&backup, &current);
    }
}
