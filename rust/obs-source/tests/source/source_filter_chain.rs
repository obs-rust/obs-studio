//! Tier 1: `obs_source::source_filter_chain`, the filter chain decisions of
//! `filter_compatible()`, `obs_source_filter_add()`,
//! `obs_source_filter_remove_refless()`, `move_filter_dir()` (with
//! `find_next_filter()`, `find_prev_filter()` and
//! `reorder_filter_targets()`) and `obs_source_restore_filters()`.
//!
//! The `steps_*` cases port `test/cmocka/test_source_filter_chain_steps.c`
//! 1:1, under the same case names, in the same order, with the same inputs
//! and expectations, the models of the callers included.
//!
//! The `edge_*` cases are new: the four intentional differences of the
//! port (a filter that has a parent is refused, a failed reference unlocks
//! before returning, a movement outside the enum stays, and an unmatched
//! backed-up filter is loaded private), the walks and edges at the ends of
//! `usize`, and proptests of every decision against models written another
//! way: compatibility over any flag words, the moves over random chains of
//! up to 8 filters of either kind, and the restore loop over random backup
//! and current names with repeats and unnamed filters.

use obs_source::source_filter_chain::{
    ASYNC, AUDIO, MOVE_BOTTOM, MOVE_DOWN, MOVE_TOP, MOVE_UP, RefStep, Restore, Route, Target,
    VIDEO, add_allowed, add_target, checks_compatible, compatible, edge_move, move_route, ref_step,
    remove_retarget, restore_matches, restore_step, same_kind, target_of, walk_step,
};
use proptest::prelude::*;

const ASYNC_VIDEO: u32 = ASYNC | VIDEO;

/// What a filter targets in the models: `None` is the source.
type Tgt = Option<u32>;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Chain {
    ids: Vec<u32>,
    flags: Vec<u32>,
    targets: Vec<Tgt>,
}

impl Chain {
    fn of(ids: &[u32], flags: Option<&[u32]>) -> Self {
        let mut c = Self {
            ids: ids.to_vec(),
            flags: flags.map_or_else(|| vec![VIDEO; ids.len()], <[u32]>::to_vec),
            targets: vec![None; ids.len()],
        };
        c.retarget_all();
        c
    }

    /// `reorder_filter_targets()`.
    fn retarget_all(&mut self) {
        let n = self.ids.len();
        for i in 0..n {
            self.targets[i] = match target_of(i, n) {
                Target::Source => None,
                Target::Filter => Some(self.ids[i + 1]),
            };
        }
    }
}

fn assert_chain(c: &Chain, ids: &[u32]) {
    assert_eq!(c.ids, ids);
    for (i, t) in c.targets.iter().enumerate() {
        assert_eq!(*t, ids.get(i + 1).copied());
    }
}

// ---------------------------------------------------------------------------
// filter_compatible()

#[test]
fn steps_compatibility() {
    // an audio filter on a video source
    assert!(!compatible(VIDEO, AUDIO));
    // an async video filter on a sync video source
    assert!(!compatible(VIDEO, ASYNC_VIDEO));
    assert!(compatible(ASYNC_VIDEO, ASYNC_VIDEO));
    // an audio filter that also sets the async bit: the bit is dropped
    assert!(compatible(AUDIO, AUDIO | ASYNC));
    assert!(compatible(VIDEO | AUDIO, AUDIO | ASYNC));
    // but not for a filter that also has video
    assert!(!compatible(VIDEO | AUDIO, ASYNC_VIDEO | AUDIO));
    assert!(compatible(ASYNC_VIDEO | AUDIO, ASYNC_VIDEO | AUDIO));
    // a video filter needs video, an audio filter audio
    assert!(!compatible(AUDIO, VIDEO));
    assert!(!compatible(0, VIDEO));
    assert!(compatible(VIDEO | AUDIO, VIDEO));
    assert!(compatible(VIDEO | AUDIO, AUDIO));
    // only an async filter needs the async bit
    assert!(!compatible(VIDEO, ASYNC));
    assert!(compatible(ASYNC, ASYNC));
    // other bits are ignored
    assert!(compatible(0, 0xffff_fff8));
    assert!(compatible(VIDEO, 0xffff_fff8 | VIDEO));
    assert!(compatible(0xffff_fff8, 0));
}

