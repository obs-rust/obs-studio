//! Tier 3 parity: the shims of `obs_scene::ffi::scene_item_order` (and the
//! safe core under them) against the original
//! `libobs/obs-scene-item-order.c.h`, compiled as
//! `oracle_obs_scene_item_order_*` (obs-c-oracle,
//! oracle/scene_item_order.c).
//!
//! Every call runs the shim and the C and compares the results and every
//! value written through a pointer; the safe core is checked against the
//! shim where the types differ.
//!
//! Runs: exhaustive over the movements around and at the ends of the `int`
//! range, the positions -10 to 10 and the ends, every flag, every order of
//! up to 5 entries over 4 items for the match step, every order of up to 4
//! entries over the scene's items, an item from elsewhere and the groups
//! for the pair check, every order of up to 5 entries over 9 pairs for
//! each step of the group runs, every scene pair and triple over NULL and
//! three addresses for the add and active-child gates and the ends of
//! `long` for the active count; a model of `obs_scene_reorder_items2()`'s relinking with
//! the shims on one side and the C on the other; and proptests over the
//! full ranges.
//!
//! Intentional differences (#2895), and the only inputs where the shim and
//! the C differ:
//! 1. a movement outside `enum obs_order_movement`: the C gives
//!    `OBS_SCENE_ITEM_ORDER_ANCHOR_NONE` (its caller detaches the item and
//!    never attaches it again), the shim `OBS_SCENE_ITEM_ORDER_ANCHOR_STAY`
//!    (the item stays where it is);
//! 2. `obs_scene_item_order_item_attached()` for a NULL or detached item:
//!    the C says go on (its caller then reads through it), the shim stops;
//! 3. `obs_scene_item_order_reorder_check()` for an order whose items are
//!    not the scene's items, each once: the C gives
//!    `OBS_SCENE_ITEM_ORDER_APPLY` (its caller relinks it), the shim
//!    `OBS_SCENE_ITEM_ORDER_INVALID` (refused);
//! 4. `obs_scene_item_order_remove_item_allowed()` for an item not in the
//!    group: the C moves it, the shim does not.
//!
//! `test_scene_item_order_steps` runs the C test against the C,
//! `test_scene_item_order_steps_rust` against these shims; the real-libobs
//! scene and group tests run the callers through the hybrid libobs, where
//! these shims replace the C.

use std::ffi::{c_int, c_long, c_void};

use obs_c_oracle::scene_item_order::{
    OracleOrderInfo, oracle_obs_scene_item_order_add_item_allowed,
    oracle_obs_scene_item_order_anchor, oracle_obs_scene_item_order_in_group_run,
    oracle_obs_scene_item_order_item_active, oracle_obs_scene_item_order_item_attached,
    oracle_obs_scene_item_order_match_item, oracle_obs_scene_item_order_moves_scene,
    oracle_obs_scene_item_order_position, oracle_obs_scene_item_order_remove_item_allowed,
    oracle_obs_scene_item_order_reorder_check, oracle_scene_item_order_layout,
};
use obs_scene::ffi::scene_item_order::{
    OBS_SCENE_ITEM_ORDER_ANCHOR_BOTTOM, OBS_SCENE_ITEM_ORDER_ANCHOR_LAST,
    OBS_SCENE_ITEM_ORDER_ANCHOR_NEXT, OBS_SCENE_ITEM_ORDER_ANCHOR_PREV,
    OBS_SCENE_ITEM_ORDER_ANCHOR_PREV_PREV, OBS_SCENE_ITEM_ORDER_ANCHOR_STAY,
    OBS_SCENE_ITEM_ORDER_APPLY, OBS_SCENE_ITEM_ORDER_INVALID, OBS_SCENE_ITEM_ORDER_SAME, OrderInfo,
    obs_scene_item_order_add_item_allowed, obs_scene_item_order_anchor,
    obs_scene_item_order_in_group_run, obs_scene_item_order_item_active,
    obs_scene_item_order_item_attached, obs_scene_item_order_match_item,
    obs_scene_item_order_moves_scene, obs_scene_item_order_position,
    obs_scene_item_order_remove_item_allowed, obs_scene_item_order_reorder_check,
};
use obs_scene::item_order::{
    self as core, OBS_ORDER_MOVE_BOTTOM, OBS_ORDER_MOVE_DOWN, OBS_ORDER_MOVE_TOP, OBS_ORDER_MOVE_UP,
};
use proptest::prelude::*;

