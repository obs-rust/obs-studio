//! Tier 1: `obs_scene::item_order`, the scene item order, reorder and group
//! membership decisions of `obs-scene.c.h`.
//!
//! The `steps_*` cases port `test/cmocka/test_scene_item_order_steps.c` 1:1:
//! its 16 cases under the same names, in the same order, with the same
//! inputs and expectations.
//!
//! The `edge_*` cases are new: the four intentional differences (#2895), and
//! proptests of every decision against models written another way, over any
//! `i32` movement and position and over random scenes and orders.

use obs_scene::item_order::{
    Anchor, Entry, OBS_ORDER_MOVE_BOTTOM, OBS_ORDER_MOVE_DOWN, OBS_ORDER_MOVE_TOP,
    OBS_ORDER_MOVE_UP, Position, ReorderCheck, add_item_allowed, anchor, in_group_run, item_active,
    item_attached, match_item, moves_scene, position, remove_item_allowed, reorder_check,
};
use proptest::prelude::*;

// Item ids, as the C's item addresses.
const A: u32 = 1;
const B: u32 = 2;
const C: u32 = 3;
const D: u32 = 4;
const G: u32 = 5;
const H: u32 = 6;

fn top(item: u32) -> Entry<u32> {
    Entry { group: None, item }
}

fn sub(group: u32, item: u32) -> Entry<u32> {
    Entry {
        group: Some(group),
        item,
    }
}

// ---------------------------------------------------------------------------
// obs_sceneitem_set_order()

fn check_anchors(cases: &[(i32, bool, bool, Anchor)]) {
    for &(movement, has_prev, has_next, expected) in cases {
        assert_eq!(
            anchor(movement, has_prev, has_next),
            expected,
            "anchor({movement}, {has_prev}, {has_next})"
        );
    }
}

#[test]
fn steps_anchor_down() {
    check_anchors(&[
        (OBS_ORDER_MOVE_DOWN, false, false, Anchor::Bottom),
        (OBS_ORDER_MOVE_DOWN, false, true, Anchor::Bottom),
        (OBS_ORDER_MOVE_DOWN, true, false, Anchor::PrevPrev),
        (OBS_ORDER_MOVE_DOWN, true, true, Anchor::PrevPrev),
    ]);
}

#[test]
fn steps_anchor_up() {
    check_anchors(&[
        (OBS_ORDER_MOVE_UP, false, false, Anchor::Prev),
        (OBS_ORDER_MOVE_UP, false, true, Anchor::Next),
        (OBS_ORDER_MOVE_UP, true, false, Anchor::Prev),
        (OBS_ORDER_MOVE_UP, true, true, Anchor::Next),
    ]);
}

#[test]
fn steps_anchor_top_bottom() {
    check_anchors(&[
        (OBS_ORDER_MOVE_TOP, false, false, Anchor::Last),
        (OBS_ORDER_MOVE_TOP, false, true, Anchor::Last),
        (OBS_ORDER_MOVE_TOP, true, false, Anchor::Last),
        (OBS_ORDER_MOVE_TOP, true, true, Anchor::Last),
        (OBS_ORDER_MOVE_BOTTOM, false, false, Anchor::Bottom),
        (OBS_ORDER_MOVE_BOTTOM, false, true, Anchor::Bottom),
        (OBS_ORDER_MOVE_BOTTOM, true, false, Anchor::Bottom),
        (OBS_ORDER_MOVE_BOTTOM, true, true, Anchor::Bottom),
    ]);
}

/// A model of `obs_sceneitem_set_order()` on a list of item numbers from the
/// bottom: the caller's detach and attach around the anchor.
fn model_set_order(list: &mut Vec<u32>, index: usize, movement: i32) {
    let has_prev = index > 0;
    let has_next = index + 1 < list.len();
    let prev = has_prev.then(|| list[index - 1]);
    let prev_prev = (index > 1).then(|| list[index - 2]);
    let next = has_next.then(|| list[index + 1]);
    let found = anchor(movement, has_prev, has_next);
    if found == Anchor::Stay {
        return;
    }

    let item = list.remove(index);
    let after = match found {
        Anchor::Bottom => None,
        Anchor::PrevPrev => prev_prev,
        Anchor::Next => next,
        Anchor::Prev => prev,
        Anchor::Last => list.last().copied(),
        Anchor::Stay => unreachable!(),
    };
    let at = after.map_or(0, |after| {
        list.iter().position(|&i| i == after).unwrap() + 1
    });
    list.insert(at, item);
}