// ---------------------------------------------------------------------------
// obs_source_filter_add()

#[test]
fn steps_add_gates() {
    // a filter already in the list is refused
    assert!(!add_allowed(true, true));
    assert!(add_allowed(false, false));

    // a source that owns its info id skips the compatibility check
    assert!(!checks_compatible(true));
    assert!(checks_compatible(false));

    assert_eq!(ref_step(true), RefStep::Add);
    // a failed reference never adds
    assert_ne!(ref_step(false), RefStep::Add);
}

/// A model of `obs_source_filter_add()` from the list check on, for a
/// source whose filters all pass.
fn add(c: &mut Chain, id: u32, flags: u32, owns_info_id: bool, source_flags: u32) -> bool {
    let listed = c.ids.contains(&id);

    if !add_allowed(listed, listed) {
        return false;
    }
    if checks_compatible(owns_info_id) && !compatible(source_flags, flags) {
        return false;
    }
    if ref_step(true) != RefStep::Add {
        return false;
    }

    let target = match add_target(c.ids.len()) {
        Target::Source => None,
        Target::Filter => Some(c.ids[0]),
    };
    c.ids.insert(0, id);
    c.flags.insert(0, flags);
    c.targets.insert(0, target);
    true
}

#[test]
fn steps_add_targets() {
    let mut c = Chain::default();

    assert_eq!(add_target(0), Target::Source);
    assert_eq!(add_target(1), Target::Filter);
    assert_eq!(add_target(2), Target::Filter);

    // onto an empty list: the new filter targets the source
    assert!(add(&mut c, 1, VIDEO, false, VIDEO));
    assert_chain(&c, &[1]);

    // onto a list of two: it goes first and targets the old first
    assert!(add(&mut c, 2, VIDEO, false, VIDEO));
    assert!(add(&mut c, 3, VIDEO, false, VIDEO));
    assert_chain(&c, &[3, 2, 1]);

    // again: refused, nothing changes
    assert!(!add(&mut c, 2, VIDEO, false, VIDEO));
    assert_chain(&c, &[3, 2, 1]);

    // an audio filter on a video source is refused...
    assert!(!add(&mut c, 4, AUDIO, false, VIDEO));
    assert_chain(&c, &[3, 2, 1]);
    // ...unless the source owns its info id
    assert!(add(&mut c, 4, AUDIO, true, VIDEO));
    assert_chain(&c, &[4, 3, 2, 1]);
}

// ---------------------------------------------------------------------------
// obs_source_filter_remove_refless()

fn remove_at(c: &mut Chain, idx: usize) {
    if let Some(prev) = remove_retarget(idx) {
        c.targets[prev] = c.targets[idx];
    }
    c.ids.remove(idx);
    c.flags.remove(idx);
    c.targets.remove(idx);
}

#[test]
fn steps_remove_retargets() {
    // the first filter: nothing to retarget
    assert_eq!(remove_retarget(0), None);
    assert_eq!(remove_retarget(1), Some(0));
    assert_eq!(remove_retarget(5), Some(4));

    let ids = [1, 2, 3, 4];

    let mut c = Chain::of(&ids, None);
    remove_at(&mut c, 0);
    assert_chain(&c, &[2, 3, 4]);

    let mut c = Chain::of(&ids, None);
    remove_at(&mut c, 2);
    assert_chain(&c, &[1, 2, 4]);

    // the last: the one before it takes the source
    let mut c = Chain::of(&ids, None);
    remove_at(&mut c, 3);
    assert_chain(&c, &[1, 2, 3]);

    let mut c = Chain::of(&ids[..1], None);
    remove_at(&mut c, 0);
    assert!(c.ids.is_empty());
}

