//! Hidden release ABI over the unchanged C weak-control layout.
use super::service_weak_acquire::WeakService;
use crate::service_weak_release;
use obs_util::ffi::darray::bfree;

/// # Safety
/// `weak` is live, aligned and retained throughout this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_weak_service_strong_release(weak: *mut WeakService) -> bool {
    // SAFETY: the caller retains the control; only its atomic count is accessed.
    service_weak_release::strong_release(unsafe { &(*weak).counts.refs })
}

/// # Safety
/// `weak` is live, aligned and retained throughout this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_weak_service_weak_addref(weak: *mut WeakService) {
    // SAFETY: the caller retains the control; only its atomic count is accessed.
    service_weak_release::weak_addref(unsafe { &(*weak).counts.weak_refs });
}

/// # Safety
/// `weak` is live, aligned and retained throughout this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_weak_service_weak_release(weak: *mut WeakService) -> bool {
    // SAFETY: the caller retains the control; only its atomic count is accessed.
    service_weak_release::weak_release(unsafe { &(*weak).counts.weak_refs })
}

/// # Safety
/// `weak` is NULL or a bmalloc-family allocation with no remaining users.
/// It must not be used again after this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_weak_service_free(weak: *mut WeakService) {
    // SAFETY: the caller transfers the allocation back to its owning allocator.
    unsafe { bfree(weak.cast()) };
}
