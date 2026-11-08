//! C owns the canvas/view layout and the existing lock/reference implementation.
use core::ffi::c_void;

/// Private C view and its unchanged source acquisition operation.
#[repr(C)]
pub struct Fields {
    pub view: *mut c_void,
    pub get_source: unsafe extern "C" fn(*mut c_void, u32) -> *mut c_void,
}

unsafe extern "C" {
    fn obs_canvas_channel_fields(canvas: *mut c_void) -> Fields;
}

/// # Safety
/// `canvas` must be non-null and live under the original C synchronization rules.
/// A non-null result is a new source reference which the caller must release.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_canvas_get_channel(canvas: *mut c_void, channel: u32) -> *mut c_void {
    // SAFETY: caller supplies a live non-null canvas; C borrows its embedded view.
    let fields = unsafe { obs_canvas_channel_fields(canvas) };
    crate::canvas_channel_getter::get_channel(fields.view, channel, |view, channel| {
        // SAFETY: C supplies the matching view operation; it owns bounds, lock and retain.
        unsafe { (fields.get_source)(view, channel) }
    })
}