// ---------------------------------------------------------------------------
// move_filter_dir()

/// `find_next_filter()` and `find_prev_filter()`.
fn walk(c: &Chain, up: bool, idx: usize) -> Option<usize> {
    let mut cur = idx;
    while let Some(next) = walk_step(up, cur, c.ids.len()) {
        if same_kind(c.flags[idx], c.flags[next]) {
            return Some(next);
        }
        cur = next;
    }
    None
}

fn move_item(c: &mut Chain, from: usize, to: usize) {
    let id = c.ids.remove(from);
    let flags = c.flags.remove(from);
    c.ids.insert(to, id);
    c.flags.insert(to, flags);
}

/// A model of `move_filter_dir()` for a listed filter, as the C caller
/// handles each route (a [`Route::Retarget`] retargets and reports a move).
fn move_filter(c: &mut Chain, idx: usize, movement: i32) -> bool {
    let route = move_route(movement);
    match route {
        Route::Up | Route::Down => {
            let Some(to) = walk(c, route == Route::Up, idx) else {
                return false;
            };
            move_item(c, idx, to);
        }
        Route::Top | Route::Bottom => {
            let Some(to) = edge_move(route == Route::Top, idx, c.ids.len()) else {
                return false;
            };
            move_item(c, idx, to);
        }
        Route::Retarget => {}
        Route::Stay => return false,
    }
    c.retarget_all();
    true
}

#[test]
fn steps_move_routes() {
    assert_eq!(move_route(MOVE_UP), Route::Up);
    assert_eq!(move_route(MOVE_DOWN), Route::Down);
    assert_eq!(move_route(MOVE_TOP), Route::Top);
    assert_eq!(move_route(MOVE_BOTTOM), Route::Bottom);

    // a movement outside the enum takes none of the four routes
    for movement in [-1, 4, 5, 1000] {
        let route = move_route(movement);
        assert!(!matches!(
            route,
            Route::Up | Route::Down | Route::Top | Route::Bottom
        ));
    }
}

#[test]
fn steps_walk_steps() {
    assert_eq!(walk_step(true, 0, 3), Some(1));
    assert_eq!(walk_step(true, 1, 3), Some(2));
    assert_eq!(walk_step(true, 2, 3), None);

    assert_eq!(walk_step(false, 2, 3), Some(1));
    assert_eq!(walk_step(false, 0, 3), None);

    // only the async bit decides the kind
    assert!(same_kind(VIDEO, AUDIO));
    assert!(same_kind(ASYNC_VIDEO, ASYNC));
    assert!(same_kind(AUDIO | ASYNC, ASYNC_VIDEO));
    assert!(!same_kind(VIDEO, ASYNC_VIDEO));
    assert!(!same_kind(ASYNC_VIDEO, 0));
    assert!(same_kind(0xffff_fffb, 0));
}

#[test]
fn steps_edge_moves() {
    assert_eq!(edge_move(true, 0, 3), Some(2));
    assert_eq!(edge_move(true, 2, 3), None);
    assert_eq!(edge_move(false, 2, 3), Some(0));
    assert_eq!(edge_move(false, 0, 3), None);
    // the only filter is both
    assert_eq!(edge_move(true, 0, 1), None);
    assert_eq!(edge_move(false, 0, 1), None);
}

#[test]
fn steps_target_chain() {
    assert_eq!(target_of(0, 1), Target::Source);
    assert_eq!(target_of(0, 3), Target::Filter);
    assert_eq!(target_of(1, 3), Target::Filter);
    assert_eq!(target_of(2, 3), Target::Source);
}

