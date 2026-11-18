//! C boundary for the priming-sample getter; encoder layout remains C-owned.
use crate::encoder_priming_samples::get_priming_samples;
use core::ffi::c_void;

pub type PrimingSamplesFn = unsafe extern "C" fn(*mut c_void) -> u32;

/// Borrowed snapshot measured against the private C header by Tier 3.
#[repr(C)]
pub struct PrimingSamplesFields {
    pub callback: Option<PrimingSamplesFn>,
    pub data: *mut c_void,
}

unsafe extern "C" {
    fn obs_encoder_priming_samples_fields(encoder: *const c_void) -> PrimingSamplesFields;
}

/// # Safety
/// The original API requires a non-NULL live encoder and a valid installed callback/data pair.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_get_priming_samples(encoder: *const c_void) -> u32 {
    if encoder.is_null() {
        return 0;
    }
    // SAFETY: NULL was checked; the caller guarantees the encoder is live.
    let fields = unsafe { obs_encoder_priming_samples_fields(encoder) };
    get_priming_samples(fields.callback.map(|callback| {
        move || {
            // SAFETY: callback and its borrowed data (which may be NULL) came from this live encoder.
            unsafe { callback(fields.data) }
        }
    }))
}
