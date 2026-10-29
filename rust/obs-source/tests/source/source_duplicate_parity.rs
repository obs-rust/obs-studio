//! Tier 3 parity: the shims of `obs_source::ffi::source_duplicate` (and the
//! safe core under them) against the original
//! `libobs/obs-source-duplicate.c.h`, compiled as
//! `oracle_obs_source_duplicate_*` and `oracle_obs_scene_duplicate_*`
//! (obs-c-oracle, oracle/source_duplicate.c).
//!
//! Every call runs the shim and the C and compares the results; the safe
//! core is checked against the shim where the types differ.
//!
//! Runs: the header's values; exhaustive over every flag, every type and
//! duplicate type from -1000 to 1000 and the ends of `int`, every single
//! output flag bit and its complement, every filter number from -100,000
//! to 100,000 and near the ends, every reuse step over NULL and three
//! addresses; a model of `obs_scene_duplicate()`'s item loop over every
//! scene of up to 6 items over 3 sources with every item copied, refused
//! by its duplicate or refused by `obs_scene_add()`, with the shims on one
//! side and the C on the other; and proptests over the full ranges and
//! scenes of up to 12 items.
//!
//! Intentional differences (#2905), and the only inputs where the shim and
//! the C differ:
//! 1. `obs_source_duplicate_created_step()` when the create returned no
//!    source: the C gives `OBS_SOURCE_DUPLICATE_COPY_STATE` (its caller
//!    writes through NULL), the shim `OBS_SOURCE_DUPLICATE_NONE` (its
//!    caller releases the settings and returns NULL);
//! 2. `obs_scene_duplicate_reuse()` for an earlier item with the same
//!    source: the C gives `OBS_SCENE_DUPLICATE_REUSE_AT_INDEX` (the new
//!    scene's item at the old index, another item's once an earlier item
//!    was not added), the shim `OBS_SCENE_DUPLICATE_REUSE_COPY` (that
//!    item's own copy) when its copy was added and
//!    `OBS_SCENE_DUPLICATE_REUSE_NONE` (go on) when it was not.
//!
//! `obs_source_duplicate_filter_number(INT_MAX)` is not compared: the C
//! overflows `int` there (undefined), and the shim stays at `INT_MAX`.
//!
//! `test_source_duplicate_steps` runs the C test against the C,
//! `test_source_duplicate_steps_rust` against these shims; the real-libobs
//! scene duplicate tests run the callers through the hybrid libobs, where
//! these shims replace the C.

use std::ffi::{c_int, c_void};

use obs_c_oracle::source_duplicate::{
    oracle_obs_scene_duplicate_item_route, oracle_obs_scene_duplicate_keeps_name,
    oracle_obs_scene_duplicate_keeps_unique, oracle_obs_scene_duplicate_private,
    oracle_obs_scene_duplicate_resizes, oracle_obs_scene_duplicate_reuse,
    oracle_obs_scene_duplicate_unique, oracle_obs_source_duplicate_copies_filters,
    oracle_obs_source_duplicate_created_step, oracle_obs_source_duplicate_filter_number,
    oracle_obs_source_duplicate_flags_step, oracle_obs_source_duplicate_group_step,
    oracle_obs_source_duplicate_scene_step, oracle_obs_source_duplicate_scene_type,
    oracle_obs_source_duplicate_type_step, oracle_source_duplicate_layout,
};
use obs_source::ffi::source_duplicate::{
    OBS_SCENE_DUPLICATE_ITEM_CHILD, OBS_SCENE_DUPLICATE_ITEM_GROUP, OBS_SCENE_DUPLICATE_ITEM_REF,
    OBS_SCENE_DUPLICATE_REUSE_AT_INDEX, OBS_SCENE_DUPLICATE_REUSE_COPY,
    OBS_SCENE_DUPLICATE_REUSE_NONE, OBS_SOURCE_DUPLICATE_ASK_FLAGS, OBS_SOURCE_DUPLICATE_ASK_GROUP,
    OBS_SOURCE_DUPLICATE_ASK_SCENE, OBS_SOURCE_DUPLICATE_COPY_STATE, OBS_SOURCE_DUPLICATE_CREATE,
    OBS_SOURCE_DUPLICATE_NONE, OBS_SOURCE_DUPLICATE_REF, OBS_SOURCE_DUPLICATE_SCENE_COPY,
    obs_scene_duplicate_item_route, obs_scene_duplicate_keeps_name,
    obs_scene_duplicate_keeps_unique, obs_scene_duplicate_private, obs_scene_duplicate_resizes,
    obs_scene_duplicate_reuse, obs_scene_duplicate_unique, obs_source_duplicate_copies_filters,
    obs_source_duplicate_created_step, obs_source_duplicate_filter_number,
    obs_source_duplicate_flags_step, obs_source_duplicate_group_step,
    obs_source_duplicate_scene_step, obs_source_duplicate_scene_type,
    obs_source_duplicate_type_step,
};
use obs_source::source_duplicate::{
    self as core, DO_NOT_DUPLICATE, DUP_COPY, DUP_PRIVATE_COPY, DUP_PRIVATE_REFS, DUP_REFS,
    TYPE_FILTER, TYPE_SCENE,
};
use proptest::prelude::*;