#[test]
fn steps_moves_skip_the_other_kind() {
    // sync, async, async, sync, sync
    let ids = [1, 2, 3, 4, 5];
    let flags = [VIDEO, ASYNC_VIDEO, ASYNC_VIDEO, VIDEO, AUDIO];

    // up from 1 passes the two async filters
    let mut c = Chain::of(&ids, Some(&flags));
    assert!(move_filter(&mut c, 0, MOVE_UP));
    assert_chain(&c, &[2, 3, 4, 1, 5]);

    // down from 4 passes them too
    let mut c = Chain::of(&ids, Some(&flags));
    assert!(move_filter(&mut c, 3, MOVE_DOWN));
    assert_chain(&c, &[4, 1, 2, 3, 5]);

    // an async filter moves among the async ones
    let mut c = Chain::of(&ids, Some(&flags));
    assert!(move_filter(&mut c, 1, MOVE_UP));
    assert_chain(&c, &[1, 3, 2, 4, 5]);

    // and no further: no async filter above 3, none below 2
    let mut c = Chain::of(&ids, Some(&flags));
    assert!(!move_filter(&mut c, 2, MOVE_UP));
    assert_chain(&c, &ids);
    assert!(!move_filter(&mut c, 1, MOVE_DOWN));
    assert_chain(&c, &ids);

    // up at the top and down at the bottom are refused
    assert!(!move_filter(&mut c, 4, MOVE_UP));
    assert!(!move_filter(&mut c, 0, MOVE_DOWN));
    assert_chain(&c, &ids);
}

#[test]
fn steps_top_and_bottom_ignore_the_kind() {
    let ids = [1, 2, 3, 4];
    let flags = [VIDEO, ASYNC_VIDEO, VIDEO, AUDIO];

    // characterized, not endorsed: an async filter goes above sync ones
    let mut c = Chain::of(&ids, Some(&flags));
    assert!(move_filter(&mut c, 1, MOVE_TOP));
    assert_chain(&c, &[1, 3, 4, 2]);

    let mut c = Chain::of(&ids, Some(&flags));
    assert!(move_filter(&mut c, 1, MOVE_BOTTOM));
    assert_chain(&c, &[2, 1, 3, 4]);

    let mut c = Chain::of(&ids, Some(&flags));
    assert!(!move_filter(&mut c, 3, MOVE_TOP));
    assert!(!move_filter(&mut c, 0, MOVE_BOTTOM));
    assert_chain(&c, &ids);
}

// ---------------------------------------------------------------------------
// obs_source_restore_filters()

/// A model of the backup loop: for each backed-up name, the index of the
/// current filter it keeps, or `None` for a load. Asserts each step.
fn restore(backup: &[&str], current: &[Option<&str>]) -> Vec<Option<usize>> {
    backup
        .iter()
        .map(|name| {
            let found = current
                .iter()
                .position(|cur| restore_matches(cur.map(str::as_bytes), name.as_bytes()));
            let step = restore_step(found.is_some());
            if found.is_some() {
                assert_eq!(step, Restore::Reuse);
            } else {
                assert_ne!(step, Restore::Reuse);
            }
            found
        })
        .collect()
}

#[test]
fn steps_restore_matches() {
    assert!(restore_matches(Some(b"Color"), b"Color"));
    assert!(!restore_matches(Some(b"Color"), b"color"));
    assert!(!restore_matches(Some(b"Color"), b"Color 2"));
    assert!(restore_matches(Some(b""), b""));
    // a filter with no name never matches, not even ""
    assert!(!restore_matches(None, b""));
    assert!(!restore_matches(None, b"Color"));

    assert_eq!(restore_step(true), Restore::Reuse);
    assert_ne!(restore_step(false), Restore::Reuse);
}

