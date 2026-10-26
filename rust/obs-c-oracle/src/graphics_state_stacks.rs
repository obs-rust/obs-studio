//! `oracle_*` build of `libobs/graphics/graphics-state-stacks.c.h`
//! (oracle/graphics_state_stacks.c), on the C graphics math. The darrays
//! are `obs_util`'s `darray`; the values are the shim's `#[repr(C)]` types,
//! checked through `oracle_gs_state_stacks_layout`.

use core::ffi::c_void;

/// `struct gs_blend_reset`, returned by value.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OracleBlendReset {
    pub enable: bool,
    pub function: bool,
    pub op: bool,
}

/// Index of each entry in `oracle_gs_state_stacks_layout`, in the C's order.
pub const LAYOUT_NAMES: [&str; 14] = [
    "sizeof(struct gs_rect)",
    "sizeof(struct blend_state)",
    "offsetof(blend_state, src_c)",
    "offsetof(blend_state, dest_c)",
    "offsetof(blend_state, src_a)",
    "offsetof(blend_state, dest_a)",
    "offsetof(blend_state, op)",
    "sizeof(struct gs_blend_reset)",
    "offsetof(gs_blend_reset, function)",
    "offsetof(gs_blend_reset, op)",
    "sizeof(struct matrix4)",
    "GS_BLEND_SRCALPHA",
    "GS_BLEND_INVSRCALPHA",
    "GS_BLEND_OP_ADD",
];

unsafe extern "C" {
    pub fn oracle_gs_matrix_stack_push(stack: *mut c_void, cur: *mut usize);
    pub fn oracle_gs_matrix_stack_pop(stack: *mut c_void, cur: *mut usize) -> bool;
    pub fn oracle_gs_matrix_stack_identity(stack: *mut c_void, cur: usize);
    pub fn oracle_gs_matrix_stack_transpose(stack: *mut c_void, cur: usize);
    pub fn oracle_gs_matrix_stack_set(stack: *mut c_void, cur: usize, matrix: *const c_void);
    pub fn oracle_gs_matrix_stack_get(stack: *const c_void, cur: usize, dst: *mut c_void);
    pub fn oracle_gs_matrix_stack_mul(stack: *mut c_void, cur: usize, matrix: *const c_void);
    pub fn oracle_gs_matrix_stack_rotquat(stack: *mut c_void, cur: usize, rot: *const c_void);
    pub fn oracle_gs_matrix_stack_rotaa(stack: *mut c_void, cur: usize, rot: *const c_void);
    pub fn oracle_gs_matrix_stack_translate(stack: *mut c_void, cur: usize, pos: *const c_void);
    pub fn oracle_gs_matrix_stack_scale(stack: *mut c_void, cur: usize, scale: *const c_void);
    pub fn oracle_gs_viewport_stack_push(stack: *mut c_void, rect: *const c_void);
    pub fn oracle_gs_viewport_stack_pop(stack: *mut c_void, rect: *mut c_void) -> bool;
    pub fn oracle_gs_blend_state_stack_push(stack: *mut c_void, state: *const c_void);
    pub fn oracle_gs_blend_state_stack_pop(stack: *mut c_void, state: *mut c_void) -> bool;
    pub fn oracle_gs_blend_state_reset_steps(cur: *const c_void) -> OracleBlendReset;
    /// The entry named `LAYOUT_NAMES[which]`; 0 out of range.
    pub fn oracle_gs_state_stacks_layout(which: usize) -> usize;
}