const BOOLS: [bool; 2] = [false, true];

fn layout(which: usize) -> usize {
    // SAFETY: the oracle reads nothing and returns a constant.
    unsafe { oracle_source_duplicate_layout(which) }
}

/// The `int` values compared for each type and duplicate type argument.
fn ints() -> impl Iterator<Item = c_int> {
    (-1000..=1000).chain([
        c_int::MIN,
        c_int::MIN + 1,
        -65_536,
        65_536,
        c_int::MAX - 1,
        c_int::MAX,
    ])
}

#[test]
fn header_values_match() {
    let to = |v: c_int| usize::try_from(v).expect("a header value is non-negative");
    let want = [
        to(TYPE_FILTER),
        to(TYPE_SCENE),
        usize::try_from(DO_NOT_DUPLICATE).expect("fits"),
        to(DUP_REFS),
        to(DUP_COPY),
        to(DUP_PRIVATE_REFS),
        to(DUP_PRIVATE_COPY),
        to(OBS_SOURCE_DUPLICATE_NONE),
        to(OBS_SOURCE_DUPLICATE_REF),
        to(OBS_SOURCE_DUPLICATE_ASK_SCENE),
        to(OBS_SOURCE_DUPLICATE_ASK_GROUP),
        to(OBS_SOURCE_DUPLICATE_SCENE_COPY),
        to(OBS_SOURCE_DUPLICATE_ASK_FLAGS),
        to(OBS_SOURCE_DUPLICATE_CREATE),
        to(OBS_SOURCE_DUPLICATE_COPY_STATE),
        to(OBS_SCENE_DUPLICATE_ITEM_GROUP),
        to(OBS_SCENE_DUPLICATE_ITEM_CHILD),
        to(OBS_SCENE_DUPLICATE_ITEM_REF),
        to(OBS_SCENE_DUPLICATE_REUSE_NONE),
        to(OBS_SCENE_DUPLICATE_REUSE_AT_INDEX),
        to(OBS_SCENE_DUPLICATE_REUSE_COPY),
        size_of::<c_int>(),
        size_of::<c_int>(),
        size_of::<c_int>(),
        size_of::<bool>(),
    ];
    for (which, &value) in want.iter().enumerate() {
        assert_eq!(layout(which), value, "value {which}");
    }
    assert_eq!(layout(want.len()), 0);
}

