//! Private layout access and existing canvas ownership/signal operations.
use crate::canvas_remove::{self, Host};
use core::ffi::c_void;

unsafe extern "C" {
    fn obs_canvas_remove_set_flags(canvas: *mut c_void, flags: u32);
}

struct CanvasHost;
impl Host for CanvasHost {
    type Canvas = *mut c_void;
    fn flags(&mut self, canvas: Self::Canvas) -> u32 {
        // SAFETY: the entry point borrows a non-null readable canvas; after
        // promotion, the reference is held until release after the callbacks.
        unsafe { super::canvas_flags_getter::obs_canvas_get_flags(canvas) }
    }
    fn retain(&mut self, canvas: Self::Canvas) -> Option<Self::Canvas> {
        // SAFETY: the caller keeps the canvas/control valid for promotion.
        let retained = unsafe { super::canvas_ref::obs_canvas_get_ref(canvas) };
        (!retained.is_null()).then_some(retained)
    }
    fn set_flags(&mut self, canvas: Self::Canvas, flags: u32) {
        // SAFETY: the promoted canvas is writable under the original caller's
        // synchronization contract. No field borrow survives this call.
        unsafe { obs_canvas_remove_set_flags(canvas, flags) };
    }
    fn signal(&mut self, canvas: Self::Canvas) {
        // SAFETY: hold the promoted canvas across both synchronous signals.
        unsafe {
            super::canvas_signals::canvas_dosignal(
                canvas,
                c"canvas_remove".as_ptr(),
                c"remove".as_ptr(),
            )
        };
    }
    fn release(&mut self, canvas: Self::Canvas) {
        // SAFETY: release exactly the promotion owned by this invocation.
        unsafe { super::canvas_release::obs_canvas_release(canvas) };
    }
}

/// # Safety
/// `canvas` is non-null and readable, including its weak control, for the
/// promotion attempt. The core is initialized. Callers provide the same
/// synchronization as C for flags and synchronous callbacks.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_canvas_remove(canvas: *mut c_void) {
    canvas_remove::remove(&mut CanvasHost, canvas);
}