/// `OBS_SCENE_ITEM_ORDER_ANCHOR_NONE`: only the C gives it.
const ANCHOR_NONE: c_int = 0;

/// Distinct fake item addresses; only compared, never followed.
static SLOTS: [u8; 16] = [0; 16];

fn item(i: usize) -> *const c_void {
    std::ptr::from_ref(&SLOTS[i]).cast()
}

fn info(group: Option<usize>, it: usize) -> OrderInfo {
    OrderInfo {
        group: group.map_or(std::ptr::null(), item),
        item: item(it),
    }
}

fn oracle_pairs(pairs: &[OrderInfo]) -> Vec<OracleOrderInfo> {
    pairs
        .iter()
        .map(|p| OracleOrderInfo {
            group: p.group,
            item: p.item,
        })
        .collect()
}

fn known(movement: c_int) -> bool {
    [
        OBS_ORDER_MOVE_UP,
        OBS_ORDER_MOVE_DOWN,
        OBS_ORDER_MOVE_TOP,
        OBS_ORDER_MOVE_BOTTOM,
    ]
    .contains(&movement)
}

// ---------------------------------------------------------------------------
// one call on both sides

fn check_anchor(movement: c_int, has_prev: bool, has_next: bool) {
    let shim = obs_scene_item_order_anchor(movement, has_prev, has_next);
    // SAFETY: plain values in, an int out; the oracle keeps no state.
    let c = unsafe { oracle_obs_scene_item_order_anchor(movement, has_prev, has_next) };
    let what = format!("anchor({movement}, {has_prev}, {has_next})");
    if known(movement) {
        assert_eq!(shim, c, "{what}");
    } else {
        // difference 1
        assert_eq!(c, ANCHOR_NONE, "{what}");
        assert_eq!(shim, OBS_SCENE_ITEM_ORDER_ANCHOR_STAY, "{what}");
    }
}

fn check_position(position: c_int, has_first: bool) {
    const SENTINEL: usize = 0xdead_beef;
    let mut shim_steps = SENTINEL;
    let mut c_steps = SENTINEL;
    // SAFETY: a local, written only by the call.
    let shim = unsafe { obs_scene_item_order_position(position, has_first, &mut shim_steps) };
    // SAFETY: a local, written only by the call; the oracle keeps no state.
    let c = unsafe { oracle_obs_scene_item_order_position(position, has_first, &mut c_steps) };
    let what = format!("position({position}, {has_first})");
    assert_eq!(shim, c, "{what}");
    assert_eq!(shim_steps, c_steps, "{what}: steps");
    let core = core::position(position, has_first);
    match core {
        core::Position::Bottom => assert!(shim && shim_steps == SENTINEL, "{what}: core"),
        core::Position::After { steps } => {
            assert!(!shim && shim_steps == steps, "{what}: core");
        }
    }
}

fn check_match(order: &[usize], it: usize, index: usize, start: bool) {
    let ptrs: Vec<*const c_void> = order.iter().map(|&i| item(i)).collect();
    let data = if ptrs.is_empty() {
        std::ptr::null()
    } else {
        ptrs.as_ptr()
    };
    let mut shim_matches = start;
    let mut c_matches = start;
    // SAFETY: `ptrs.len()` pointers at `data` (NULL for none), and a local.
    let shim = unsafe {
        obs_scene_item_order_match_item(data, ptrs.len(), item(it), index, &mut shim_matches)
    };
    // SAFETY: as above; the oracle keeps no state.
    let c = unsafe {
        oracle_obs_scene_item_order_match_item(data, ptrs.len(), item(it), index, &mut c_matches)
    };
    let what = format!("match({order:?}, {it}, {index}, {start})");
    assert_eq!(shim, c, "{what}");
    assert_eq!(shim_matches, c_matches, "{what}: order_matches");
}

