//! `oracle_*` build of `libobs/graphics/effect-passes.c.h`
//! (oracle/effect_passes.c), and the recording graphics stand-in both it
//! and the Rust shim call (oracle/effect_passes_device.c). Effects,
//! techniques and graphics subsystems cross as untyped pointers; the caller
//! lays the effects out (the mirrors of `obs_graphics::ffi::effect_lookups`).

use core::ffi::{c_char, c_void};

unsafe extern "C" {
    pub fn oracle_gs_effect_loop(effect: *mut c_void, name: *const c_char) -> bool;
    pub fn oracle_gs_technique_begin(tech: *mut c_void) -> usize;
    pub fn oracle_gs_technique_end(tech: *mut c_void);
    pub fn oracle_gs_effect_update_params(effect: *mut c_void);
    pub fn oracle_gs_technique_begin_pass(tech: *mut c_void, idx: usize) -> bool;
    pub fn oracle_gs_technique_begin_pass_by_name(tech: *mut c_void, name: *const c_char) -> bool;
    pub fn oracle_gs_technique_end_pass(tech: *mut c_void);

    /// A zeroed `struct graphics_subsystem` for an effect's `graphics`.
    pub fn oracle_passes_graphics_new() -> *mut c_void;
    pub fn oracle_passes_graphics_free(graphics: *mut c_void);
    /// The subsystem `gs_get_effect` reads.
    pub fn oracle_passes_set_graphics(graphics: *mut c_void);
    pub fn oracle_passes_cur_effect(graphics: *mut c_void) -> *mut c_void;
    pub fn oracle_passes_set_cur_effect(graphics: *mut c_void, effect: *mut c_void);
    /// size, align, offsetof type, offsetof name of
    /// `struct gs_shader_param_info`; 0 out of range.
    pub fn oracle_passes_shader_param_info_layout(which: usize) -> usize;
    /// The recorded device calls; recording on by default (the bench turns
    /// it off, so the stand-in does nothing).
    pub fn oracle_passes_set_recording(recording: bool);
    pub fn oracle_passes_reset();
    pub fn oracle_passes_count() -> usize;
    pub fn oracle_passes_event(i: usize) -> *const c_char;
}
