//! C ABI bridge borrowing the real encoder type and writable color fields.
use core::ffi::{c_int, c_void};

#[repr(C)]
pub struct Fields {
    pub kind: *const c_int,
    pub space: *mut u32,
    pub range: *mut u32,
}

unsafe extern "C" {
    fn obs_encoder_preferred_color_setters_fields(encoder: *mut c_void) -> Fields;
}

/// # Safety
/// A non-null encoder must be live, with exclusive access to its preferred color space.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_set_preferred_color_space(encoder: *mut c_void, space: u32) {
    if encoder.is_null() {
        return;
    }
    // SAFETY: the encoder is non-null and live; the adapter only borrows member addresses.
    let fields = unsafe { obs_encoder_preferred_color_setters_fields(encoder) };
    // SAFETY: the caller provides readable type and exclusive writable color-space storage.
    let values = unsafe { fields.kind.as_ref().copied().zip(fields.space.as_mut()) };
    crate::encoder_preferred_color_setters::set(values, space);
}

/// # Safety
/// A non-null encoder must be live, with exclusive access to its preferred range.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_set_preferred_range(encoder: *mut c_void, range: u32) {
    if encoder.is_null() {
        return;
    }
    // SAFETY: the encoder is non-null and live; the adapter only borrows member addresses.
    let fields = unsafe { obs_encoder_preferred_color_setters_fields(encoder) };
    // SAFETY: the caller provides readable type and exclusive writable range storage.
    let values = unsafe { fields.kind.as_ref().copied().zip(fields.range.as_mut()) };
    crate::encoder_preferred_color_setters::set(values, range);
}
