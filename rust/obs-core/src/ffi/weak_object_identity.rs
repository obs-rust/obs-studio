//! Convert opaque handles only after rejecting null weak handles.
use core::ffi::c_void;

#[repr(C)]
pub struct Fields {
    pub object: *const *mut c_void,
}

unsafe extern "C" {
    fn obs_weak_object_identity_fields(weak: *const c_void) -> Fields;
}

/// # Safety
/// Non-null weak handles must remain live and readable for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_weak_object_references_object(
    weak: *mut c_void,
    object: *mut c_void,
) -> bool {
    let stored = if weak.is_null() || object.is_null() {
        None
    } else {
        // SAFETY: Both NULL handles are rejected before C takes a member address. The caller
        // retains the weak handle and its stored object field is readable.
        Some(unsafe { *obs_weak_object_identity_fields(weak).object } as usize)
    };
    crate::weak_object_identity::references(stored, object as usize)
}