#[test]
fn steps_set_order_moves() {
    let cases: [(usize, i32, [u32; 4]); 13] = [
        (0, OBS_ORDER_MOVE_UP, [2, 1, 3, 4]),
        (1, OBS_ORDER_MOVE_UP, [1, 3, 2, 4]),
        (3, OBS_ORDER_MOVE_UP, [1, 2, 3, 4]),
        (0, OBS_ORDER_MOVE_DOWN, [1, 2, 3, 4]),
        (1, OBS_ORDER_MOVE_DOWN, [2, 1, 3, 4]),
        (2, OBS_ORDER_MOVE_DOWN, [1, 3, 2, 4]),
        (3, OBS_ORDER_MOVE_DOWN, [1, 2, 4, 3]),
        (0, OBS_ORDER_MOVE_TOP, [2, 3, 4, 1]),
        (1, OBS_ORDER_MOVE_TOP, [1, 3, 4, 2]),
        (3, OBS_ORDER_MOVE_TOP, [1, 2, 3, 4]),
        (0, OBS_ORDER_MOVE_BOTTOM, [1, 2, 3, 4]),
        (2, OBS_ORDER_MOVE_BOTTOM, [3, 1, 2, 4]),
        (3, OBS_ORDER_MOVE_BOTTOM, [4, 1, 2, 3]),
    ];
    for (index, movement, after) in cases {
        let mut list = vec![1, 2, 3, 4];
        model_set_order(&mut list, index, movement);
        assert_eq!(list, after, "item {index} movement {movement}");
    }
}

#[test]
fn steps_set_order_single() {
    for movement in [
        OBS_ORDER_MOVE_UP,
        OBS_ORDER_MOVE_DOWN,
        OBS_ORDER_MOVE_TOP,
        OBS_ORDER_MOVE_BOTTOM,
    ] {
        let mut list = vec![7];
        model_set_order(&mut list, 0, movement);
        assert_eq!(list, [7]);
    }
}

// ---------------------------------------------------------------------------
// obs_sceneitem_set_order_position()

#[test]
fn steps_position_bottom() {
    assert_eq!(position(0, true), Position::Bottom);
    for p in [0, 1, 2, 5, -1, i32::MIN, i32::MAX] {
        assert_eq!(position(p, false), Position::Bottom, "{p}");
    }
}

#[test]
fn steps_position_steps() {
    let max_steps = usize::try_from(i32::MAX).unwrap() - 1;
    for (p, steps) in [
        (1, 0),
        (2, 1),
        (3, 2),
        (6, 5),
        (11, 10),
        (i32::MAX, max_steps),
        // characterized, not endorsed: a negative position takes no step
        (-1, 0),
        (-2, 0),
        (i32::MIN, 0),
    ] {
        assert_eq!(position(p, true), Position::After { steps }, "{p}");
    }
}

/// The caller on a list: where the item lands for each position, the walk
/// stopping at the last item.
fn model_set_position(list: &mut Vec<u32>, item: u32, p: i32) {
    let at = match position(p, !list.is_empty()) {
        Position::Bottom => 0,
        Position::After { steps } => steps.min(list.len() - 1) + 1,
    };
    list.insert(at, item);
}

#[test]
fn steps_position_walk() {
    for (p, after) in [
        (-1, [1, 3, 2, 4, 5]),
        (0, [3, 1, 2, 4, 5]),
        (1, [1, 3, 2, 4, 5]),
        (2, [1, 2, 3, 4, 5]),
        (3, [1, 2, 4, 3, 5]),
        (4, [1, 2, 4, 5, 3]),
        (5, [1, 2, 4, 5, 3]),
        (9, [1, 2, 4, 5, 3]),
    ] {
        let mut list = vec![1, 2, 4, 5];
        model_set_position(&mut list, 3, p);
        assert_eq!(list, after, "{p}");
    }
}

