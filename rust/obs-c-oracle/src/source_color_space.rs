//! `oracle_*` build of `libobs/obs-source-color-space.c.h`
//! (oracle/source_color_space.c): the source colour space query routing,
//! renamed. The node is passed by value in the C layout, mirrored here.

use core::ffi::c_int;

/// Index of each entry in `oracle_source_color_space_layout`, in the C's
/// order.
pub const LAYOUT_NAMES: [&str; 17] = [
    "sizeof(struct obs_source_color_space_node)",
    "alignof(struct obs_source_color_space_node)",
    "offsetof(node, type)",
    "offsetof(node, output_flags)",
    "offsetof(node, has_data)",
    "offsetof(node, enabled)",
    "offsetof(node, has_filter_parent)",
    "offsetof(node, has_filter_target)",
    "sizeof(enum obs_source_color_space_ask)",
    "OBS_SOURCE_COLOR_SPACE_ASK_PARENT",
    "OBS_SOURCE_COLOR_SPACE_ASK_TARGET",
    "OBS_SOURCE_COLOR_SPACE_ASYNC_PICK",
    "OBS_SOURCE_COLOR_SPACE_CALLBACK",
    "OBS_SOURCE_COLOR_SPACE_TYPE_FILTER",
    "OBS_SOURCE_COLOR_SPACE_VIDEO",
    "OBS_SOURCE_COLOR_SPACE_ASYNC",
    "sizeof(int)",
];

/// `struct obs_source_color_space_node`, as the C lays it out.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OracleColorSpaceNode {
    pub source_type: c_int,
    pub output_flags: u32,
    pub has_data: bool,
    pub enabled: bool,
    pub has_filter_parent: bool,
    pub has_filter_target: bool,
}

unsafe extern "C" {
    /// `obs_source_color_space_route`: the route as the C's enum, an `int`
    /// or `unsigned int` (every value fits both).
    pub fn oracle_obs_source_color_space_route(node: OracleColorSpaceNode) -> u32;
    /// `obs_source_color_space_pick`: `preferred_spaces` holds `count`
    /// values (it may be null when `count` is 0).
    pub fn oracle_obs_source_color_space_pick(
        video_space: c_int,
        count: usize,
        preferred_spaces: *const c_int,
    ) -> c_int;
    /// The entry named `LAYOUT_NAMES[which]`; 0 out of range.
    pub fn oracle_source_color_space_layout(which: usize) -> usize;
}
