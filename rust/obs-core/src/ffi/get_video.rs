//! Thin C ABI shim for the main canvas's borrowed video output.
use core::ffi::c_void;
use core::ptr::{self, NonNull};

/// Private adapter layout measured against the real C declaration: the
/// current main-canvas field and the two borrowed field reads.
#[repr(C)]
pub struct Fields {
    pub canvas: *const c_void,
    pub mix: unsafe extern "C" fn(*const c_void) -> *const c_void,
    pub video: unsafe extern "C" fn(*const c_void) -> *mut c_void,
}

unsafe extern "C" {
    fn obs_get_video_fields() -> Fields;
}

/// # Safety
/// The core must be initialized. Its main canvas is NULL or live, and its mix
/// is NULL or live, under the original getter's synchronization. The result
/// is borrowed from the mix and must not be closed by the caller; a video
/// reset or shutdown may invalidate it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_video() -> *mut c_void {
    // SAFETY: the caller preserves the original initialized-core precondition;
    // the adapter borrows the current field without acquiring ownership.
    let fields = unsafe { obs_get_video_fields() };
    crate::get_video::get_video(
        NonNull::new(fields.canvas.cast_mut()),
        // SAFETY: a non-NULL main canvas is live under the caller's contract.
        |canvas| NonNull::new(unsafe { (fields.mix)(canvas.as_ptr()) }.cast_mut()),
        // SAFETY: a non-NULL mix is live under the caller's contract.
        |mix| NonNull::new(unsafe { (fields.video)(mix.as_ptr()) }),
    )
    .map_or(ptr::null_mut(), NonNull::as_ptr)
}
