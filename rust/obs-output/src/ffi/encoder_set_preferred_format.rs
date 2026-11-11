//! C ABI bridge borrowing the real encoder type and writable format field.
use core::ffi::{c_int, c_void};

#[repr(C)]
pub struct Fields {
    pub kind: *const c_int,
    pub format: *mut u32,
}

unsafe extern "C" {
    fn obs_encoder_set_preferred_format_fields(encoder: *mut c_void) -> Fields;
}

/// # Safety
/// A non-null encoder must be live, with exclusive access to its preferred format.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_set_preferred_video_format(encoder: *mut c_void, format: u32) {
    // SAFETY: the adapter returns live member addresses, or two null pointers.
    let fields = unsafe { obs_encoder_set_preferred_format_fields(encoder) };
    // SAFETY: the caller provides readable type and exclusive writable format storage.
    let values = unsafe { fields.kind.as_ref().copied().zip(fields.format.as_mut()) };
    crate::encoder_set_preferred_format::set(values, format);
}
