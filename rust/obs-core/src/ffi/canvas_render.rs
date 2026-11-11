//! Canvas rendering borrows the embedded view through the existing C adapter.
use super::canvas_channel_getter::Fields;
use core::ffi::c_void;

unsafe extern "C" {
    fn obs_canvas_channel_fields(canvas: *mut c_void) -> Fields;
}

/// # Safety
/// `canvas` must be non-null and live; called on the graphics thread under the
/// original C synchronization rules. NULL is outside the original C domain.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_canvas_render(canvas: *mut c_void) {
    if canvas.is_null() {
        // Match the characterized precondition failure without forming a member
        // address through NULL in the C field adapter.
        // SAFETY: raise accepts SIGSEGV; no canvas accessor is called.
        unsafe { libc::raise(libc::SIGSEGV) };
        return;
    }
    crate::canvas_render::render(
        canvas,
        |canvas| {
            // SAFETY: the caller supplies a live canvas; C owns its private layout.
            unsafe { obs_canvas_channel_fields(canvas).view }
        },
        |view| {
            // SAFETY: the adapter borrows this live canvas's embedded view.
            unsafe { super::view::obs_view_render(view) };
        },
    );
}
