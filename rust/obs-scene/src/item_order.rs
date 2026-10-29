//! Safe core of `libobs/obs-scene-item-order.c.h`: the scene item order,
//! reorder and group membership decisions of `obs-scene.c.h`.
//!
//! `obs-scene.c.h` keeps every lock, every scene reference, every list walk
//! and every `prev`, `next`, `first_item` and `parent` write
//! (`detach_sceneitem()`, `attach_sceneitem()`), `build_current_order_info()`,
//! `move_would_recurse()`, the group transforms, `resize_group()`, the hotkey
//! moves, every signal and every log, and asks each question where the C
//! decided it:
//! - [`anchor`]: where `obs_sceneitem_set_order()` puts the item back;
//! - [`position`]: where `obs_sceneitem_set_order_position()` puts it;
//! - [`item_attached`]: whether `obs_sceneitem_get_order_position()` and
//!   `obs_sceneitem_group_ungroup2()` go on with an item;
//! - [`match_item`]: one step of the check of `obs_scene_reorder_items()`;
//! - [`reorder_check`] and [`group_run_end`]: the check and the group runs
//!   of `obs_scene_reorder_items2()`;
//! - [`add_item_allowed`] and [`remove_item_allowed`]: the gates of
//!   `obs_sceneitem_group_add_item()` and `obs_sceneitem_group_remove_item()`.
//!
//! Items are ids that are only compared (the C passes the item pointers),
//! never followed. Nothing here keeps state, takes a lock or calls libobs.
//!
//! Intentional differences from the C, each a fix (#2895):
//! 1. [`anchor`]: a movement outside `enum obs_order_movement` is
//!    [`Anchor::Stay`]: the item stays where it was. The C detaches it and
//!    never attaches it again.
//! 2. [`item_attached`]: a NULL or detached item stops the caller. The C
//!    reads `item->parent` whatever it is given (and locks a NULL scene for
//!    a detached item).
//! 3. [`reorder_check`]: an order whose items are not exactly the scene's
//!    items (top level and in groups), each once, is
//!    [`ReorderCheck::Invalid`]. The C relinks any order that differs from
//!    the current one, so a missing item is unlinked while its `parent`
//!    still names the scene, and an item from elsewhere or a repeat is
//!    linked in.
//! 4. [`remove_item_allowed`]: an item that is not in the group stays where
//!    it is. The C has no check, so a top-level item or one from another
//!    scene loses the group's transform and moves to the group's parent
//!    scene.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::as_conversions,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable
)]

/// `enum obs_order_movement` (`obs.h`).
pub const OBS_ORDER_MOVE_UP: i32 = 0;
/// `enum obs_order_movement` (`obs.h`).
pub const OBS_ORDER_MOVE_DOWN: i32 = 1;
/// `enum obs_order_movement` (`obs.h`).
pub const OBS_ORDER_MOVE_TOP: i32 = 2;
/// `enum obs_order_movement` (`obs.h`).
pub const OBS_ORDER_MOVE_BOTTOM: i32 = 3;

/// Where `obs_sceneitem_set_order()` attaches the item after detaching it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Anchor {
    /// At the bottom (no previous item).
    Bottom,
    /// After `prev->prev`: the item below the one it was above (the bottom
    /// when that is NULL).
    PrevPrev,
    /// After `next`: the item it was below.
    Next,
    /// After `prev`: the item it was above (the bottom when NULL).
    Prev,
    /// After the last item.
    Last,
    /// Not detached at all: the item stays where it is (difference 1).
    Stay,
}

/// The movement branches of `obs_sceneitem_set_order()`. `has_prev` and
/// `has_next`: whether the item had an item below and above it.
pub fn anchor(movement: i32, has_prev: bool, has_next: bool) -> Anchor {
    let _ = (movement, has_prev, has_next);
    todo!()
}

/// Where `obs_sceneitem_set_order_position()` attaches the item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Position {
    /// At the bottom.
    Bottom,
    /// After the item `steps` items above the first one (the caller's walk
    /// stops early at the last item).
    After { steps: usize },
}

/// The bottom test and the walk length of
/// `obs_sceneitem_set_order_position()`. `has_first`: whether the scene
/// still has a first item after the detach.
///
/// The bottom for an empty scene or position 0; otherwise `position - 1`
/// steps for a position above 1, and none for 1 or a negative position.
pub fn position(position: i32, has_first: bool) -> Position {
    let _ = (position, has_first);
    todo!()
}

/// Whether `obs_sceneitem_get_order_position()` and
/// `obs_sceneitem_group_ungroup2()` go on with an item: it is not NULL and
/// it is in a scene (difference 2).
pub fn item_attached(has_item: bool, has_parent: bool) -> bool {
    let _ = (has_item, has_parent);
    todo!()
}

/// The inner loop of `sceneitems_match()`: whether `item`, the scene's item
/// at `index` from the bottom, is in `order`. Clears `order_matches` when
/// its first entry is not at `index`.
pub fn match_item<T: PartialEq>(
    order: &[T],
    item: &T,
    index: usize,
    order_matches: &mut bool,
) -> bool {
    let _ = (order, item, index, order_matches);
    todo!()
}

/// One `struct obs_sceneitem_order_info`: an item, and the group it goes
/// into (`None` at the top level).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Entry<T> {
    pub group: Option<T>,
    pub item: T,
}

/// What `obs_scene_reorder_items2()` does with an order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReorderCheck {
    /// The order is the current one: refused, nothing to do.
    Same,
    /// Relink the scene in this order.
    Apply,
    /// The order is not the scene's items rearranged: refused
    /// (difference 3).
    Invalid,
}

/// The check of `obs_scene_reorder_items2()`: `current` is
/// `build_current_order_info()`'s pairs, `order` the order asked for.
///
/// [`ReorderCheck::Same`] when every pair is the current one;
/// [`ReorderCheck::Invalid`] unless the items of `order` are those of
/// `current`, each once; else [`ReorderCheck::Apply`]. The groups are not
/// checked: the caller only compares them to the items it links.
pub fn reorder_check<T: Ord + Copy>(current: &[Entry<T>], order: &[Entry<T>]) -> ReorderCheck {
    let _ = (current, order);
    todo!()
}

/// The end of the group run of `obs_scene_reorder_items2()`: the index
/// after the entries that follow `head` with `order[head].item` as their
/// group. `head` past the end gives `head + 1` (the C would read past the
/// order; libobs never asks).
pub fn group_run_end<T: PartialEq>(order: &[Entry<T>], head: usize) -> usize {
    let _ = (order, head);
    todo!()
}

/// The two parent checks of `obs_sceneitem_group_add_item()`: the item is
/// in the group's parent scene and not already in the group's own scene.
pub fn add_item_allowed(in_scene: bool, in_group: bool) -> bool {
    let _ = (in_scene, in_group);
    todo!()
}

/// The check of `obs_sceneitem_group_remove_item()`: the item is in the
/// group's own scene (difference 4).
pub fn remove_item_allowed(in_group: bool) -> bool {
    let _ = in_group;
    todo!()
}