/// Whether the items of `order` are those of `current`, each once.
fn rearranged(current: &[OrderInfo], order: &[OrderInfo]) -> bool {
    let mut a: Vec<usize> = current.iter().map(|p| p.item.addr()).collect();
    let mut b: Vec<usize> = order.iter().map(|p| p.item.addr()).collect();
    a.sort_unstable();
    b.sort_unstable();
    a == b
}

fn call_check(current: &[OrderInfo], order: &[OrderInfo]) -> (c_int, c_int) {
    let ptr = |v: &[OrderInfo]| {
        if v.is_empty() {
            std::ptr::null()
        } else {
            v.as_ptr()
        }
    };
    let (oc, oo) = (oracle_pairs(current), oracle_pairs(order));
    let optr = |v: &[OracleOrderInfo]| {
        if v.is_empty() {
            std::ptr::null()
        } else {
            v.as_ptr()
        }
    };
    // SAFETY: each array holds its length in pairs (NULL for none).
    let shim = unsafe {
        obs_scene_item_order_reorder_check(ptr(current), current.len(), ptr(order), order.len())
    };
    // SAFETY: as above; the oracle keeps no state.
    let c = unsafe {
        oracle_obs_scene_item_order_reorder_check(optr(&oc), oc.len(), optr(&oo), oo.len())
    };
    (shim, c)
}

fn check_reorder(current: &[OrderInfo], order: &[OrderInfo]) {
    let (shim, c) = call_check(current, order);
    let what = format!("check({current:?}, {order:?})");
    if c == OBS_SCENE_ITEM_ORDER_APPLY && !rearranged(current, order) {
        // difference 3
        assert_eq!(shim, OBS_SCENE_ITEM_ORDER_INVALID, "{what}");
    } else {
        assert_eq!(shim, c, "{what}");
    }
}

/// Every step of a run from `head`: each index of the order and the two past
/// it (which neither side reads).
fn check_run_end(order: &[OrderInfo], head: usize) {
    let oo = oracle_pairs(order);
    for index in 0..order.len() + 2 {
        // SAFETY: `order.len()` pairs, head < len; an index past the end is
        // not read.
        let shim =
            unsafe { obs_scene_item_order_in_group_run(order.as_ptr(), order.len(), head, index) };
        // SAFETY: as above; the oracle keeps no state.
        let c =
            unsafe { oracle_obs_scene_item_order_in_group_run(oo.as_ptr(), oo.len(), head, index) };
        assert_eq!(shim, c, "in_group_run({order:?}, {head}, {index})");
    }
}

/// The caller's run end, one step at a time, on the shim or the C side.
fn run_end(order: &[OrderInfo], head: usize, use_c: bool) -> usize {
    let oo = oracle_pairs(order);
    let mut i = head + 1;
    loop {
        // SAFETY: `order.len()` pairs, head < len; an index past the end is
        // not read; the oracle keeps no state.
        let more = unsafe {
            if use_c {
                oracle_obs_scene_item_order_in_group_run(oo.as_ptr(), oo.len(), head, i)
            } else {
                obs_scene_item_order_in_group_run(order.as_ptr(), order.len(), head, i)
            }
        };
        if !more {
            return i;
        }
        i += 1;
    }
}

// ---------------------------------------------------------------------------
// layout

