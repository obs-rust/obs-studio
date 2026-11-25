//! `obs_weak_source_addref` / `obs_weak_source_release` over the C-owned
//! control: Rust borrows only its weak count, through a private C adapter,
//! and returns the control to its original allocator on the final release.
use crate::weak_refs;
use core::ffi::c_void;
use obs_util::ffi::bmem::bfree;
use obs_util::platform::CLongAtomic;

unsafe extern "C" {
    fn obs_weak_source_weak_refs_field(weak: *mut c_void) -> *const CLongAtomic;
}

/// # Safety
/// A non-null handle must be a live control the caller holds a weak
/// reference on; NULL is rejected before the adapter.
unsafe fn weak_count<'a>(weak: *mut c_void) -> Option<&'a CLongAtomic> {
    if weak.is_null() {
        return None;
    }
    // SAFETY: NULL rejected first; the caller's weak reference keeps the
    // control, and so its aligned native long, alive. C accesses it atomically.
    Some(unsafe { &*obs_weak_source_weak_refs_field(weak) })
}

/// # Safety
/// A non-null handle must be a live source control the caller holds a weak
/// reference on.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_weak_source_addref(weak: *mut c_void) {
    // SAFETY: inherited weak-handle contract.
    weak_refs::addref(unsafe { weak_count(weak) });
}

/// # Safety
/// Transfers one weak reference; the final release frees the handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_weak_source_release(weak: *mut c_void) {
    // SAFETY: inherited weak-handle contract.
    if weak_refs::release(unsafe { weak_count(weak) }) {
        // SAFETY: the last weak owner returns the bzalloc'd control to bfree.
        unsafe { bfree(weak) };
    }
}
