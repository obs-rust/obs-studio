//! Tier 3 parity: the shims of `obs_scene::ffi::scene_transform_update`
//! (and the safe core under them) against the original
//! `libobs/obs-scene-transform-update.c.h`, compiled as
//! `oracle_obs_scene_transform_update_*` (obs-c-oracle,
//! oracle/scene_transform_update.c).
//!
//! Every call runs the shim, the C and the safe core, and compares the
//! results and the stored size exactly.
//!
//! Runs: exhaustive over the flags and a set of sizes at 0, 1, common
//! sizes and the `u32` ends, in every pair; a walk model that runs
//! `update_transforms_and_prune_sources()`'s per-item steps over a scene
//! of items and renders, with the shims on one side and the C on the other
//! (the same glue on both: the removed check, the group rebuild flag, the
//! size read only when the flag is clear); and proptests over the full
//! ranges.
//!
//! No intentional differences. `test_scene_transform_update_steps` runs the
//! C test against the C, `test_scene_transform_update_steps_rust` against
//! these shims; `test_scene_transform_update_walk` runs the real walk
//! through the hybrid libobs, where these shims replace the C.

use obs_c_oracle::scene_transform_update::{
    oracle_obs_scene_transform_update_item, oracle_obs_scene_transform_update_scene_size,
};
use obs_scene::ffi::scene_transform_update::{
    obs_scene_transform_update_item, obs_scene_transform_update_scene_size,
};
use obs_scene::transform_update::{self as core, Size};
use proptest::prelude::*;

const MAX32: u32 = u32::MAX;

/// Sizes at the edges and some common ones.
const DIMS: [u32; 9] = [0, 1, 2, 480, 720, 1080, 1920, MAX32 - 1, MAX32];

// ---------------------------------------------------------------------------
// one call on all three sides

/// The scene size step; returns the result and the stored size.
fn check_scene_size(last: Size, now: Size) -> (bool, Size) {
    let (mut shim_w, mut shim_h) = (last.width, last.height);
    // SAFETY: two locals, written only by the call.
    let shim = unsafe {
        obs_scene_transform_update_scene_size(&mut shim_w, &mut shim_h, now.width, now.height)
    };
    let (mut c_w, mut c_h) = (last.width, last.height);
    // SAFETY: two locals, written only by the call; the oracle keeps no state.
    let c = unsafe {
        oracle_obs_scene_transform_update_scene_size(&mut c_w, &mut c_h, now.width, now.height)
    };
    let mut core_last = last;
    let core = core::scene_size(&mut core_last, now);

    let what = format!("{last:?} -> {now:?}");
    assert_eq!(shim, c, "shim vs C: {what}");
    assert_eq!((shim_w, shim_h), (c_w, c_h), "shim vs C stored: {what}");
    assert_eq!(shim, core, "shim vs core: {what}");
    assert_eq!(
        Size::new(shim_w, shim_h),
        core_last,
        "shim vs core stored: {what}"
    );
    (shim, core_last)
}

fn check_item(update: bool, last: Size, source: Size, scene: bool) -> bool {
    let shim = obs_scene_transform_update_item(
        update,
        last.width,
        last.height,
        source.width,
        source.height,
        scene,
    );
    // SAFETY: plain values in, a bool out; the oracle keeps no state.
    let c = unsafe {
        oracle_obs_scene_transform_update_item(
            update,
            last.width,
            last.height,
            source.width,
            source.height,
            scene,
        )
    };
    let core = core::item_needs_update(update, last, source, scene);
    let what = format!("update {update} last {last:?} source {source:?} scene {scene}");
    assert_eq!(shim, c, "shim vs C: {what}");
    assert_eq!(shim, core, "shim vs core: {what}");
    shim
}

fn sizes() -> impl Iterator<Item = Size> {
    DIMS.iter()
        .flat_map(|&w| DIMS.iter().map(move |&h| Size::new(w, h)))
}

#[test]
fn scene_size_every_pair() {
    let mut changes = 0_usize;
    for last in sizes() {
        for now in sizes() {
            let (changed, _) = check_scene_size(last, now);
            changes += usize::from(changed);
        }
    }
    // 81 sizes: every pair but the 81 equal ones is a change
    assert_eq!(changes, 81 * 81 - 81);
}

#[test]
fn item_every_pattern() {
    let mut updates = 0_usize;
    for update in [false, true] {
        for scene in [false, true] {
            for last in sizes() {
                for source in sizes() {
                    updates += usize::from(check_item(update, last, source, scene));
                }
            }
        }
    }
    // only a clear flag, the same size and an unchanged scene stay
    assert_eq!(updates, 4 * 81 * 81 - 81);
}

// ---------------------------------------------------------------------------
// the walk: update_transforms_and_prune_sources() over a scene of items

/// One item of the model: what obs-scene.c reads where it decides.
#[derive(Debug, Clone, Copy)]
struct Item {
    removed: bool,
    update_transform: bool,
    deferred: bool,
    last: Size,
    source: Size,
}

/// What one walk did: per item removed, updated (transform asked for),
/// whether the source size was read, and the group rebuild at the end.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Walk {
    steps: Vec<(bool, bool, bool)>,
    rebuild_group: bool,
}

type ItemFn = fn(bool, Size, Size, bool) -> bool;

