//! `oracle_*` build of `libobs/obs-scene-item-names.c.h`
//! (oracle/scene_item_names.c), with the values its header gives the C. The
//! loaders take the compare as a parameter; pass the C `dstr.c`'s
//! (`crate::dstr::oracle_astrcmpi`) to compare with libobs's C side.

use std::ffi::{c_char, c_int};

/// `obs_scene_item_name_cmp`: `astrcmpi`'s signature.
pub type NameCmp = unsafe extern "C" fn(name: *const c_char, known: *const c_char) -> c_int;

/// How many values `oracle_scene_item_names_layout` gives, in the order of
/// oracle/scene_item_names.c.
pub const LAYOUT_COUNT: usize = 19;

unsafe extern "C" {
    pub fn oracle_obs_scene_item_scale_filter_from_name(name: *const c_char, cmp: NameCmp)
    -> c_int;

    pub fn oracle_obs_scene_item_blend_method_from_name(name: *const c_char, cmp: NameCmp)
    -> c_int;

    pub fn oracle_obs_scene_item_blend_type_from_name(name: *const c_char, cmp: NameCmp) -> c_int;

    pub fn oracle_obs_scene_item_scale_filter_name(scale_filter: c_int) -> *const c_char;

    pub fn oracle_obs_scene_item_blend_method_name(blend_method: c_int) -> *const c_char;

    pub fn oracle_obs_scene_item_blend_type_name(blend_type: c_int) -> *const c_char;

    pub fn oracle_obs_scene_item_transform_keys(
        absolute_coordinates: bool,
        has_pos_rel: bool,
        has_scale_rel: bool,
        has_scale_ref: bool,
    ) -> u32;

    pub fn oracle_obs_scene_item_bounds_keys(
        absolute_coordinates: bool,
        has_bounds_rel: bool,
    ) -> u32;

    pub fn oracle_obs_scene_item_save_relative(absolute_coordinates: bool) -> bool;

    pub fn oracle_scene_item_names_layout(which: usize) -> usize;
}
