//! C owns the canvas/view layout and the existing lock/reference implementation.
use core::ffi::c_void;
use core::ptr::NonNull;

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
    let canvas = NonNull::new(canvas);
    if canvas.is_none() {
        // Preserve the observed C precondition failure without forming a field
        // address from NULL (which is itself undefined behavior in C).
        // SAFETY: raise accepts SIGSEGV; no field accessor is called.
        unsafe { libc::raise(libc::SIGSEGV) };
        return core::ptr::null_mut();
    }
    crate::canvas_channel_getter::get_channel(canvas, channel, |canvas, channel| {
        // SAFETY: a present canvas is live; C borrows its embedded view.
        let fields = unsafe { obs_canvas_channel_fields(canvas.as_ptr()) };
        // SAFETY: C supplies the matching view operation; it owns bounds, lock and retain.
        NonNull::new(unsafe { (fields.get_source)(fields.view, channel) })
    })
    .map_or(core::ptr::null_mut(), NonNull::as_ptr)
}
