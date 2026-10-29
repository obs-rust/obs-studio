//! C ABI for `libobs/obs-scene-item-order.c.h`, the scene item order,
//! reorder and group membership decisions of `obs-scene.c.h`, declared in
//! `libobs/obs-scene-item-order.h` without `EXPORT`.
//!
//! `obs-scene.c.h` (still C) reads the neighbours, the parents and the
//! position where it made each decision and passes them by value. The item
//! pointers of `obs_scene_reorder_items()` and the pairs of
//! `obs_scene_reorder_items2()` cross as arrays the shims only read and
//! compare by address; no item is followed, so `obs_sceneitem_t` stays
//! opaque. `struct obs_sceneitem_order_info` is mirrored as
//! [`OrderInfo`], which the core reads in place ([`Pair`]): no order is
//! copied. These shims take no lock and call nothing in libobs. All
//! stay hidden (`rust-hidden.txt`).
//!
//! The enum values cross as `int`: [`anchor_value`] and [`check_value`]
//! give `obs-scene-item-order.h`'s values, and `obs-scene.c.h` checks
//! `enum obs_order_movement` against [`crate::item_order`]'s constants at
//! compile time. The intentional differences are the core's
//! ([`crate::item_order`]). A null array is read as empty only with a
//! length of 0, as the C reads nothing then.

use std::ffi::{c_int, c_long, c_void};

use crate::item_order::{self as core, Anchor, Pair, Position, ReorderCheck};

/// `OBS_SCENE_ITEM_ORDER_ANCHOR_BOTTOM`.
pub const OBS_SCENE_ITEM_ORDER_ANCHOR_BOTTOM: c_int = 1;
/// `OBS_SCENE_ITEM_ORDER_ANCHOR_PREV_PREV`.
pub const OBS_SCENE_ITEM_ORDER_ANCHOR_PREV_PREV: c_int = 2;
/// `OBS_SCENE_ITEM_ORDER_ANCHOR_NEXT`.
pub const OBS_SCENE_ITEM_ORDER_ANCHOR_NEXT: c_int = 3;
/// `OBS_SCENE_ITEM_ORDER_ANCHOR_PREV`.
pub const OBS_SCENE_ITEM_ORDER_ANCHOR_PREV: c_int = 4;
/// `OBS_SCENE_ITEM_ORDER_ANCHOR_LAST`.
pub const OBS_SCENE_ITEM_ORDER_ANCHOR_LAST: c_int = 5;
/// `OBS_SCENE_ITEM_ORDER_ANCHOR_STAY`.
pub const OBS_SCENE_ITEM_ORDER_ANCHOR_STAY: c_int = 6;

/// `OBS_SCENE_ITEM_ORDER_SAME`.
pub const OBS_SCENE_ITEM_ORDER_SAME: c_int = 0;
/// `OBS_SCENE_ITEM_ORDER_APPLY`.
pub const OBS_SCENE_ITEM_ORDER_APPLY: c_int = 1;
/// `OBS_SCENE_ITEM_ORDER_INVALID`.
pub const OBS_SCENE_ITEM_ORDER_INVALID: c_int = 2;

/// `struct obs_sceneitem_order_info` (`obs.h`): two `obs_sceneitem_t *`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrderInfo {
    pub group: *const c_void,
    pub item: *const c_void,
}

/// The `int` the C reads for an anchor.
pub fn anchor_value(anchor: Anchor) -> c_int {
    match anchor {
        Anchor::Bottom => OBS_SCENE_ITEM_ORDER_ANCHOR_BOTTOM,
        Anchor::PrevPrev => OBS_SCENE_ITEM_ORDER_ANCHOR_PREV_PREV,
        Anchor::Next => OBS_SCENE_ITEM_ORDER_ANCHOR_NEXT,
        Anchor::Prev => OBS_SCENE_ITEM_ORDER_ANCHOR_PREV,
        Anchor::Last => OBS_SCENE_ITEM_ORDER_ANCHOR_LAST,
        Anchor::Stay => OBS_SCENE_ITEM_ORDER_ANCHOR_STAY,
    }
}

/// The `int` the C reads for a reorder check.
pub fn check_value(check: ReorderCheck) -> c_int {
    match check {
        ReorderCheck::Same => OBS_SCENE_ITEM_ORDER_SAME,
        ReorderCheck::Apply => OBS_SCENE_ITEM_ORDER_APPLY,
        ReorderCheck::Invalid => OBS_SCENE_ITEM_ORDER_INVALID,
    }
}

/// The pair as the core reads it, in place: the addresses, with a NULL
/// group as the top level. A NULL item is address 0, which no scene item
/// has.
impl Pair for OrderInfo {
    type Id = usize;

    fn group(&self) -> Option<usize> {
        (!self.group.is_null()).then(|| self.group.addr())
    }

    fn item(&self) -> usize {
        self.item.addr()
    }
}

