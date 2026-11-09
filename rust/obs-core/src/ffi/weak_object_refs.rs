//! Null-safe adapters over the private C control and its atomic counts.
use crate::weak_object_refs;
use core::ffi::c_void;
use obs_util::ffi::darray::bfree;
use obs_util::platform::CLongAtomic;

#[repr(C)]
pub struct Fields {
    pub refs: *const CLongAtomic,
    pub weak_refs: *const CLongAtomic,
    pub object: *mut c_void,
}

unsafe extern "C" {
    fn obs_weak_object_refs_fields(weak: *mut c_void) -> Fields;
    fn obs_object_refs_control(object: *mut c_void) -> *mut c_void;
}

/// # Safety
/// Non-null weak handles must be live and retained throughout this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_weak_object_addref(weak: *mut c_void) {
    let count = if weak.is_null() {
        None
    } else {
        // SAFETY: NULL rejected before the accessor; caller retains the allocation.
        Some(unsafe { &*obs_weak_object_refs_fields(weak).weak_refs })
    };
    weak_object_refs::addref(count);
}

/// # Safety
/// Transfers one weak reference; the final release invalidates the handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_weak_object_release(weak: *mut c_void) {
    let count = if weak.is_null() {
        None
    } else {
        // SAFETY: NULL rejected before the accessor; caller owns one weak reference.
        Some(unsafe { &*obs_weak_object_refs_fields(weak).weak_refs })
    };
    if weak_object_refs::release(count) {
        // SAFETY: the final owner returns the C allocation to its original allocator.
        unsafe { bfree(weak) };
    }
}

/// # Safety
/// A non-null object and its control must be live for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_object_get_weak_object(object: *mut c_void) -> *mut c_void {
    let control = if object.is_null() {
        None
    } else {
        // SAFETY: NULL rejected before reading the private object field.
        let weak = unsafe { obs_object_refs_control(object) };
        if weak.is_null() {
            None
        } else {
            // SAFETY: control is non-null and retained by the caller's live object.
            Some((weak, unsafe {
                &*obs_weak_object_refs_fields(weak).weak_refs
            }))
        }
    };
    weak_object_refs::get_weak(control).unwrap_or(core::ptr::null_mut())
}

/// # Safety
/// Non-null weak handles must be live and retained for the call. A successful
/// promotion transfers a strong reference to the caller.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_weak_object_get_object(weak: *mut c_void) -> *mut c_void {
    let fields = if weak.is_null() {
        None
    } else {
        // SAFETY: NULL rejected before the accessor; weak ownership keeps fields alive.
        let fields = unsafe { obs_weak_object_refs_fields(weak) };
        // SAFETY: C's count has CLongAtomic's alignment and is accessed atomically.
        Some((unsafe { &*fields.refs }, fields.object))
    };
    weak_object_refs::get_object(fields).unwrap_or(core::ptr::null_mut())
}