#[test]
fn source_steps_match_c() {
    for t in ints() {
        // SAFETY: plain values in and out.
        unsafe {
            assert_eq!(
                obs_source_duplicate_type_step(t),
                oracle_obs_source_duplicate_type_step(t),
                "type_step({t})"
            );
            assert_eq!(
                obs_source_duplicate_copies_filters(t),
                oracle_obs_source_duplicate_copies_filters(t),
                "copies_filters({t})"
            );
        }
    }
    for has_scene in BOOLS {
        for create_private in BOOLS {
            // SAFETY: plain values in and out.
            unsafe {
                assert_eq!(
                    obs_source_duplicate_scene_step(has_scene, create_private),
                    oracle_obs_source_duplicate_scene_step(has_scene, create_private),
                    "scene_step({has_scene}, {create_private})"
                );
            }
        }
        // SAFETY: plain values in and out.
        unsafe {
            assert_eq!(
                obs_source_duplicate_group_step(has_scene),
                oracle_obs_source_duplicate_group_step(has_scene)
            );
            assert_eq!(
                obs_source_duplicate_scene_type(has_scene),
                oracle_obs_source_duplicate_scene_type(has_scene)
            );
        }
    }
    let mut flags: Vec<u32> = vec![0, u32::MAX];
    for bit in 0..32 {
        flags.push(1 << bit);
        flags.push(!(1 << bit));
        flags.push((1 << bit) | DO_NOT_DUPLICATE);
    }
    for f in flags {
        // SAFETY: plain values in and out.
        unsafe {
            assert_eq!(
                obs_source_duplicate_flags_step(f),
                oracle_obs_source_duplicate_flags_step(f),
                "flags_step({f:#x})"
            );
        }
    }
}

/// Intentional difference 1: a create that returned no source.
#[test]
fn created_step_differs_only_without_a_source() {
    // SAFETY: plain values in and out.
    unsafe {
        assert_eq!(
            obs_source_duplicate_created_step(true),
            oracle_obs_source_duplicate_created_step(true)
        );
        assert_eq!(
            oracle_obs_source_duplicate_created_step(false),
            OBS_SOURCE_DUPLICATE_COPY_STATE
        );
    }
    assert_eq!(
        obs_source_duplicate_created_step(false),
        OBS_SOURCE_DUPLICATE_NONE
    );
}

#[test]
fn filter_numbers_match_c() {
    let near_ends = (c_int::MIN..c_int::MIN + 1000).chain(c_int::MAX - 1000..c_int::MAX);
    for taken in (-100_000..=100_000).chain(near_ends) {
        // SAFETY: plain values in and out; taken < INT_MAX, so the C does
        // not overflow.
        let c = unsafe { oracle_obs_source_duplicate_filter_number(taken) };
        assert_eq!(obs_source_duplicate_filter_number(taken), c, "{taken}");
        assert_eq!(core::filter_number(taken), c);
    }
    // The C overflows at INT_MAX (not compared); the shim stays there.
    assert_eq!(obs_source_duplicate_filter_number(c_int::MAX), c_int::MAX);
}

#[test]
fn scene_flags_match_c() {
    for t in ints() {
        for unique in BOOLS {
            for is_group in BOOLS {
                // SAFETY: plain values in and out.
                unsafe {
                    assert_eq!(
                        obs_scene_duplicate_keeps_unique(unique, is_group, t),
                        oracle_obs_scene_duplicate_keeps_unique(unique, is_group, t),
                        "keeps_unique({unique}, {is_group}, {t})"
                    );
                }
            }
        }
        // SAFETY: plain values in and out.
        unsafe {
            assert_eq!(
                obs_scene_duplicate_unique(t),
                oracle_obs_scene_duplicate_unique(t),
                "unique({t})"
            );
            assert_eq!(
                obs_scene_duplicate_private(t),
                oracle_obs_scene_duplicate_private(t),
                "private({t})"
            );
        }
    }
    for a in BOOLS {
        for b in BOOLS {
            // SAFETY: plain values in and out.
            unsafe {
                assert_eq!(
                    obs_scene_duplicate_item_route(a, b),
                    oracle_obs_scene_duplicate_item_route(a, b),
                    "item_route({a}, {b})"
                );
            }
        }
        // SAFETY: plain values in and out.
        unsafe {
            assert_eq!(
                obs_scene_duplicate_keeps_name(a),
                oracle_obs_scene_duplicate_keeps_name(a)
            );
            assert_eq!(
                obs_scene_duplicate_resizes(a),
                oracle_obs_scene_duplicate_resizes(a)
            );
        }
    }
}

