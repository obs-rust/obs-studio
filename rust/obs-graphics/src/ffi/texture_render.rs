//! C ABI for `libobs/graphics/texture-render.c.h`.
//!
//! `struct gs_texture_render` is private to that file, so callers only hold
//! the pointer. It is a `bmalloc` block, so `bnum_allocs` counts the same
//! object the C does. The graphics calls are the public `gs_*` functions in
//! `graphics.c.h`, which stays C: libobs links them, and cargo tests get a
//! recording stand-in from the oracle crate (`oracle/texture_render_device.c`).
//!
//! A null texrender returns as in the C, except in `gs_texrender_get_format`,
//! which the C does not check either.

use core::ffi::{c_int, c_void};
use core::ptr::{self, NonNull};

use obs_util::ffi::bmem::{bfree, bmalloc};

use crate::texture_render::{ColorFormat, ColorSpace, Device, TexRender, ZStencilFormat};

unsafe extern "C" {
    fn gs_texture_create(
        width: u32,
        height: u32,
        color_format: ColorFormat,
        levels: u32,
        data: *const *const u8,
        flags: u32,
    ) -> *mut c_void;
    fn gs_zstencil_create(width: u32, height: u32, format: ZStencilFormat) -> *mut c_void;
    fn gs_texture_destroy(tex: *mut c_void);
    fn gs_zstencil_destroy(zstencil: *mut c_void);
    fn gs_viewport_push();
    fn gs_viewport_pop();
    fn gs_projection_push();
    fn gs_projection_pop();
    fn gs_matrix_push();
    fn gs_matrix_pop();
    fn gs_matrix_identity();
    fn gs_get_render_target() -> *mut c_void;
    fn gs_get_zstencil_target() -> *mut c_void;
    fn gs_get_color_space() -> ColorSpace;
    fn gs_set_render_target_with_color_space(
        tex: *mut c_void,
        zstencil: *mut c_void,
        space: ColorSpace,
    );
    fn gs_set_viewport(x: c_int, y: c_int, width: c_int, height: c_int);
}

fn raw(handle: Option<NonNull<c_void>>) -> *mut c_void {
    handle.map_or(ptr::null_mut(), NonNull::as_ptr)
}

/// The thread's current graphics context, through the public `gs_*` API.
/// Each function checks for a context itself, as it does for the C.
pub struct GsDevice;

impl Device for GsDevice {
    type Texture = NonNull<c_void>;
    type ZStencil = NonNull<c_void>;

    fn texture_create(
        &mut self,
        cx: u32,
        cy: u32,
        format: ColorFormat,
        levels: u32,
        flags: u32,
    ) -> Option<Self::Texture> {
        // SAFETY: plain values and no initial data, as the C passes.
        NonNull::new(unsafe { gs_texture_create(cx, cy, format, levels, ptr::null(), flags) })
    }
    fn zstencil_create(
        &mut self,
        cx: u32,
        cy: u32,
        format: ZStencilFormat,
    ) -> Option<Self::ZStencil> {
        // SAFETY: plain values.
        NonNull::new(unsafe { gs_zstencil_create(cx, cy, format) })
    }
    fn texture_destroy(&mut self, texture: Option<Self::Texture>) {
        // SAFETY: null, or the texture this texrender created and still owns.
        unsafe { gs_texture_destroy(raw(texture)) }
    }
    fn zstencil_destroy(&mut self, zstencil: Option<Self::ZStencil>) {
        // SAFETY: null, or the zstencil this texrender created and still owns.
        unsafe { gs_zstencil_destroy(raw(zstencil)) }
    }
    fn viewport_push(&mut self) {
        // SAFETY: no arguments.
        unsafe { gs_viewport_push() }
    }
    fn viewport_pop(&mut self) {
        // SAFETY: no arguments.
        unsafe { gs_viewport_pop() }
    }
    fn projection_push(&mut self) {
        // SAFETY: no arguments.
        unsafe { gs_projection_push() }
    }
    fn projection_pop(&mut self) {
        // SAFETY: no arguments.
        unsafe { gs_projection_pop() }
    }
    fn matrix_push(&mut self) {
        // SAFETY: no arguments.
        unsafe { gs_matrix_push() }
    }
    fn matrix_pop(&mut self) {
        // SAFETY: no arguments.
        unsafe { gs_matrix_pop() }
    }
    fn matrix_identity(&mut self) {
        // SAFETY: no arguments.
        unsafe { gs_matrix_identity() }
    }
    fn render_target(&mut self) -> Option<Self::Texture> {
        // SAFETY: no arguments.
        NonNull::new(unsafe { gs_get_render_target() })
    }
    fn zstencil_target(&mut self) -> Option<Self::ZStencil> {
        // SAFETY: no arguments.
        NonNull::new(unsafe { gs_get_zstencil_target() })
    }
    fn color_space(&mut self) -> ColorSpace {
        // SAFETY: no arguments.
        unsafe { gs_get_color_space() }
    }
    fn set_render_target(
        &mut self,
        texture: Option<Self::Texture>,
        zstencil: Option<Self::ZStencil>,
        space: ColorSpace,
    ) {
        // SAFETY: handles the device returned or this texrender created,
        // passed back as the C passes them.
        unsafe { gs_set_render_target_with_color_space(raw(texture), raw(zstencil), space) }
    }
    fn set_viewport(&mut self, x: i32, y: i32, cx: i32, cy: i32) {
        // SAFETY: plain values.
        unsafe { gs_set_viewport(x, y, cx, cy) }
    }
}

