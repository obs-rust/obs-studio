//! The public entry over the existing flags getter, `obs_video_active` and the
//! C-owned internal reset (graphics, mix and signals stay in C).
use crate::canvas_reset_video::{self, Host};
use core::ffi::c_void;

unsafe extern "C" {
    fn obs_canvas_reset_video_internal(canvas: *mut c_void, ovi: *mut c_void) -> bool;
}

struct CanvasHost;
impl Host for CanvasHost {
    type Canvas = *mut c_void;
    type Info = *mut c_void;
    fn flags(&mut self, canvas: Self::Canvas) -> u32 {
        // SAFETY: the caller passes a live canvas once `ovi` is non-null, as C requires.
        unsafe { super::canvas_flags_getter::obs_canvas_get_flags(canvas) }
    }
    fn video_active(&mut self) -> bool {
        // SAFETY: the core is initialized, as the original entry requires.
        unsafe { super::video_active::obs_video_active() }
    }
    fn reset_internal(&mut self, canvas: Self::Canvas, info: Self::Info) -> bool {
        // SAFETY: the same live canvas and non-null caller-owned info the C entry passed.
        unsafe { obs_canvas_reset_video_internal(canvas, info) }
    }
}

/// # Safety
/// As for the C function: `ovi` is NULL or a valid video info; when it is
/// non-null, `canvas` is a live canvas and the core is initialized.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_canvas_reset_video(canvas: *mut c_void, ovi: *mut c_void) -> bool {
    canvas_reset_video::reset_video(&mut CanvasHost, canvas, (!ovi.is_null()).then_some(ovi))
}