static SOURCES: [u8; 3] = [0; 3];

fn source(i: usize) -> *const c_void {
    if i == 0 {
        std::ptr::null()
    } else {
        std::ptr::from_ref(&SOURCES[i - 1]).cast()
    }
}

/// Intentional difference 2, step by step: the C reuses the first earlier
/// item with the same source by its index, the shim that item's own copy,
/// and only when it was added.
#[test]
fn reuse_step_differs_only_for_the_same_source() {
    for e in 0..=3 {
        for s in 0..=3 {
            for copied in BOOLS {
                let (earlier, src) = (source(e), source(s));
                let shim = obs_scene_duplicate_reuse(earlier, src, copied);
                // SAFETY: the oracle only compares the pointers.
                let c = unsafe { oracle_obs_scene_duplicate_reuse(earlier, src, copied) };
                if e == s {
                    assert_eq!(c, OBS_SCENE_DUPLICATE_REUSE_AT_INDEX);
                    let want = if copied {
                        OBS_SCENE_DUPLICATE_REUSE_COPY
                    } else {
                        OBS_SCENE_DUPLICATE_REUSE_NONE
                    };
                    assert_eq!(shim, want, "{e} {s} {copied}");
                } else {
                    assert_eq!(shim, c, "{e} {s} {copied}");
                    assert_eq!(shim, OBS_SCENE_DUPLICATE_REUSE_NONE);
                }
            }
        }
    }
}

/// What happens to one old item in the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fate {
    /// Copied: its duplicate is refused only if it was not reused.
    Copied,
    /// `obs_source_duplicate()` returns NULL for it.
    DupFails,
    /// `obs_scene_add()` refuses it.
    AddFails,
}

/// A copy in the new scene: the old source it copies, and which copy.
type CopyId = (usize, usize);

type ReuseFn = fn(*const c_void, *const c_void, bool) -> c_int;

fn c_reuse(earlier: *const c_void, src: *const c_void, copied: bool) -> c_int {
    // SAFETY: the oracle only compares the pointers.
    unsafe { oracle_obs_scene_duplicate_reuse(earlier, src, copied) }
}

/// A model of `obs_scene_duplicate()`'s item loop for a unique copy of a
/// scene: `dup_child()` with `reuse` asking each earlier item, the copy or
/// the reused source added unless `obs_scene_add()` refuses it. Returns the
/// new scene's items and each old item's kept copy.
fn duplicate_scene(
    reuse: ReuseFn,
    sources: &[usize],
    fates: &[Fate],
) -> (Vec<CopyId>, Vec<Option<CopyId>>) {
    let mut new_scene: Vec<CopyId> = Vec::new();
    let mut copies: Vec<Option<CopyId>> = Vec::new();
    for (idx, (&s, &fate)) in sources.iter().zip(fates).enumerate() {
        let mut got: Option<Option<CopyId>> = None;
        for i in 0..idx {
            let r = reuse(source(sources[i]), source(s), copies[i].is_some());
            if r == OBS_SCENE_DUPLICATE_REUSE_AT_INDEX {
                // get_child_at_idx(): NULL past the end
                got = Some(new_scene.get(i).copied());
                break;
            } else if r == OBS_SCENE_DUPLICATE_REUSE_COPY {
                got = Some(copies[i]);
                break;
            }
        }
        let new_source = got.unwrap_or(if fate == Fate::DupFails {
            None
        } else {
            Some((s, idx))
        });
        let kept = match new_source {
            Some(copy) if fate != Fate::AddFails => {
                new_scene.push(copy);
                Some(copy)
            }
            _ => None,
        };
        copies.push(kept);
    }
    (new_scene, copies)
}

fn shim_reuse(earlier: *const c_void, src: *const c_void, copied: bool) -> c_int {
    obs_scene_duplicate_reuse(earlier, src, copied)
}

