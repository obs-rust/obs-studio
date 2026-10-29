//! `oracle_*` build of `libobs/obs-scene-item-order.c.h`
//! (oracle/scene_item_order.c): the scene item order, reorder and group
//! membership decisions of `obs-scene.c.h`, renamed. The items cross as
//! opaque pointers, the pairs as `struct obs_sceneitem_order_info`.

use std::ffi::{c_int, c_long, c_void};

/// `struct obs_sceneitem_order_info` (`obs.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OracleOrderInfo {
    pub group: *const c_void,
    pub item: *const c_void,
}

unsafe extern "C" {
    pub fn oracle_obs_scene_item_order_anchor(
        movement: c_int,
        has_prev: bool,
        has_next: bool,
    ) -> c_int;

    pub fn oracle_obs_scene_item_order_position(
        position: c_int,
        has_first: bool,
        steps: *mut usize,
    ) -> bool;

    pub fn oracle_obs_scene_item_order_item_attached(has_item: bool, has_parent: bool) -> bool;

    pub fn oracle_obs_scene_item_order_match_item(
        order: *const *const c_void,
        size: usize,
        item: *const c_void,
        index: usize,
        order_matches: *mut bool,
    ) -> bool;

    pub fn oracle_obs_scene_item_order_reorder_check(
        current: *const OracleOrderInfo,
        current_size: usize,
        order: *const OracleOrderInfo,
        size: usize,
    ) -> c_int;

    pub fn oracle_obs_scene_item_order_in_group_run(
        order: *const OracleOrderInfo,
        size: usize,
        head: usize,
        index: usize,
    ) -> bool;

    pub fn oracle_obs_scene_item_order_add_item_allowed(
        parent: *const c_void,
        scene: *const c_void,
        group_scene: *const c_void,
    ) -> bool;

    pub fn oracle_obs_scene_item_order_moves_scene(from: *const c_void, to: *const c_void) -> bool;

    pub fn oracle_obs_scene_item_order_item_active(active_refs: c_long) -> bool;

    pub fn oracle_obs_scene_item_order_remove_item_allowed(in_group: bool) -> bool;

    /// The layouts and values of `oracle/scene_item_order.c`, by index:
    /// size, alignment and the two field offsets of
    /// `struct obs_sceneitem_order_info`; the size of
    /// `enum obs_order_movement` and its four values; the seven anchor
    /// values (NONE to STAY); the three check values; `sizeof(size_t)`.
    pub fn oracle_scene_item_order_layout(which: usize) -> usize;
}
