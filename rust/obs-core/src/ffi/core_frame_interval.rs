//! Convert private field addresses to the safe core's optional borrowed scalar.
use crate::core_frame_interval::interval;
use std::ffi::c_void;
use std::ptr::NonNull;

unsafe extern "C" {
    fn obs_core_frame_interval_core() -> *const c_void;
    fn obs_core_frame_interval_field(core: *const c_void) -> *const u64;
}

/// Return the stored video frame interval in nanoseconds.
///
/// # Safety
/// Startup, shutdown and video reconfiguration must not race this read.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_frame_interval_ns() -> u64 {
    // SAFETY: obtaining the global pointer does not dereference it.
    let core = unsafe { obs_core_frame_interval_core() };
    let field = NonNull::new(core.cast_mut()).map(|core| {
        // SAFETY: null is rejected before the C accessor forms a member address.
        // The caller keeps this core and its scalar readable for the call.
        unsafe { &*obs_core_frame_interval_field(core.as_ptr()) }
    });
    interval(field)
}