/// Checks one scene: the same new scene from the shims and the C while
/// every item is copied, and from the shims, only copies of each item's
/// own source, each reusing the first earlier added copy of that source.
/// Returns whether the C gave an item another item's source.
fn check_scene(sources: &[usize], fates: &[Fate]) -> bool {
    let (shim_scene, shim_copies) = duplicate_scene(shim_reuse, sources, fates);
    let (c_scene, c_copies) = duplicate_scene(c_reuse, sources, fates);

    if fates.iter().all(|&f| f == Fate::Copied) {
        assert_eq!(shim_scene, c_scene, "{sources:?}");
        assert_eq!(shim_copies, c_copies);
    }

    for (idx, copy) in shim_copies.iter().enumerate() {
        let Some((of, which)) = *copy else {
            continue;
        };
        assert_eq!(
            of, sources[idx],
            "shim: item {idx} of {sources:?} {fates:?}"
        );
        let first = (0..idx).find(|&i| sources[i] == sources[idx] && shim_copies[i].is_some());
        assert_eq!(
            which,
            first.map_or(idx, |i| shim_copies[i].map_or(i, |c| c.1)),
            "shim: item {idx} of {sources:?} {fates:?}"
        );
    }

    c_copies
        .iter()
        .enumerate()
        .any(|(idx, copy)| copy.is_some_and(|(of, _)| of != sources[idx]))
}

#[test]
fn scene_copies_match_c_while_every_item_is_added() {
    let fates_all = [Fate::Copied, Fate::DupFails, Fate::AddFails];
    let mut c_wrong = 0_u32;
    let mut scenes = 0_u32;
    for len in 0..=6_u32 {
        let combos = 3_usize.pow(len);
        let len = usize::try_from(len).expect("small");
        for s in 0..combos {
            let sources: Vec<usize> = (0..len)
                .map(|i| 1 + (s / 3_usize.pow(u32::try_from(i).expect("small"))) % 3)
                .collect();
            for f in 0..combos {
                let fates: Vec<Fate> = (0..len)
                    .map(|i| fates_all[(f / 3_usize.pow(u32::try_from(i).expect("small"))) % 3])
                    .collect();
                scenes += 1;
                if check_scene(&sources, &fates) {
                    c_wrong += 1;
                }
            }
        }
    }
    // The C's wrong source (intentional difference 2) does happen.
    assert!(c_wrong > 0);
    assert!(scenes > 500_000);
}

proptest! {
    #[test]
    fn steps_match_c_anywhere(
        t in any::<c_int>(),
        f in any::<u32>(),
        a in any::<bool>(),
        b in any::<bool>(),
        taken in any::<c_int>().prop_filter("C overflows at INT_MAX", |&n| n != c_int::MAX),
    ) {
        // SAFETY: plain values in and out.
        unsafe {
            prop_assert_eq!(obs_source_duplicate_type_step(t), oracle_obs_source_duplicate_type_step(t));
            prop_assert_eq!(obs_source_duplicate_copies_filters(t), oracle_obs_source_duplicate_copies_filters(t));
            prop_assert_eq!(obs_source_duplicate_flags_step(f), oracle_obs_source_duplicate_flags_step(f));
            prop_assert_eq!(obs_source_duplicate_filter_number(taken), oracle_obs_source_duplicate_filter_number(taken));
            prop_assert_eq!(obs_scene_duplicate_unique(t), oracle_obs_scene_duplicate_unique(t));
            prop_assert_eq!(obs_scene_duplicate_private(t), oracle_obs_scene_duplicate_private(t));
            prop_assert_eq!(
                obs_scene_duplicate_keeps_unique(a, b, t),
                oracle_obs_scene_duplicate_keeps_unique(a, b, t)
            );
        }
    }

    #[test]
    fn scene_copies_hold_on_larger_scenes(
        items in proptest::collection::vec((1..5_usize, 0..6_u8), 0..=12),
    ) {
        let sources: Vec<usize> = items.iter().map(|i| i.0.min(3)).collect();
        let fates: Vec<Fate> = items
            .iter()
            .map(|i| match i.1 {
                0 => Fate::DupFails,
                1 => Fate::AddFails,
                _ => Fate::Copied,
            })
            .collect();
        check_scene(&sources, &fates);
    }
}
