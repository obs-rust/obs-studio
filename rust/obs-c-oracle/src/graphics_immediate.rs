//! `oracle_*` build of `libobs/graphics/graphics-immediate.c.h`
//! (oracle/graphics_immediate.c): the decisions of the immediate-mode
//! vertex builder, renamed, and the header's constants with the sizes and
//! alignments of the types that cross, through
//! `oracle_gs_immediate_layout`.

use std::ffi::c_int;

/// Index of each entry in `oracle_gs_immediate_layout`, in the C's order.
pub mod layout {
    pub const IMMEDIATE_COUNT: usize = 0;
    pub const TEX_ARRAYS: usize = 1;
    pub const SIZE_T_SIZE: usize = 2;
    pub const SIZE_T_ALIGN: usize = 3;
    pub const BOOL_SIZE: usize = 4;
    pub const BOOL_ALIGN: usize = 5;
    pub const INT_SIZE: usize = 6;
    pub const INT_ALIGN: usize = 7;
}

unsafe extern "C" {
    pub fn oracle_gs_immediate_cut(num: usize, count: usize) -> usize;
    pub fn oracle_gs_immediate_mismatch(count: usize, verts: usize) -> bool;
    pub fn oracle_gs_immediate_push_ok(immediate: bool, num: usize) -> bool;
    pub fn oracle_gs_immediate_save_cut(num: usize, count: usize) -> usize;
    pub fn oracle_gs_immediate_save_free_from(num_tex: usize) -> usize;
    pub fn oracle_gs_immediate_unit_ok(immediate: bool, unit: c_int) -> bool;
    pub fn oracle_gs_immediate_layout(which: usize) -> usize;
}