fn shim_item(update: bool, last: Size, source: Size, scene: bool) -> bool {
    obs_scene_transform_update_item(
        update,
        last.width,
        last.height,
        source.width,
        source.height,
        scene,
    )
}

fn c_item(update: bool, last: Size, source: Size, scene: bool) -> bool {
    // SAFETY: plain values in, a bool out; the oracle keeps no state.
    unsafe {
        oracle_obs_scene_transform_update_item(
            update,
            last.width,
            last.height,
            source.width,
            source.height,
            scene,
        )
    }
}

/// The walk of `update_transforms_and_prune_sources()` for one scene, with
/// the glue `obs-scene.c.h` keeps: the removed check first, the size read
/// only when the flag is clear, and `update_item_transform()`, which stores
/// the size and clears the flag unless the item is deferred.
fn walk(items: &mut [Item], group_resize: Option<bool>, scene: bool, decide: ItemFn) -> Walk {
    let mut out = Walk {
        rebuild_group: group_resize == Some(true),
        ..Walk::default()
    };
    for item in items.iter_mut() {
        if item.removed {
            out.steps.push((true, false, false));
            out.rebuild_group = true;
            continue;
        }
        let read = !item.update_transform;
        let source = if read { item.source } else { Size::default() };
        let update = decide(item.update_transform, item.last, source, scene);
        if update {
            if !item.deferred {
                item.last = item.source;
                item.update_transform = false;
            }
            out.rebuild_group = true;
        }
        out.steps.push((false, update, read));
    }
    if group_resize.is_none() {
        out.rebuild_group = false;
    }
    out
}

fn item_strategy() -> impl Strategy<Value = Item> {
    let dim = prop_oneof![Just(0_u32), Just(1), Just(1920), Just(MAX32), 0_u32..4];
    let size = (dim.clone(), dim).prop_map(|(w, h)| Size::new(w, h));
    (
        prop::bool::weighted(0.1),
        any::<bool>(),
        prop::bool::weighted(0.2),
        size.clone(),
        size,
    )
        .prop_map(|(removed, update_transform, deferred, last, source)| Item {
            removed,
            update_transform,
            deferred,
            last,
            source,
        })
}

/// A render of a scene: the scene size, then the walk with its result.
fn render(
    items: &mut [Item],
    stored: &mut Size,
    now: Size,
    group_resize: Option<bool>,
    decide: ItemFn,
    scene_size: fn(&mut Size, Size) -> bool,
) -> (bool, Walk) {
    let changed = scene_size(stored, now);
    (changed, walk(items, group_resize, changed, decide))
}

fn shim_scene_size(last: &mut Size, now: Size) -> bool {
    let (mut w, mut h) = (last.width, last.height);
    // SAFETY: two locals, written only by the call.
    let changed =
        unsafe { obs_scene_transform_update_scene_size(&mut w, &mut h, now.width, now.height) };
    *last = Size::new(w, h);
    changed
}

fn c_scene_size(last: &mut Size, now: Size) -> bool {
    let (mut w, mut h) = (last.width, last.height);
    // SAFETY: two locals, written only by the call; the oracle keeps no state.
    let changed = unsafe {
        oracle_obs_scene_transform_update_scene_size(&mut w, &mut h, now.width, now.height)
    };
    *last = Size::new(w, h);
    changed
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4000))]

    #[test]
    fn scene_size_any(lw in any::<u32>(), lh in any::<u32>(), w in any::<u32>(), h in any::<u32>(), same in any::<bool>()) {
        let last = Size::new(lw, lh);
        let now = if same { last } else { Size::new(w, h) };
        check_scene_size(last, now);
    }

    #[test]
    fn item_any(
        update in any::<bool>(),
        lw in any::<u32>(), lh in any::<u32>(),
        w in any::<u32>(), h in any::<u32>(),
        same in any::<bool>(),
        scene in any::<bool>(),
    ) {
        let last = Size::new(lw, lh);
        let source = if same { last } else { Size::new(w, h) };
        check_item(update, last, source, scene);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// Up to 16 items, 12 renders, with sizes from a few values so that
    /// "the same" happens often; between renders some sources change size
    /// and some flags are set, the same on both sides. The shims and the C
    /// give the same scene size answers, the same per-item steps and the
    /// same group rebuild on every render.
    #[test]
    fn walk_sequences(
        items in prop::collection::vec(item_strategy(), 0..16),
        renders in prop::collection::vec(
            (0_u32..3, 0_u32..3, prop::collection::vec((any::<bool>(), 0_u32..3), 16)),
            1..12,
        ),
        group in prop::option::of(any::<bool>()),
    ) {
        let mut rust_items = items.clone();
        let mut c_items = items;
        let mut rust_stored = Size::default();
        let mut c_stored = Size::default();
        for (w, h, changes) in renders {
            let now = Size::new(w * 960, h * 540);
            let rust = render(&mut rust_items, &mut rust_stored, now, group, shim_item, shim_scene_size);
            let c = render(&mut c_items, &mut c_stored, now, group, c_item, c_scene_size);
            prop_assert_eq!(&rust, &c);
            prop_assert_eq!(rust_stored, c_stored);
            for (i, (flag, size)) in changes.iter().enumerate() {
                for its in [&mut rust_items, &mut c_items] {
                    if let Some(item) = its.get_mut(i) {
                        item.update_transform |= *flag;
                        item.source = Size::new(*size, *size);
                    }
                }
            }
        }
    }
}
