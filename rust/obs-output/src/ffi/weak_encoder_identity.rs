//! Convert opaque handles only after rejecting null weak handles.
use core::ffi::c_void;

#[repr(C)]
pub struct Fields {
    pub encoder: *const *mut c_void,
}

unsafe extern "C" {
    fn obs_weak_encoder_identity_fields(weak: *const c_void) -> Fields;
}

/// # Safety
/// Non-null weak handles must remain live and readable for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_weak_encoder_references_encoder(
    weak: *mut c_void,
    encoder: *mut c_void,
) -> bool {
    let stored = if weak.is_null() || encoder.is_null() {
        None
    } else {
        // SAFETY: Both NULL handles are rejected before C takes a member address. The caller
        // retains the weak handle and its stored encoder field is readable.
        Some(unsafe { *obs_weak_encoder_identity_fields(weak).encoder } as usize)
    };
    crate::weak_encoder_identity::references(stored, encoder as usize)
}
