//! C ABI for the canvas strong release over the unchanged private layout.
use crate::ffi::{
    canvas_ref::{WeakCanvas, obs_weak_canvas_release},
    core_initialized::obs_initialized,
};
use crate::{canvas_release, object_ref::Host};
use core::ffi::{c_char, c_int, c_void};
use obs_util::platform::CLongAtomic;

unsafe extern "C" {
    fn obs_canvas_ref_control(canvas: *const c_void) -> *mut WeakCanvas;
    fn obs_canvas_destroy(canvas: *mut c_void);
    fn blog(level: c_int, format: *const c_char, ...);
}

struct Canvas(*mut c_void);

impl Host for Canvas {
    type Control = *mut WeakCanvas;
    fn initialized(&self) -> bool {
        // SAFETY: callers synchronize core startup and shutdown, as in C.
        unsafe { obs_initialized() }
    }
    fn warn_shutdown(&mut self) {
        // SAFETY: static NUL-terminated format with no arguments, LOG_WARNING.
        unsafe {
            blog(
                200,
                c"Tried to release a canvas when the OBS core is shut down!".as_ptr(),
            )
        };
    }
    fn control(&self) -> Option<Self::Control> {
        if self.0.is_null() {
            return None;
        }
        // SAFETY: NULL was rejected; the caller owns a strong reference.
        Some(unsafe { obs_canvas_ref_control(self.0) })
    }
    fn strong(&self, control: Self::Control) -> &CLongAtomic {
        // SAFETY: a strong owner retains its control throughout the decrement.
        // This borrow ends before destroy can free the canvas context.
        unsafe { &(*control).counts.refs }
    }
    fn destroy(&mut self) {
        // SAFETY: the last strong owner destroys the canvas exactly once.
        // It frees self.0, which is never read again afterwards.
        unsafe { obs_canvas_destroy(self.0) };
    }
    fn release_weak(&mut self, control: Self::Control) {
        // SAFETY: the canvas's own weak reference retained control across
        // destroy; this releases exactly that reference, as C does.
        unsafe { obs_weak_canvas_release(control) };
    }
}

/// # Safety
/// A non-NULL canvas must own a strong reference while the core is
/// initialized; after shutdown it is never read. The final release destroys
/// the canvas, and callers synchronize core shutdown.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_canvas_release(canvas: *mut c_void) {
    canvas_release::release(!canvas.is_null(), &mut Canvas(canvas));
}
