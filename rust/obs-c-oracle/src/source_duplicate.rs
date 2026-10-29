//! `oracle_*` build of `libobs/obs-source-duplicate.c.h`
//! (oracle/source_duplicate.c): the duplicate decisions of
//! `obs_source_duplicate()`, `get_new_filter_name()`,
//! `obs_scene_duplicate()`, `dup_child()` and `dup_group()`, renamed. The
//! sources cross as opaque pointers.

use std::ffi::{c_int, c_void};

unsafe extern "C" {
    pub fn oracle_obs_source_duplicate_type_step(source_type: c_int) -> c_int;

    pub fn oracle_obs_source_duplicate_scene_step(has_scene: bool, create_private: bool) -> c_int;

    pub fn oracle_obs_source_duplicate_group_step(has_group: bool) -> c_int;

    pub fn oracle_obs_source_duplicate_scene_type(create_private: bool) -> c_int;

    pub fn oracle_obs_source_duplicate_flags_step(output_flags: u32) -> c_int;

    pub fn oracle_obs_source_duplicate_created_step(has_source: bool) -> c_int;

    pub fn oracle_obs_source_duplicate_copies_filters(source_type: c_int) -> bool;

    pub fn oracle_obs_source_duplicate_filter_number(taken: c_int) -> c_int;

    pub fn oracle_obs_scene_duplicate_unique(dup_type: c_int) -> bool;

    pub fn oracle_obs_scene_duplicate_private(dup_type: c_int) -> bool;

    pub fn oracle_obs_scene_duplicate_keeps_unique(
        unique: bool,
        is_group: bool,
        dup_type: c_int,
    ) -> bool;

    pub fn oracle_obs_scene_duplicate_item_route(is_group: bool, unique: bool) -> c_int;

    pub fn oracle_obs_scene_duplicate_reuse(
        earlier: *const c_void,
        source: *const c_void,
        earlier_copied: bool,
    ) -> c_int;

    pub fn oracle_obs_scene_duplicate_keeps_name(make_private: bool) -> bool;

    pub fn oracle_obs_scene_duplicate_resizes(is_group: bool) -> bool;

    /// The values of `oracle/source_duplicate.c`, by index: the filter and
    /// scene types, `OBS_SOURCE_DUPLICATE_DO_NOT_DUPLICATE`, the four
    /// duplicate types; the eight step values (NONE to COPY_STATE), the
    /// three item routes, the three reuse values; the sizes of the three
    /// enums and of `bool`.
    pub fn oracle_source_duplicate_layout(which: usize) -> usize;
}