// ---------------------------------------------------------------------------
// obs_sceneitem_get_order_position() and obs_sceneitem_group_ungroup2()

#[test]
fn steps_item_attached() {
    assert!(item_attached(true, true));
}

// ---------------------------------------------------------------------------
// obs_scene_reorder_items()

#[test]
fn steps_match_item_steps() {
    let order = [A, B, C];
    let mut matches = true;

    assert!(match_item(&order, &A, 0, &mut matches));
    assert!(matches);
    assert!(match_item(&order, &C, 2, &mut matches));
    assert!(matches);

    // found elsewhere: the order differs
    assert!(match_item(&order, &B, 0, &mut matches));
    assert!(!matches);

    // once cleared, it stays cleared
    assert!(match_item(&order, &A, 0, &mut matches));
    assert!(!matches);

    // not in the order: not found, the flag untouched
    matches = true;
    assert!(!match_item(&order, &D, 0, &mut matches));
    assert!(matches);
    assert!(!match_item(&order[..0], &A, 0, &mut matches));
    assert!(matches);
    assert!(!match_item::<u32>(&[], &A, 0, &mut matches));
    assert!(matches);

    // a repeated item counts at its first entry
    let repeated = [A, A, B];
    assert!(match_item(&repeated, &A, 1, &mut matches));
    assert!(!matches);
    matches = true;
    assert!(match_item(&repeated, &A, 0, &mut matches));
    assert!(matches);
}

/// `sceneitems_match()` and the refusal of `obs_scene_reorder_items()`:
/// true when the order is applied.
fn model_reorder(scene: &[u32], order: &[u32]) -> bool {
    let mut matches = true;
    for (index, item) in scene.iter().enumerate() {
        if !match_item(order, item, index, &mut matches) {
            return false;
        }
    }
    scene.len() == order.len() && !matches
}

#[test]
fn steps_reorder_flat() {
    let scene = [A, B, C];
    assert!(!model_reorder(&scene, &[A, B, C]));
    assert!(model_reorder(&scene, &[C, A, B]));
    assert!(!model_reorder(&scene, &[B, A]));
    assert!(!model_reorder(&scene, &[B, A, C, D]));
    assert!(!model_reorder(&scene, &[B, A, A]));
    // a repeat that still holds every item is longer than the scene
    assert!(!model_reorder(&scene, &[B, A, C, A]));
}

// ---------------------------------------------------------------------------
// obs_scene_reorder_items2()

/// A, then group G holding B and C, then D.
fn current() -> [Entry<u32>; 5] {
    [top(A), top(G), sub(G, B), sub(G, C), top(D)]
}

#[test]
fn steps_reorder2_check() {
    let current = current();
    assert_eq!(reorder_check(&current, &current), ReorderCheck::Same);
    assert_eq!(reorder_check::<u32>(&[], &[]), ReorderCheck::Same);

    let top_level = [top(D), top(G), sub(G, B), sub(G, C), top(A)];
    assert_eq!(reorder_check(&current, &top_level), ReorderCheck::Apply);

    let inside = [top(A), top(G), sub(G, C), sub(G, B), top(D)];
    assert_eq!(reorder_check(&current, &inside), ReorderCheck::Apply);

    let moved = [top(A), top(G), top(B), sub(G, C), sub(G, D)];
    assert_eq!(reorder_check(&current, &moved), ReorderCheck::Apply);
}

/// The caller's run: entries after `head`, one step at a time, up to the
/// first that is not in the run.
fn run_end(order: &[Entry<u32>], head: usize) -> usize {
    let mut i = head + 1;
    while in_group_run(order, head, i) {
        i += 1;
    }
    i
}

