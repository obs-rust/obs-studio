//! `oracle_*` build of `libobs/graphics/texture-render.c.h`
//! (oracle/texture_render.c) and the recording `gs_*` stand-in both it and
//! the Rust shim call (oracle/texture_render_device.c).

use core::ffi::{c_char, c_int, c_void};

unsafe extern "C" {
    pub fn oracle_gs_texrender_create(format: u32, zsformat: u32) -> *mut c_void;
    pub fn oracle_gs_texrender_destroy(texrender: *mut c_void);
    pub fn oracle_gs_texrender_begin(texrender: *mut c_void, cx: u32, cy: u32) -> bool;
    pub fn oracle_gs_texrender_begin_with_color_space(
        texrender: *mut c_void,
        cx: u32,
        cy: u32,
        space: u32,
    ) -> bool;
    pub fn oracle_gs_texrender_end(texrender: *mut c_void);
    pub fn oracle_gs_texrender_reset(texrender: *mut c_void);
    pub fn oracle_gs_texrender_get_texture(texrender: *const c_void) -> *mut c_void;
    pub fn oracle_gs_texrender_get_format(texrender: *const c_void) -> u32;

    /// Back to the starting device: screen target and zstencil (index 0),
    /// `GS_CS_709_EXTENDED`, no failures, empty log, handles from 1.
    pub fn oracle_texrender_device_reset();
    /// Makes the next texture and/or zstencil creates return null.
    pub fn oracle_texrender_device_fail(texture: bool, zstencil: bool);
    /// Sets the current target and zstencil (pool index, negative for null)
    /// and color space.
    pub fn oracle_texrender_device_set_state(target: c_int, zstencil: c_int, space: c_int);
    /// -1 for null, the pool index of a texture, 10000 + index of a zstencil.
    pub fn oracle_texrender_device_handle(ptr: *const c_void) -> c_int;
    /// Every recorded call since the last reset or clear, `;`-separated.
    pub fn oracle_texrender_device_log() -> *const c_char;
    pub fn oracle_texrender_device_clear_log();
    /// Stops (or restarts) recording, for benchmarks.
    pub fn oracle_texrender_device_quiet(on: bool);
}
