//! Read fields through C's private context layout, then delegate to the safe core.
use core::ffi::{c_int, c_void};
use core::ptr::NonNull;

/// Field addresses borrowed from the real C context.
#[repr(C)]
pub struct Fields {
    pub kind: *const c_int,
    pub private: *const bool,
}

unsafe extern "C" {
    fn obs_object_query_fields(obj: *const c_void) -> Fields;
}

/// # Safety
/// A non-null `obj` must begin with a live, readable obs_context_data under
/// the same synchronization and lifetime rules as the original C query.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_obj_get_type(obj: *mut c_void) -> c_int {
    let value = NonNull::new(obj).map(|obj| {
        // SAFETY: a present object has a readable context and stored type.
        unsafe { &*obs_object_query_fields(obj.as_ptr()).kind }
    });
    crate::object_queries::object_type(value)
}

/// # Safety
/// A non-null `obj` must begin with a live, readable obs_context_data under
/// the same synchronization and lifetime rules as the original C query.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_obj_is_private(obj: *mut c_void) -> bool {
    let value = NonNull::new(obj).map(|obj| {
        // SAFETY: a present object has a readable context and stored bool.
        unsafe { &*obs_object_query_fields(obj.as_ptr()).private }
    });
    crate::object_queries::is_private(value)
}