/// `size` elements at `data`, or none for a size of 0.
///
/// # Safety
///
/// With `size` above 0, `data` points to `size` initialized, aligned `T`s
/// that stay unchanged for `'a`.
unsafe fn slice<'a, T>(data: *const T, size: usize) -> &'a [T] {
    if size == 0 || data.is_null() {
        return &[];
    }
    // SAFETY: the caller guarantees `size` readable elements at `data`.
    unsafe { std::slice::from_raw_parts(data, size) }
}

/// `int obs_scene_item_order_anchor(int movement, bool has_prev, bool
/// has_next)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_scene_item_order_anchor(
    movement: c_int,
    has_prev: bool,
    has_next: bool,
) -> c_int {
    anchor_value(core::anchor(movement, has_prev, has_next))
}

/// `bool obs_scene_item_order_position(int position, bool has_first,
/// size_t *steps)`. `*steps` is written only when the result is false.
///
/// # Safety
///
/// `steps` points to a writable, aligned `size_t` that nothing else uses
/// during the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_scene_item_order_position(
    position: c_int,
    has_first: bool,
    steps: *mut usize,
) -> bool {
    match core::position(position, has_first) {
        Position::Bottom => true,
        Position::After { steps: walk } => {
            // SAFETY: the caller guarantees `steps` is writable.
            unsafe { *steps = walk };
            false
        }
    }
}

/// `bool obs_scene_item_order_item_attached(bool has_item, bool
/// has_parent)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_scene_item_order_item_attached(has_item: bool, has_parent: bool) -> bool {
    core::item_attached(has_item, has_parent)
}

/// `bool obs_scene_item_order_match_item(obs_sceneitem_t *const *order,
/// size_t size, const obs_sceneitem_t *item, size_t index, bool
/// *order_matches)`. `*order_matches` is written only to clear it.
///
/// # Safety
///
/// With `size` above 0, `order` points to `size` readable pointers.
/// `order_matches` points to a readable and writable, aligned `bool` that
/// nothing else uses during the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_scene_item_order_match_item(
    order: *const *const c_void,
    size: usize,
    item: *const c_void,
    index: usize,
    order_matches: *mut bool,
) -> bool {
    // SAFETY: the caller guarantees `size` readable pointers at `order`.
    let order = unsafe { slice(order, size) };
    // SAFETY: the caller guarantees `order_matches` is readable.
    let mut matches = unsafe { *order_matches };
    let found = core::match_item(order, &item, index, &mut matches);
    if !matches {
        // SAFETY: the caller guarantees `order_matches` is writable.
        unsafe { *order_matches = false };
    }
    found
}

/// `int obs_scene_item_order_reorder_check(const struct
/// obs_sceneitem_order_info *current, size_t current_size, const struct
/// obs_sceneitem_order_info *order, size_t size)`.
///
/// # Safety
///
/// With a size above 0, each array points to that many readable pairs.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_scene_item_order_reorder_check(
    current: *const OrderInfo,
    current_size: usize,
    order: *const OrderInfo,
    size: usize,
) -> c_int {
    // SAFETY: the caller guarantees `current_size` readable pairs.
    let current = unsafe { slice(current, current_size) };
    // SAFETY: the caller guarantees `size` readable pairs.
    let order = unsafe { slice(order, size) };
    check_value(core::reorder_check_pairs(current, order))
}

/// `bool obs_scene_item_order_in_group_run(const struct
/// obs_sceneitem_order_info *order, size_t size, size_t head, size_t
/// index)`. Reads only the pairs at `head` and `index`.
///
/// # Safety
///
/// With `size` above 0, `order` points to `size` readable pairs.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_scene_item_order_in_group_run(
    order: *const OrderInfo,
    size: usize,
    head: usize,
    index: usize,
) -> bool {
    // SAFETY: the caller guarantees `size` readable pairs.
    let order = unsafe { slice(order, size) };
    core::in_group_run_pairs(order, head, index)
}

/// `bool obs_scene_item_order_add_item_allowed(const obs_scene_t *parent,
/// const obs_scene_t *scene, const obs_scene_t *group_scene)`. The scenes
/// are compared by address, never followed.
#[unsafe(no_mangle)]
pub extern "C" fn obs_scene_item_order_add_item_allowed(
    parent: *const c_void,
    scene: *const c_void,
    group_scene: *const c_void,
) -> bool {
    core::add_item_allowed(&parent.addr(), &scene.addr(), &group_scene.addr())
}

/// `bool obs_scene_item_order_remove_item_allowed(bool in_group)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_scene_item_order_remove_item_allowed(in_group: bool) -> bool {
    core::remove_item_allowed(in_group)
}

/// `bool obs_scene_item_order_moves_scene(const obs_scene_t *from, const
/// obs_scene_t *to)`. The scenes are compared by address, never followed.
#[unsafe(no_mangle)]
pub extern "C" fn obs_scene_item_order_moves_scene(from: *const c_void, to: *const c_void) -> bool {
    core::moves_scene(&from.addr(), &to.addr())
}

/// `bool obs_scene_item_order_item_active(long active_refs)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_scene_item_order_item_active(active_refs: c_long) -> bool {
    core::item_active(active_refs)
}
