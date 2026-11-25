//! Opaque encoder/control projections; reference-count decisions live in the safe core.
use core::ffi::c_void;
use obs_util::ffi::darray::bfree;
use obs_util::platform::CLongAtomic;

unsafe extern "C" {
    fn obs_encoder_weak_refs_count(weak: *mut c_void) -> *const CLongAtomic;
    fn obs_encoder_weak_refs_control(encoder: *mut c_void) -> *mut c_void;
}

/// # Safety
/// A non-null control must remain live and retained throughout the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_weak_encoder_addref(weak: *mut c_void) {
    let count = if weak.is_null() {
        None
    } else {
        // SAFETY: caller retains the non-null allocation; C returns its aligned atomic.
        Some(unsafe { &*obs_encoder_weak_refs_count(weak) })
    };
    crate::weak_refs::addref(count);
}

/// # Safety
/// Transfers one retained weak reference; the final release invalidates the control.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_weak_encoder_release(weak: *mut c_void) {
    let count = if weak.is_null() {
        None
    } else {
        // SAFETY: caller owns the reference being released, so the count is live.
        Some(unsafe { &*obs_encoder_weak_refs_count(weak) })
    };
    if crate::weak_refs::release(count) {
        // SAFETY: the safe core identified the final owner of this bmalloc allocation.
        unsafe { bfree(weak) };
    }
}

/// # Safety
/// A non-null encoder and its stored control must be live throughout the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_get_weak_encoder(encoder: *mut c_void) -> *mut c_void {
    let control = if encoder.is_null() {
        None
    } else {
        // SAFETY: the caller retains the non-null encoder during the projection.
        let weak = unsafe { obs_encoder_weak_refs_control(encoder) };
        if weak.is_null() {
            None
        } else {
            // SAFETY: the live encoder retains the control and its aligned native-long count.
            Some((weak, unsafe { &*obs_encoder_weak_refs_count(weak) }))
        }
    };
    crate::weak_refs::get_weak(control).unwrap_or(core::ptr::null_mut())
}