/// The C `gs_texrender_t`.
#[allow(non_camel_case_types)]
pub type gs_texrender_t = TexRender<GsDevice>;

/// # Safety
/// Same contract as C `gs_texrender_create`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_texrender_create(
    format: ColorFormat,
    zsformat: ZStencilFormat,
) -> *mut gs_texrender_t {
    // SAFETY: one block the size of the state. bmalloc aligns to 32 bytes,
    // more than the state needs, and does not return null.
    let texrender = unsafe { bmalloc(size_of::<gs_texrender_t>()) }.cast::<gs_texrender_t>();
    // SAFETY: the block is fresh, writable and large enough.
    unsafe { texrender.write(TexRender::new(format, zsformat)) };
    texrender
}

/// # Safety
/// Same contract as C `gs_texrender_destroy`: null, or a live texrender.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_texrender_destroy(texrender: *mut gs_texrender_t) {
    if texrender.is_null() {
        return;
    }
    // SAFETY: a live texrender from gs_texrender_create, read out once and
    // then freed, so nothing reads it again.
    unsafe {
        texrender.read().destroy(&mut GsDevice);
        bfree(texrender.cast());
    }
}

/// # Safety
/// Same contract as C `gs_texrender_begin`: null, or a live texrender.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_texrender_begin(
    texrender: *mut gs_texrender_t,
    cx: u32,
    cy: u32,
) -> bool {
    // SAFETY: null or a live texrender, borrowed for this call.
    unsafe { texrender.as_mut() }.is_some_and(|t| t.begin(&mut GsDevice, cx, cy))
}

/// # Safety
/// Same contract as C `gs_texrender_begin_with_color_space`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_texrender_begin_with_color_space(
    texrender: *mut gs_texrender_t,
    cx: u32,
    cy: u32,
    space: ColorSpace,
) -> bool {
    // SAFETY: null or a live texrender, borrowed for this call.
    unsafe { texrender.as_mut() }
        .is_some_and(|t| t.begin_with_color_space(&mut GsDevice, cx, cy, space))
}

/// # Safety
/// Same contract as C `gs_texrender_end`: null, or a live texrender.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_texrender_end(texrender: *mut gs_texrender_t) {
    // SAFETY: null or a live texrender, borrowed for this call.
    if let Some(t) = unsafe { texrender.as_mut() } {
        t.end(&mut GsDevice);
    }
}

/// # Safety
/// Same contract as C `gs_texrender_reset`: null, or a live texrender.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_texrender_reset(texrender: *mut gs_texrender_t) {
    // SAFETY: null or a live texrender, borrowed for this call.
    if let Some(t) = unsafe { texrender.as_mut() } {
        t.reset();
    }
}

/// # Safety
/// Same contract as C `gs_texrender_get_texture`: null, or a live texrender.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_texrender_get_texture(texrender: *const gs_texrender_t) -> *mut c_void {
    // SAFETY: null or a live texrender, read for this call.
    raw(unsafe { texrender.as_ref() }.and_then(TexRender::texture))
}

/// # Safety
/// Same contract as C `gs_texrender_get_format`: a live texrender. Null is
/// not checked, as in the C.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_texrender_get_format(texrender: *const gs_texrender_t) -> ColorFormat {
    // SAFETY: the caller passes a live texrender.
    unsafe { (*texrender).format() }
}
