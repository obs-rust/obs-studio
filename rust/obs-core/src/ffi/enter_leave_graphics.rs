//! `obs_enter_graphics()` and `obs_leave_graphics()` over C-owned state: the
//! core's graphics subsystem and the graphics-layer calls, behind the private
//! adapters of `libobs/obs-enter-leave-graphics-fields.c.h`. When each is
//! called is the safe core's ([`crate::enter_leave_graphics`]).
use core::ffi::c_void;
use core::ptr::NonNull;

/// `graphics_t`, opaque here.
pub type GraphicsT = c_void;

unsafe extern "C" {
    fn obs_enter_leave_graphics_get() -> *mut GraphicsT;
    fn obs_enter_leave_graphics_enter(graphics: *mut GraphicsT);
    fn obs_enter_leave_graphics_leave();
}

struct Core;

impl crate::enter_leave_graphics::Graphics for Core {
    type Context = NonNull<GraphicsT>;
    fn graphics(&mut self) -> Option<NonNull<GraphicsT>> {
        // SAFETY: one read of the live core's graphics field.
        NonNull::new(unsafe { obs_enter_leave_graphics_get() })
    }
    fn enter_context(&mut self, graphics: NonNull<GraphicsT>) {
        // SAFETY: the core's own non-NULL graphics, just read.
        unsafe { obs_enter_leave_graphics_enter(graphics.as_ptr()) }
    }
    fn leave_context(&mut self) {
        // SAFETY: the graphics layer checks the calling thread's context
        // itself and reports a leave from outside one.
        unsafe { obs_enter_leave_graphics_leave() }
    }
}

/// Enters the core's graphics context on the calling thread, when the core
/// has a graphics subsystem.
///
/// # Safety
/// As for the C function: the core is initialized and its graphics, when
/// set, is live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_enter_graphics() {
    crate::enter_leave_graphics::enter_graphics(&mut Core);
}

/// Leaves the calling thread's graphics context, when the core has a
/// graphics subsystem.
///
/// # Safety
/// As for [`obs_enter_graphics`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_leave_graphics() {
    crate::enter_leave_graphics::leave_graphics(&mut Core);
}
