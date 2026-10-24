//! `oracle_*` build of `libobs/obs-source-size.c.h` (oracle/source_size.c):
//! the source size query routing, renamed. The node is passed by value in
//! the C layout, mirrored here.
//!
//! `oracle32_*` (oracle/source_size_long32.c) is the same unchanged file
//! built with a 32-bit `long`, as MSVC builds it, so a host whose `long` is
//! 64 bits can compare the route of every value a 32-bit `long` holds.

use core::ffi::{c_int, c_long};

/// Index of each entry in `oracle_source_size_layout`, in the C's order.
pub const LAYOUT_NAMES: [&str; 23] = [
    "sizeof(struct obs_source_size_node)",
    "alignof(struct obs_source_size_node)",
    "offsetof(node, type)",
    "offsetof(node, has_data)",
    "offsetof(node, has_callback)",
    "offsetof(node, has_filter_parent)",
    "offsetof(node, enabled)",
    "offsetof(node, async_active)",
    "offsetof(node, async_rotation)",
    "sizeof(enum obs_source_size_ask)",
    "OBS_SOURCE_SIZE_TRANSITION",
    "OBS_SOURCE_SIZE_CALLBACK",
    "OBS_SOURCE_SIZE_ASK_TARGET",
    "OBS_SOURCE_SIZE_ASYNC",
    "OBS_SOURCE_SIZE_ASYNC_SWAPPED",
    "OBS_SOURCE_SIZE_ZERO",
    "sizeof(enum obs_source_size_start)",
    "OBS_SOURCE_SIZE_ENTRY_SELF",
    "OBS_SOURCE_SIZE_ENTRY_FIRST_FILTER",
    "OBS_SOURCE_SIZE_TYPE_FILTER",
    "OBS_SOURCE_SIZE_TYPE_TRANSITION",
    "sizeof(long)",
    "sizeof(int)",
];

/// `struct obs_source_size_node`, as the C lays it out.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OracleSizeNode {
    pub source_type: c_int,
    pub has_data: bool,
    pub has_callback: bool,
    pub has_filter_parent: bool,
    pub enabled: bool,
    pub async_active: bool,
    pub async_rotation: c_long,
}

/// `struct obs_source_size_node` with a 32-bit `long`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OracleSizeNode32 {
    pub source_type: c_int,
    pub has_data: bool,
    pub has_callback: bool,
    pub has_filter_parent: bool,
    pub enabled: bool,
    pub async_active: bool,
    pub async_rotation: i32,
}

unsafe extern "C" {
    /// `obs_source_size_route`: the route as the C's enum, an `int` or
    /// `unsigned int` (every value fits both).
    pub fn oracle_obs_source_size_route(node: OracleSizeNode) -> u32;
    /// `obs_source_size_entry`: the start as the C's enum.
    pub fn oracle_obs_source_size_entry(source_type: c_int, has_filters: bool) -> u32;
    /// The entry named `LAYOUT_NAMES[which]`; 0 out of range.
    pub fn oracle_source_size_layout(which: usize) -> usize;
    /// `obs_source_size_route` built with a 32-bit `long`.
    pub fn oracle32_obs_source_size_route(node: OracleSizeNode32) -> u32;
    /// `obs_source_size_entry` built with a 32-bit `long`.
    pub fn oracle32_obs_source_size_entry(source_type: c_int, has_filters: bool) -> u32;
    /// The 32-bit node's size (0), its rotation's offset (1) and
    /// `sizeof(long)` there (2); 0 out of range.
    pub fn oracle32_source_size_layout(which: usize) -> usize;
}