#[test]
fn layout_matches_the_c() {
    // SAFETY: the oracle's table reader; any index is fine.
    let c = |which| unsafe { oracle_scene_item_order_layout(which) };
    assert_eq!(c(0), size_of::<OrderInfo>());
    assert_eq!(c(1), align_of::<OrderInfo>());
    assert_eq!(c(2), std::mem::offset_of!(OrderInfo, group));
    assert_eq!(c(3), std::mem::offset_of!(OrderInfo, item));
    assert_eq!(
        c(4),
        size_of::<c_int>(),
        "enum obs_order_movement is an int"
    );
    let value = |v: c_int| usize::try_from(v).unwrap();
    assert_eq!(c(5), value(OBS_ORDER_MOVE_UP));
    assert_eq!(c(6), value(OBS_ORDER_MOVE_DOWN));
    assert_eq!(c(7), value(OBS_ORDER_MOVE_TOP));
    assert_eq!(c(8), value(OBS_ORDER_MOVE_BOTTOM));
    assert_eq!(c(9), value(ANCHOR_NONE));
    assert_eq!(c(10), value(OBS_SCENE_ITEM_ORDER_ANCHOR_BOTTOM));
    assert_eq!(c(11), value(OBS_SCENE_ITEM_ORDER_ANCHOR_PREV_PREV));
    assert_eq!(c(12), value(OBS_SCENE_ITEM_ORDER_ANCHOR_NEXT));
    assert_eq!(c(13), value(OBS_SCENE_ITEM_ORDER_ANCHOR_PREV));
    assert_eq!(c(14), value(OBS_SCENE_ITEM_ORDER_ANCHOR_LAST));
    assert_eq!(c(15), value(OBS_SCENE_ITEM_ORDER_ANCHOR_STAY));
    assert_eq!(c(16), value(OBS_SCENE_ITEM_ORDER_SAME));
    assert_eq!(c(17), value(OBS_SCENE_ITEM_ORDER_APPLY));
    assert_eq!(c(18), value(OBS_SCENE_ITEM_ORDER_INVALID));
    assert_eq!(c(19), size_of::<usize>());
}

// ---------------------------------------------------------------------------
// exhaustive runs

const MOVEMENTS: [c_int; 14] = [
    c_int::MIN,
    c_int::MIN + 1,
    -100,
    -2,
    -1,
    0,
    1,
    2,
    3,
    4,
    5,
    100,
    c_int::MAX - 1,
    c_int::MAX,
];

#[test]
fn anchor_matches_c_for_every_movement() {
    for movement in MOVEMENTS {
        for has_prev in [false, true] {
            for has_next in [false, true] {
                check_anchor(movement, has_prev, has_next);
            }
        }
    }
}

#[test]
fn position_matches_c_for_every_position() {
    let ends = [c_int::MIN, c_int::MIN + 1, c_int::MAX - 1, c_int::MAX];
    for position in (-10..=10).chain(ends) {
        for has_first in [false, true] {
            check_position(position, has_first);
        }
    }
}

#[test]
fn gates_match_c_for_every_flag() {
    // the scenes: NULL and three addresses
    let scenes = [std::ptr::null(), item(10), item(11), item(12)];
    for &p in &scenes {
        for &s in &scenes {
            for &g in &scenes {
                // SAFETY: addresses only compared; the oracle keeps no state.
                let c = unsafe { oracle_obs_scene_item_order_add_item_allowed(p, s, g) };
                assert_eq!(
                    obs_scene_item_order_add_item_allowed(p, s, g),
                    c,
                    "add({p:?}, {s:?}, {g:?})"
                );
            }
            // SAFETY: as above.
            let c = unsafe { oracle_obs_scene_item_order_moves_scene(p, s) };
            assert_eq!(
                obs_scene_item_order_moves_scene(p, s),
                c,
                "moves({p:?}, {s:?})"
            );
        }
    }
    for refs in [
        c_long::MIN,
        c_long::MIN + 1,
        -2,
        -1,
        0,
        1,
        2,
        c_long::MAX - 1,
        c_long::MAX,
    ] {
        // SAFETY: a plain value; the oracle keeps no state.
        let c = unsafe { oracle_obs_scene_item_order_item_active(refs) };
        assert_eq!(obs_scene_item_order_item_active(refs), c, "active({refs})");
    }
    for a in [false, true] {
        for b in [false, true] {
            // SAFETY: plain values; the oracle keeps no state.
            let c = unsafe { oracle_obs_scene_item_order_item_attached(a, b) };
            let shim = obs_scene_item_order_item_attached(a, b);
            if a && b {
                assert_eq!(shim, c, "attached({a}, {b})");
            } else {
                // difference 2
                assert!(c && !shim, "attached({a}, {b})");
            }
        }
        // SAFETY: as above.
        let c = unsafe { oracle_obs_scene_item_order_remove_item_allowed(a) };
        let shim = obs_scene_item_order_remove_item_allowed(a);
        if a {
            assert_eq!(shim, c, "remove({a})");
        } else {
            // difference 4
            assert!(c && !shim, "remove({a})");
        }
    }
}