#[test]
fn steps_restore_plans() {
    let current = [Some("Color"), Some("Crop"), Some("Sharpen")];

    // every name matched, in the backup's order
    assert_eq!(
        restore(&["Sharpen", "Color", "Crop"], &current),
        [Some(2), Some(0), Some(1)]
    );

    // none matched: each is loaded
    assert_eq!(restore(&["Mask", "Gain"], &current), [None, None]);

    // characterized, not endorsed: a name backed up twice keeps the same
    // filter twice
    assert_eq!(
        restore(&["Crop", "Mask", "Crop"], &current),
        [Some(1), None, Some(1)]
    );

    // a current name that appears twice: the first is kept
    assert_eq!(
        restore(&["Crop"], &[None, Some("Crop"), Some("Crop")]),
        [Some(1)]
    );

    // nothing current
    assert_eq!(restore(&["Crop"], &[]), [None]);
}

// ---------------------------------------------------------------------------
// The intentional differences

/// A filter that has a parent is on another source (or this one): refused,
/// with the same warning as a listed filter. The C took it.
#[test]
fn edge_add_refuses_a_filter_with_a_parent() {
    assert!(!add_allowed(false, true));
    assert!(!add_allowed(true, false));
    assert!(!add_allowed(true, true));
    assert!(add_allowed(false, false));
}

/// A filter that can't be referenced: unlock, then return. The C returned
/// with `filter_mutex` still locked.
#[test]
fn edge_failed_reference_unlocks() {
    assert_eq!(ref_step(false), RefStep::UnlockReturn);
    assert_eq!(ref_step(true), RefStep::Add);
}

/// A movement outside the enum moves nothing, retargets nothing and
/// reports no move, so nothing is signalled. The C retargeted every filter
/// and reported a move.
#[test]
fn edge_unknown_movement_stays() {
    for movement in [i32::MIN, -2, -1, 4, 5, 1000, i32::MAX] {
        assert_eq!(move_route(movement), Route::Stay, "movement {movement}");
    }

    let ids = [1, 2, 3];
    let mut c = Chain::of(&ids, None);
    // a chain whose targets are stale stays stale: nothing is retargeted
    c.targets[0] = None;
    let before = c.clone();
    assert!(!move_filter(&mut c, 1, -1));
    assert_eq!(c, before);
}

/// A backed-up filter with no current one is loaded private. The C loaded
/// it public, so a filter whose plugin is missing came back as a public
/// source.
#[test]
fn edge_unmatched_restore_loads_private() {
    assert_eq!(restore_step(false), Restore::LoadPrivate);
    assert_eq!(restore_step(true), Restore::Reuse);
}

/// The C's `size_t` arithmetic at the ends: the last index of an empty
/// list is `usize::MAX`, and a step up from `usize::MAX` wraps to 0.
#[test]
fn edge_walks_and_edges_at_the_ends_of_usize() {
    assert_eq!(walk_step(true, usize::MAX, 0), None);
    assert_eq!(walk_step(true, 0, 0), Some(1));
    assert_eq!(walk_step(true, usize::MAX, 3), Some(0));
    assert_eq!(walk_step(false, usize::MAX, 3), Some(usize::MAX - 1));
    assert_eq!(walk_step(false, 0, 0), None);

    assert_eq!(edge_move(true, usize::MAX, 0), None);
    assert_eq!(edge_move(true, 0, 0), Some(usize::MAX));
    assert_eq!(edge_move(false, usize::MAX, 0), Some(0));

    assert_eq!(target_of(usize::MAX, 0), Target::Source);
    assert_eq!(target_of(0, 0), Target::Filter);
    assert_eq!(add_target(usize::MAX), Target::Filter);
    assert_eq!(remove_retarget(usize::MAX), Some(usize::MAX - 1));
}

// ---------------------------------------------------------------------------
// Proptests against models written another way

/// The filter's needs as a set of bits: video, audio and async, with async
/// dropped for an audio-only filter.
fn needs(flags: u32) -> u32 {
    let has_video = flags & VIDEO != 0;
    let has_audio = flags & AUDIO != 0;
    let mut out = flags & (VIDEO | AUDIO | ASYNC);
    if has_audio && !has_video {
        out &= !ASYNC;
    }
    out
}

