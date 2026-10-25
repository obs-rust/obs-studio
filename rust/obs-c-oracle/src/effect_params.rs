//! `oracle_*` build of `libobs/graphics/effect-params.c.h`
//! (oracle/effect_params.c). The parameter crosses as an untyped pointer to
//! a `struct gs_effect_param` the caller lays out (the mirror of
//! `obs_graphics::ffi::effect_lookups`).

use core::ffi::{c_int, c_void};

/// Index of each entry in `oracle_effect_params_layout`, in the C's order.
pub const LAYOUT_NAMES: [&str; 4] = [
    "sizeof(struct gs_shader_texture)",
    "alignof(struct gs_shader_texture)",
    "offsetof(struct gs_shader_texture, tex)",
    "offsetof(struct gs_shader_texture, srgb)",
];

unsafe extern "C" {
    pub fn oracle_gs_effect_set_bool(param: *mut c_void, val: bool);
    pub fn oracle_gs_effect_set_float(param: *mut c_void, val: f32);
    pub fn oracle_gs_effect_set_int(param: *mut c_void, val: c_int);
    pub fn oracle_gs_effect_set_matrix4(param: *mut c_void, val: *const c_void);
    pub fn oracle_gs_effect_set_vec2(param: *mut c_void, val: *const c_void);
    pub fn oracle_gs_effect_set_vec3(param: *mut c_void, val: *const c_void);
    pub fn oracle_gs_effect_set_vec4(param: *mut c_void, val: *const c_void);
    pub fn oracle_gs_effect_set_color(param: *mut c_void, argb: u32);
    pub fn oracle_gs_effect_set_texture(param: *mut c_void, val: *mut c_void);
    pub fn oracle_gs_effect_set_texture_srgb(param: *mut c_void, val: *mut c_void);
    pub fn oracle_gs_effect_set_val(param: *mut c_void, val: *const c_void, size: usize);
    pub fn oracle_gs_effect_get_val(param: *mut c_void) -> *mut c_void;
    pub fn oracle_gs_effect_get_val_size(param: *mut c_void) -> usize;
    pub fn oracle_gs_effect_get_default_val(param: *mut c_void) -> *mut c_void;
    pub fn oracle_gs_effect_get_default_val_size(param: *mut c_void) -> usize;
    pub fn oracle_gs_effect_set_default(param: *mut c_void);
    pub fn oracle_gs_effect_set_next_sampler(param: *mut c_void, sampler: *mut c_void);
    /// The entry named `LAYOUT_NAMES[which]`; 0 out of range.
    pub fn oracle_effect_params_layout(which: usize) -> usize;
}
