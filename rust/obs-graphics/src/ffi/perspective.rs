//! The C ABI of `gs_perspective`.
//!
//! The context read, the debug line and the device call are one-line C
//! adapters (`libobs/graphics/perspective-fields.c.h`), which keep the
//! private graphics layout and the device ABI in C. The call reads this
//! thread's context once and makes at most one device call.
use crate::perspective::{self, Frustum, Host};
use std::ffi::{CStr, c_char, c_void};

unsafe extern "C" {
    fn obs_graphics_perspective_context() -> *mut c_void;
    fn obs_graphics_perspective_debug(function: *const c_char);
    fn obs_graphics_perspective_frustum(
        graphics: *mut c_void,
        left: f32,
        right: f32,
        top: f32,
        bottom: f32,
        znear: f32,
        zfar: f32,
    );
}

/// This thread's graphics context, as read at the start of the call.
struct Context(*mut c_void);

impl Host for Context {
    fn has_context(&self) -> bool {
        !self.0.is_null()
    }
    fn debug(&mut self, function: &'static CStr) {
        // SAFETY: a static NUL-terminated name, read during the call only.
        unsafe { obs_graphics_perspective_debug(function.as_ptr()) }
    }
    fn frustum(&mut self, f: Frustum) {
        // SAFETY: the core calls this only after `has_context` saw a non-NULL
        // context, current on this thread for the call; the device and its
        // exports belong to the backend.
        unsafe {
            obs_graphics_perspective_frustum(
                self.0, f.left, f.right, f.top, f.bottom, f.znear, f.zfar,
            );
        }
    }
}

/// Hand the current device the perspective frustum of `angle` degrees
/// (vertical), `aspect`, `near` and `far`.
/// # Safety
/// An active context must hold a live device and its matching export table.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_perspective(angle: f32, aspect: f32, near: f32, far: f32) {
    // SAFETY: reads the calling thread's context pointer; nothing is retained.
    let mut context = Context(unsafe { obs_graphics_perspective_context() });
    perspective::perspective(&mut context, angle, aspect, near, far);
}