#[test]
fn steps_group_runs() {
    let order = [
        top(A),
        top(G),
        sub(G, B),
        sub(G, C),
        top(D),
        top(H),
        sub(G, A),
        sub(H, B),
    ];
    assert_eq!(run_end(&order, 1), 4);
    // characterized, not endorsed: a sub-item that does not follow its
    // group ends the run and goes to the top level
    assert_eq!(run_end(&order, 5), 6);
    assert_eq!(run_end(&order[..4], 1), 4);

    let empty = [top(G), top(A), top(H)];
    assert_eq!(run_end(&empty, 0), 1);
    assert_eq!(run_end(&empty, 2), 3);

    // each step on its own: past the order, the head itself, another
    // group's sub-item, a top-level entry
    assert!(!in_group_run(&order, 1, 8));
    assert!(!in_group_run(&order[..4], 1, 4));
    assert!(!in_group_run(&order, 1, 1));
    assert!(in_group_run(&order, 1, 6));
    assert!(!in_group_run(&order, 1, 7));
    assert!(!in_group_run(&order, 1, 0));
}

// ---------------------------------------------------------------------------
// obs_sceneitem_group_add_item() and obs_sceneitem_group_remove_item()

#[test]
fn steps_add_item_gate() {
    // scenes as ids: 0 is NULL
    let (scene, group_scene, other) = (1u32, 2u32, 3u32);
    assert!(add_item_allowed(&scene, &scene, &group_scene));
    assert!(!add_item_allowed(&group_scene, &scene, &group_scene));
    assert!(!add_item_allowed(&other, &scene, &group_scene));
    assert!(!add_item_allowed(&0, &scene, &group_scene));
    // characterized, not endorsed: never decides in libobs
    assert!(!add_item_allowed(&scene, &scene, &scene));
}

#[test]
fn steps_remove_item_gate() {
    assert!(remove_item_allowed(true));
}

// ---------------------------------------------------------------------------
// move_active_child()

#[test]
fn steps_active_child_gates() {
    assert!(moves_scene(&1u32, &2));
    assert!(moves_scene(&1u32, &0));
    assert!(moves_scene(&0u32, &2));
    assert!(!moves_scene(&1u32, &1));
    assert!(!moves_scene(&0u32, &0));

    assert!(item_active(1));
    assert!(item_active(2_147_483_647));
    // a count gone negative counts as inactive, as 0 does
    assert!(!item_active(0));
    assert!(!item_active(-1));
    assert!(!item_active(-2_147_483_648));
}

// ---------------------------------------------------------------------------
// The intentional differences (#2895)

/// Difference 1: a movement outside the enum leaves the item where it is.
#[test]
fn edge_unknown_movement_stays() {
    for movement in [-1, 4, 5, 100, i32::MIN, i32::MAX] {
        for has_prev in [false, true] {
            for has_next in [false, true] {
                assert_eq!(anchor(movement, has_prev, has_next), Anchor::Stay);
            }
        }
        let mut list = vec![1, 2, 3, 4];
        model_set_order(&mut list, 1, movement);
        assert_eq!(list, [1, 2, 3, 4]);
    }
}

/// Difference 2: a NULL or detached item stops the caller.
#[test]
fn edge_null_or_detached_item() {
    assert!(!item_attached(false, false));
    assert!(!item_attached(true, false));
    // has_parent is not read without an item; any value stops
    assert!(!item_attached(false, true));
}

/// Difference 3: an order that is not the scene's items rearranged.
#[test]
fn edge_reorder2_refuses_other_items() {
    let current = current();
    // an item left out
    let missing = [top(A), top(G), sub(G, B), top(D)];
    assert_eq!(reorder_check(&current, &missing), ReorderCheck::Invalid);
    // an item from another scene
    let foreign = [top(A), top(G), sub(G, B), sub(G, C), top(D), top(H)];
    assert_eq!(reorder_check(&current, &foreign), ReorderCheck::Invalid);
    // one listed twice, in place of another
    let twice = [top(A), top(G), sub(G, B), sub(G, B), top(D)];
    assert_eq!(reorder_check(&current, &twice), ReorderCheck::Invalid);
    // one listed twice, on top of every item
    let extra = [top(A), top(G), sub(G, B), sub(G, C), top(D), top(A)];
    assert_eq!(reorder_check(&current, &extra), ReorderCheck::Invalid);
    // an item swapped for one from elsewhere
    let swapped = [top(A), top(G), sub(G, B), sub(G, H), top(D)];
    assert_eq!(reorder_check(&current, &swapped), ReorderCheck::Invalid);
    // an empty scene with an order
    assert_eq!(reorder_check(&[], &[top(A)]), ReorderCheck::Invalid);
    // a group's own entry left out, its items kept
    let headless = [top(A), sub(G, B), sub(G, C), top(D)];
    assert_eq!(reorder_check(&current, &headless), ReorderCheck::Invalid);
}