/// A brute-force walk: the nearest index in the direction whose async bit
/// matches, by scanning the whole list.
fn nearest_same_kind(flags: &[u32], idx: usize, up: bool) -> Option<usize> {
    let kind = flags[idx] & ASYNC != 0;
    let matches = |j: &usize| (flags[*j] & ASYNC != 0) == kind;
    if up {
        (idx + 1..flags.len()).find(matches)
    } else {
        (0..idx).rev().find(matches)
    }
}

fn flag_word() -> impl Strategy<Value = u32> {
    prop_oneof![
        Just(0),
        Just(VIDEO),
        Just(AUDIO),
        Just(ASYNC_VIDEO),
        Just(ASYNC_VIDEO | AUDIO),
        Just(AUDIO | ASYNC),
        Just(VIDEO | AUDIO),
        any::<u32>(),
    ]
}

proptest! {
    #[test]
    fn edge_compatible_matches_model(source in flag_word(), filter in flag_word()) {
        let need = needs(filter);
        prop_assert_eq!(compatible(source, filter), source & need == need);
        prop_assert_eq!(same_kind(source, filter), (source & ASYNC) == (filter & ASYNC));
    }

    #[test]
    fn edge_add_gates_match_model(listed in any::<bool>(), has_parent in any::<bool>(), owns in any::<bool>(), referenced in any::<bool>(), count in any::<usize>()) {
        prop_assert_eq!(add_allowed(listed, has_parent), !(listed || has_parent));
        prop_assert_eq!(checks_compatible(owns), !owns);
        prop_assert_eq!(ref_step(referenced), if referenced { RefStep::Add } else { RefStep::UnlockReturn });
        prop_assert_eq!(add_target(count), if count == 0 { Target::Source } else { Target::Filter });
        prop_assert_eq!(remove_retarget(count), count.checked_sub(1));
    }

    #[test]
    fn edge_moves_match_model(
        flags in prop::collection::vec(flag_word(), 1..=8),
        pick in any::<prop::sample::Index>(),
        movement in -2i32..=6,
    ) {
        let n = flags.len();
        let idx = pick.index(n);
        let ids: Vec<u32> = (1..).take(n).collect();
        let mut c = Chain::of(&ids, Some(&flags));
        let moved = move_filter(&mut c, idx, movement);

        let to = match movement {
            MOVE_UP => nearest_same_kind(&flags, idx, true),
            MOVE_DOWN => nearest_same_kind(&flags, idx, false),
            MOVE_TOP => (idx != n - 1).then_some(n - 1),
            MOVE_BOTTOM => (idx != 0).then_some(0),
            _ => None,
        };
        prop_assert_eq!(moved, to.is_some());

        let mut want = ids.clone();
        if let Some(to) = to {
            let id = want.remove(idx);
            want.insert(to, id);
        }
        prop_assert_eq!(&c.ids, &want);
        for (i, t) in c.targets.iter().enumerate() {
            prop_assert_eq!(*t, want.get(i + 1).copied());
        }
    }

    #[test]
    fn edge_restore_matches_model(
        backup in prop::collection::vec(0u8..6, 0..8),
        current in prop::collection::vec(prop::option::of(0u8..6), 0..8),
    ) {
        let name = |n: u8| ["", "A", "B", "C", "A 2", "a"][usize::from(n)];
        let backup: Vec<&str> = backup.into_iter().map(name).collect();
        let current: Vec<Option<&str>> = current.into_iter().map(|n| n.map(name)).collect();
        let plan = restore(&backup, &current);
        for (b, p) in backup.iter().zip(&plan) {
            let want = current.iter().position(|c| *c == Some(*b));
            prop_assert_eq!(*p, want);
            prop_assert_eq!(restore_step(p.is_some()), if p.is_some() { Restore::Reuse } else { Restore::LoadPrivate });
        }
    }
}