/// Every sequence of `len` symbols out of `alphabet`.
fn sequences(alphabet: usize, len: usize) -> impl Iterator<Item = Vec<usize>> {
    let count = alphabet.pow(u32::try_from(len).unwrap());
    (0..count).map(move |mut n| {
        (0..len)
            .map(|_| {
                let s = n % alphabet;
                n /= alphabet;
                s
            })
            .collect()
    })
}

#[test]
fn match_item_matches_c_for_every_short_order() {
    for len in 0..=5 {
        for order in sequences(4, len) {
            for it in 0..4 {
                for index in 0..7 {
                    for start in [false, true] {
                        check_match(&order, it, index, start);
                    }
                }
            }
        }
    }
}

/// The scenes of the pair check: item ids 0 to 3 top level or in group 4
/// or 5 (ids 4 and 5 are the group items).
fn scenes() -> Vec<Vec<OrderInfo>> {
    vec![
        vec![],
        vec![info(None, 0)],
        vec![info(None, 0), info(None, 1)],
        vec![info(None, 4), info(Some(4), 0), info(Some(4), 1)],
        vec![
            info(None, 0),
            info(None, 4),
            info(Some(4), 1),
            info(None, 2),
        ],
        vec![
            info(None, 4),
            info(Some(4), 0),
            info(None, 5),
            info(Some(5), 1),
        ],
    ]
}

#[test]
fn reorder_check_matches_c_for_every_short_order() {
    // entries: item 0-5 or 6 (from elsewhere), group none, 4 or 5
    let entry = |s: usize| info([None, Some(4), Some(5)][s % 3], s / 3);
    let mut invalid = 0usize;
    for current in scenes() {
        for len in 0..=4 {
            for symbols in sequences(21, len) {
                let order: Vec<OrderInfo> = symbols.iter().map(|&s| entry(s)).collect();
                check_reorder(&current, &order);
                if !rearranged(&current, &order) {
                    invalid += 1;
                }
            }
        }
        // the scene itself, and each of its pairs moved to the front
        check_reorder(&current, &current);
        for k in 0..current.len() {
            let mut order = current.clone();
            let moved = order.remove(k);
            order.insert(0, moved);
            check_reorder(&current, &order);
        }
    }
    assert!(invalid > 0);
}