/// Difference 4: only an item of the group is removed from it.
#[test]
fn edge_remove_item_not_in_group() {
    assert!(!remove_item_allowed(false));
}

/// A head or an index past the end is never in a run, and nothing is read.
#[test]
fn edge_in_group_run_past_the_end() {
    assert!(!in_group_run::<u32>(&[], 0, 1));
    assert!(!in_group_run(&[top(G)], 3, 4));
    assert!(!in_group_run(&[top(G), sub(G, A)], 5, 1));
    assert!(!in_group_run::<u32>(&[], usize::MAX, usize::MAX));
}

// ---------------------------------------------------------------------------
// Proptests against models written another way

fn known(movement: i32) -> bool {
    (0..=3).contains(&movement)
}

proptest! {
    #[test]
    fn edge_anchor_any_movement(movement in any::<i32>(), has_prev: bool, has_next: bool) {
        let expected = match movement {
            OBS_ORDER_MOVE_DOWN if has_prev => Anchor::PrevPrev,
            OBS_ORDER_MOVE_DOWN | OBS_ORDER_MOVE_BOTTOM => Anchor::Bottom,
            OBS_ORDER_MOVE_UP if has_next => Anchor::Next,
            OBS_ORDER_MOVE_UP => Anchor::Prev,
            OBS_ORDER_MOVE_TOP => Anchor::Last,
            _ => Anchor::Stay,
        };
        prop_assert_eq!(anchor(movement, has_prev, has_next), expected);
        prop_assert_eq!(anchor(movement, has_prev, has_next) == Anchor::Stay, !known(movement));
    }

    #[test]
    fn edge_position_any(p in any::<i32>(), has_first: bool) {
        let got = position(p, has_first);
        if !has_first || p == 0 {
            prop_assert_eq!(got, Position::Bottom);
        } else {
            let steps = if p > 1 { usize::try_from(i64::from(p) - 1).unwrap() } else { 0 };
            prop_assert_eq!(got, Position::After { steps });
        }
    }

    /// Every list, item and movement: the model keeps the items, each once,
    /// and a known movement moves the item at most to the far end.
    #[test]
    fn edge_set_order_keeps_every_item(len in 1usize..8, pick in any::<usize>(), movement in -2i32..6) {
        let mut list: Vec<u32> = (1..=u32::try_from(len).unwrap()).collect();
        let index = pick % len;
        let item = list[index];
        model_set_order(&mut list, index, movement);
        let mut sorted = list.clone();
        sorted.sort_unstable();
        prop_assert_eq!(sorted, (1..=u32::try_from(len).unwrap()).collect::<Vec<_>>());
        let at = list.iter().position(|&i| i == item).unwrap();
        match movement {
            OBS_ORDER_MOVE_UP => prop_assert_eq!(at, (index + 1).min(len - 1)),
            OBS_ORDER_MOVE_DOWN => prop_assert_eq!(at, index.saturating_sub(1)),
            OBS_ORDER_MOVE_TOP => prop_assert_eq!(at, len - 1),
            OBS_ORDER_MOVE_BOTTOM => prop_assert_eq!(at, 0),
            _ => prop_assert_eq!(at, index),
        }
    }

    #[test]
    fn edge_match_item_any(order in prop::collection::vec(0u8..8, 0..10), item in 0u8..8, index in 0usize..12, start: bool) {
        let mut matches = start;
        let found = match_item(&order, &item, index, &mut matches);
        let first = order.iter().position(|&i| i == item);
        prop_assert_eq!(found, first.is_some());
        prop_assert_eq!(matches, start && first.is_none_or(|first| first == index));
    }

    /// The flat refusal is the same rule as [`reorder_check`] on top-level
    /// pairs: applied exactly when the order is a different permutation.
    #[test]
    fn edge_reorder_flat_is_a_permutation_check(len in 0usize..6, order in prop::collection::vec(0u32..8, 0..8)) {
        let scene: Vec<u32> = (1..=u32::try_from(len).unwrap()).collect();
        let mut a = scene.clone();
        let mut b = order.clone();
        a.sort_unstable();
        b.sort_unstable();
        let permutation = a == b;
        prop_assert_eq!(model_reorder(&scene, &order), permutation && scene != order);
        let pairs = |v: &[u32]| v.iter().map(|&i| top(i)).collect::<Vec<_>>();
        let check = reorder_check(&pairs(&scene), &pairs(&order));
        prop_assert_eq!(check == ReorderCheck::Apply, model_reorder(&scene, &order));
    }

    /// The pair check against a model that counts each item.
    #[test]
    fn edge_reorder_check_any(
        current in prop::collection::vec((prop::option::of(0u32..4), 0u32..10), 0..8)
            .prop_map(|v| {
                // the scene holds each item once
                let mut seen = std::collections::BTreeSet::new();
                v.into_iter().filter(|&(_, i)| seen.insert(i)).map(|(group, item)| Entry { group, item }).collect::<Vec<_>>()
            }),
        order in prop::collection::vec((prop::option::of(0u32..4), 0u32..10), 0..9),
        shuffle in prop::collection::vec(any::<prop::sample::Index>(), 0..9),
        mode in 0u8..3,
    ) {
        let order: Vec<Entry<u32>> = match mode {
            // the current items, rearranged, with any groups
            0 => {
                let mut o = current.clone();
                for (k, idx) in shuffle.iter().enumerate() {
                    if !o.is_empty() {
                        let j = idx.index(o.len());
                        let i = k % o.len();
                        o.swap(i, j);
                    }
                }
                for (e, (group, _)) in o.iter_mut().zip(order.iter()) {
                    e.group = *group;
                }
                o
            }
            1 => current.clone(),
            _ => order.into_iter().map(|(group, item)| Entry { group, item }).collect(),
        };
        let mut counts = std::collections::BTreeMap::<u32, i32>::new();
        for e in &current {
            *counts.entry(e.item).or_default() += 1;
        }
        for e in &order {
            *counts.entry(e.item).or_default() -= 1;
        }
        let expected = if order == current {
            ReorderCheck::Same
        } else if counts.values().all(|&c| c == 0) {
            ReorderCheck::Apply
        } else {
            ReorderCheck::Invalid
        };
        prop_assert_eq!(reorder_check(&current, &order), expected);
    }

    #[test]
    fn edge_in_group_run_any(
        order in prop::collection::vec((prop::option::of(0u32..4), 0u32..4), 1..10),
        pick in any::<prop::sample::Index>(),
    ) {
        let order: Vec<Entry<u32>> = order.into_iter().map(|(group, item)| Entry { group, item }).collect();
        let head = pick.index(order.len());
        for index in 0..order.len() + 2 {
            prop_assert_eq!(
                in_group_run(&order, head, index),
                order.get(index).is_some_and(|e| e.group == Some(order[head].item))
            );
        }
        let end = run_end(&order, head);
        prop_assert!(end > head && end <= order.len());
        prop_assert!(order[head + 1..end].iter().all(|e| e.group == Some(order[head].item)));
        if end < order.len() {
            prop_assert_ne!(order[end].group, Some(order[head].item));
        }
    }

    #[test]
    fn edge_gates_any(
        parent in 0u8..4, scene in 0u8..4, group_scene in 0u8..4,
        in_group: bool, has_item: bool, has_parent: bool, active_refs: std::ffi::c_long,
    ) {
        prop_assert_eq!(
            add_item_allowed(&parent, &scene, &group_scene),
            parent == scene && parent != group_scene
        );
        prop_assert_eq!(moves_scene(&parent, &scene), parent != scene);
        prop_assert_eq!(item_active(active_refs), active_refs > 0);
        prop_assert_eq!(remove_item_allowed(in_group), in_group);
        prop_assert_eq!(item_attached(has_item, has_parent), has_item && has_parent);
    }
}
