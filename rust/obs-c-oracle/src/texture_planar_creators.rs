//! `oracle_*` build of `libobs/graphics/texture-planar-creators.c.h` and the
//! stand-in context it and the Rust shim's field adapter read
//! (oracle/texture_planar_creators.c). Both sides make and destroy planes
//! through the recording `gs_*` stand-in of [`crate::texture_render`].

use core::ffi::c_void;

unsafe extern "C" {
    pub fn oracle_gs_texture_create_nv12(
        tex_y: *mut *mut c_void,
        tex_uv: *mut *mut c_void,
        width: u32,
        height: u32,
        flags: u32,
    ) -> bool;
    pub fn oracle_gs_texture_create_p010(
        tex_y: *mut *mut c_void,
        tex_uv: *mut *mut c_void,
        width: u32,
        height: u32,
        flags: u32,
    ) -> bool;

    /// A fresh stand-in context for this thread: there or not, each planar
    /// device export there or not, what they answer and whether they write
    /// their two tokens to the outputs.
    pub fn oracle_tp_reset(present: bool, nv12: bool, p010: bool, answer: bool, writes: bool);
    /// 0 or 1 for a device export's token, -1 for anything else.
    pub fn oracle_tp_token(ptr: *const c_void) -> i32;
    /// Whether the field adapter returns the real private fields.
    pub fn oracle_tp_adapter_layout() -> bool;
}
