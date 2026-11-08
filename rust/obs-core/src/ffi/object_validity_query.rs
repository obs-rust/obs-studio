//! Borrow the context data slot only after validating the object pointer.
use core::ffi::c_void;
use core::ptr::NonNull;

unsafe extern "C" {
    fn obs_object_validity_field(obj: *const c_void) -> *const *mut c_void;
}

/// Query the original context validity contract without retaining or mutating it.
///
/// # Safety
/// A non-null object must begin with a live, readable obs_context_data, under
/// the same synchronization and lifetime rules as the original C query.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_obj_invalid(obj: *mut c_void) -> bool {
    let data = NonNull::new(obj).map(|obj| {
        // SAFETY: the object is non-null and its context data slot is readable.
        unsafe { *obs_object_validity_field(obj.as_ptr()) }
    });
    crate::object_validity_query::is_invalid(data)
}