#[test]
fn in_group_run_matches_c_for_every_short_order() {
    // entries over groups none, 4 and 5 and items 4, 5 and 0
    let entry = |s: usize| info([None, Some(4), Some(5)][s % 3], [4, 5, 0][s / 3]);
    for len in 1..=5 {
        for symbols in sequences(9, len) {
            let order: Vec<OrderInfo> = symbols.iter().map(|&s| entry(s)).collect();
            for head in 0..len {
                check_run_end(&order, head);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// a model of obs_scene_reorder_items2()'s relinking

/// The relinking of `obs_scene_reorder_items2()` over item ids, after its
/// check: each group entry takes the run after it (`in_group_run`, asked
/// one entry at a time), every
/// other entry goes to the top level. Returns the scene as (group, item)
/// pairs, or `None` when the check refuses the order.
fn relink(
    current: &[OrderInfo],
    order: &[OrderInfo],
    is_group: impl Fn(*const c_void) -> bool,
    use_c: bool,
) -> Option<Vec<(Option<usize>, usize)>> {
    let (shim, c) = call_check(current, order);
    if (if use_c { c } else { shim }) != OBS_SCENE_ITEM_ORDER_APPLY {
        return None;
    }
    let mut out = Vec::new();
    let mut i = 0;
    while i < order.len() {
        let head = order[i].item;
        out.push((None, head.addr()));
        if is_group(head) {
            let end = run_end(order, i, use_c);
            for sub in &order[i + 1..end] {
                out.push((Some(head.addr()), sub.item.addr()));
            }
            i = end;
        } else {
            i += 1;
        }
    }
    Some(out)
}

/// Each item once, and every item of the scene kept.
fn holds_every_item_once(current: &[OrderInfo], scene: &[(Option<usize>, usize)]) -> bool {
    let mut a: Vec<usize> = current.iter().map(|p| p.item.addr()).collect();
    let mut b: Vec<usize> = scene.iter().map(|&(_, i)| i).collect();
    a.sort_unstable();
    b.sort_unstable();
    a == b
}

proptest! {
    #[test]
    fn anchor_matches_c_for_any_movement(movement in any::<c_int>(), has_prev: bool, has_next: bool) {
        check_anchor(movement, has_prev, has_next);
    }

    #[test]
    fn position_matches_c_for_any_position(position in any::<c_int>(), has_first: bool) {
        check_position(position, has_first);
    }

    #[test]
    fn match_item_matches_c_for_any_order(
        order in prop::collection::vec(0usize..16, 0..16),
        it in 0usize..16,
        index in 0usize..20,
        start: bool,
    ) {
        check_match(&order, it, index, start);
    }

    #[test]
    fn in_group_run_matches_c_for_any_order(
        order in prop::collection::vec((prop::option::of(0usize..6), 0usize..6), 1..20),
        pick in any::<prop::sample::Index>(),
    ) {
        let order: Vec<OrderInfo> = order.iter().map(|&(g, i)| info(g, i)).collect();
        check_run_end(&order, pick.index(order.len()));
    }

    /// Random scenes and orders through the relinking model: the shims and
    /// the C give the same scene for every order the C would accept from a
    /// caller that lists the scene's items; for any other order the shims
    /// refuse it, and wherever the C relinks one it can lose or repeat an
    /// item.
    #[test]
    fn reorder2_relink_matches_c(
        tops in 0usize..6,
        groups in prop::collection::vec(0usize..4, 0..3),
        order_items in prop::collection::vec((prop::option::of(0usize..12), 0usize..12), 0..14),
        shuffle in prop::collection::vec(any::<prop::sample::Index>(), 0..12),
        mode in 0u8..3,
    ) {
        // ids: 0..6 plain items at the top level, 6.. groups, 9.. their items
        let mut current = Vec::new();
        for t in 0..tops {
            current.push(info(None, t));
        }
        let mut next = 9;
        for (g, &size) in groups.iter().enumerate() {
            current.push(info(None, 6 + g));
            for _ in 0..size {
                if next < 16 {
                    current.push(info(Some(6 + g), next));
                    next += 1;
                }
            }
        }
        let group_count = groups.len();
        let is_group = |p: *const c_void| (6..6 + group_count).any(|g| item(g) == p);
        let order: Vec<OrderInfo> = match mode {
            0 => {
                let mut o = current.clone();
                for (k, idx) in shuffle.iter().enumerate() {
                    if !o.is_empty() {
                        let len = o.len();
                        o.swap(k % len, idx.index(len));
                    }
                }
                // any groups on the plain items
                for (p, &(g, _)) in o.iter_mut().zip(order_items.iter()) {
                    if !is_group(p.item) {
                        p.group = g.filter(|&g| (6..6 + group_count).contains(&g)).map_or(std::ptr::null(), item);
                    }
                }
                o
            }
            1 => current.clone(),
            _ => order_items.iter().map(|&(g, i)| info(g.filter(|&g| g < 16), i.min(15))).collect(),
        };
        let shim = relink(&current, &order, is_group, false);
        let c = relink(&current, &order, is_group, true);
        if rearranged(&current, &order) {
            prop_assert_eq!(&shim, &c);
        } else {
            prop_assert_eq!(&shim, &None);
        }
        if let Some(scene) = &shim {
            prop_assert!(holds_every_item_once(&current, scene));
        }
    }
}
