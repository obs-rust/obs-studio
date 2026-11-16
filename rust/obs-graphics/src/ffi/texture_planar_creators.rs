//! C ABI for `libobs/graphics/texture-planar-creators.c.h`:
//! `gs_texture_create_nv12` and `gs_texture_create_p010`.
//!
//! `struct graphics_subsystem` stays private to C: the context, its device
//! and its two planar device exports come from the field adapter in
//! `libobs/graphics/graphics-linear-srgb-fields.c.h`, which also logs the
//! three lines with the C's literal formats. The planes go through the
//! public `gs_texture_create` and `gs_texture_destroy`, which stay C:
//! libobs links them, and cargo tests get the oracle crate's recording
//! stand-in (`oracle/texture_render_device.c`).
//!
//! The out-pointers are not checked, as in the C: the fallback writes
//! through them whatever they are.

use core::ffi::{c_char, c_int, c_void};
use core::ptr::{self, NonNull};

use crate::texture_planar_creators::{Host, Line, Planar, Plane, create};
use crate::texture_render::ColorFormat;

/// `device_texture_create_nv12` and `device_texture_create_p010` in
/// `struct gs_exports`.
type DeviceCreate = unsafe extern "C" fn(
    device: *mut c_void,
    tex_y: *mut *mut c_void,
    tex_uv: *mut *mut c_void,
    width: u32,
    height: u32,
    flags: u32,
) -> bool;

unsafe extern "C" {
    fn obs_texture_planar_context() -> *mut c_void;
    fn obs_texture_planar_log(which: c_int, function: *const c_char);
    fn obs_texture_planar_device(graphics: *mut c_void) -> *mut c_void;
    fn obs_texture_planar_nv12_field(graphics: *mut c_void) -> Option<DeviceCreate>;
    fn obs_texture_planar_p010_field(graphics: *mut c_void) -> Option<DeviceCreate>;

    fn gs_texture_create(
        width: u32,
        height: u32,
        color_format: ColorFormat,
        levels: u32,
        data: *const *const u8,
        flags: u32,
    ) -> *mut c_void;
    fn gs_texture_destroy(tex: *mut c_void);
}

/// `obs_texture_planar_log`'s `which`.
const LOG_NO_CONTEXT: c_int = 0;
const LOG_NV12_ODD_SIZE: c_int = 1;
const LOG_P010_ODD_SIZE: c_int = 2;

/// One call's context and its two out-pointers.
struct Libobs {
    graphics: *mut c_void,
    tex_y: *mut *mut c_void,
    tex_uv: *mut *mut c_void,
}

impl Libobs {
    fn slot(&self, plane: Plane) -> *mut *mut c_void {
        match plane {
            Plane::Y => self.tex_y,
            Plane::Uv => self.tex_uv,
        }
    }
}

impl Host for Libobs {
    type Texture = NonNull<c_void>;

    fn has_context(&mut self) -> bool {
        // SAFETY: reads the thread's graphics context, nothing else.
        self.graphics = unsafe { obs_texture_planar_context() };
        !self.graphics.is_null()
    }

    fn log(&mut self, line: Line) {
        let (which, function) = match line {
            Line::NoContext(Planar::Nv12) => (LOG_NO_CONTEXT, c"gs_texture_create_nv12".as_ptr()),
            Line::NoContext(Planar::P010) => (LOG_NO_CONTEXT, c"gs_texture_create_p010".as_ptr()),
            Line::OddSize(Planar::Nv12) => (LOG_NV12_ODD_SIZE, ptr::null()),
            Line::OddSize(Planar::P010) => (LOG_P010_ODD_SIZE, ptr::null()),
        };
        // SAFETY: `function` is a static string or unused.
        unsafe { obs_texture_planar_log(which, function) };
    }

    fn device_create(
        &mut self,
        planar: Planar,
        width: u32,
        height: u32,
        flags: u32,
    ) -> Option<bool> {
        // SAFETY: `graphics` is the live context `has_context` returned;
        // the export and the device are its own, called as the C calls
        // them with the caller's out-pointers.
        unsafe {
            let export = match planar {
                Planar::Nv12 => obs_texture_planar_nv12_field(self.graphics),
                Planar::P010 => obs_texture_planar_p010_field(self.graphics),
            }?;
            let device = obs_texture_planar_device(self.graphics);
            Some(export(
                device,
                self.tex_y,
                self.tex_uv,
                width,
                height,
                flags,
            ))
        }
    }

    fn texture_create(
        &mut self,
        width: u32,
        height: u32,
        format: ColorFormat,
        flags: u32,
    ) -> Option<NonNull<c_void>> {
        // SAFETY: one level and no data, as the C passes.
        NonNull::new(unsafe { gs_texture_create(width, height, format, 1, ptr::null(), flags) })
    }

    fn texture_destroy(&mut self, texture: NonNull<c_void>) {
        // SAFETY: a texture `texture_create` made in this call.
        unsafe { gs_texture_destroy(texture.as_ptr()) };
    }

    fn store(&mut self, plane: Plane, texture: Option<NonNull<c_void>>) {
        // SAFETY: the caller's out-pointer, written as the C writes it.
        unsafe { *self.slot(plane) = texture.map_or(ptr::null_mut(), NonNull::as_ptr) };
    }

    fn load(&mut self, plane: Plane) -> Option<NonNull<c_void>> {
        // SAFETY: the out-pointer `store` just wrote through.
        NonNull::new(unsafe { *self.slot(plane) })
    }
}

/// # Safety
/// `tex_y` and `tex_uv` must be valid for writes, and a device export
/// called with them must be safe, as for the C.
unsafe fn planar(
    planar: Planar,
    tex_y: *mut *mut c_void,
    tex_uv: *mut *mut c_void,
    width: u32,
    height: u32,
    flags: u32,
) -> bool {
    let mut host = Libobs {
        graphics: ptr::null_mut(),
        tex_y,
        tex_uv,
    };
    create(&mut host, planar, width, height, flags)
}

/// `gs_texture_create_nv12()`: a `GS_R8` luma and a half-size `GS_R8G8`
/// chroma texture, from the device's own creator when it has one.
///
/// # Safety
/// `tex_y` and `tex_uv` must be valid for writes whenever the call gets
/// past its checks; they are not checked, as in the C.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_texture_create_nv12(
    tex_y: *mut *mut c_void,
    tex_uv: *mut *mut c_void,
    width: u32,
    height: u32,
    flags: u32,
) -> bool {
    // SAFETY: the caller's contract, passed on.
    unsafe { planar(Planar::Nv12, tex_y, tex_uv, width, height, flags) }
}

/// `gs_texture_create_p010()`: a `GS_R16` luma and a half-size `GS_RG16`
/// chroma texture, from the device's own creator when it has one.
///
/// # Safety
/// As for [`gs_texture_create_nv12`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_texture_create_p010(
    tex_y: *mut *mut c_void,
    tex_uv: *mut *mut c_void,
    width: u32,
    height: u32,
    flags: u32,
) -> bool {
    // SAFETY: the caller's contract, passed on.
    unsafe { planar(Planar::P010, tex_y, tex_uv, width, height, flags) }
}
