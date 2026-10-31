//! `oracle_*` build of `libobs/graphics/graphics-immediate.c.h`
//! (oracle/graphics_immediate.c): the decisions of the immediate-mode
//! vertex builder, renamed, and the layout of its two structs, checked
//! through `oracle_gs_immediate_layout`.

use std::ffi::c_int;

/// `struct gs_immediate_draw`, as the C lays it out.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OracleDraw {
    pub num: usize,
    pub norms_mismatch: bool,
    pub colors_mismatch: bool,
    pub tex_mismatch: bool,
}

/// `struct gs_immediate_save`, as the C lays it out.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OracleSave {
    pub num: usize,
    pub num_tex: usize,
    pub free_from: usize,
}

/// Index of each entry in `oracle_gs_immediate_layout`, in the C's order.
pub mod layout {
    pub const DRAW_SIZE: usize = 0;
    pub const DRAW_NUM: usize = 1;
    pub const DRAW_NORMS_MISMATCH: usize = 2;
    pub const DRAW_COLORS_MISMATCH: usize = 3;
    pub const DRAW_TEX_MISMATCH: usize = 4;
    pub const SAVE_SIZE: usize = 5;
    pub const SAVE_NUM: usize = 6;
    pub const SAVE_NUM_TEX: usize = 7;
    pub const SAVE_FREE_FROM: usize = 8;
    pub const IMMEDIATE_COUNT: usize = 9;
    pub const TEX_ARRAYS: usize = 10;
}

unsafe extern "C" {
    pub fn oracle_gs_immediate_unit_ok(immediate: bool, unit: c_int) -> bool;
    pub fn oracle_gs_immediate_push_ok(immediate: bool, num: usize) -> bool;
    pub fn oracle_gs_immediate_draw(
        verts: usize,
        norms: usize,
        colors: usize,
        tex0: usize,
    ) -> OracleDraw;
    pub fn oracle_gs_immediate_save(
        verts: usize,
        norms: usize,
        colors: usize,
        tex: *const usize,
    ) -> OracleSave;
    pub fn oracle_gs_immediate_layout(which: usize) -> usize;
}
