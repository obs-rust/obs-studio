//! C ABI over a private field-pointer view measured from obs-internal.h.

use crate::encoder_counters;
use core::ffi::{CStr, c_char, c_int, c_void};

/// C owns the encoder and both counters; this view borrows their addresses.
#[repr(C)]
pub struct CounterFields {
    pub encoded_frames: *const u32,
    pub pause_offset: *const u64,
}

unsafe extern "C" {
    fn obs_encoder_counter_fields(encoder: *const c_void) -> CounterFields;
    fn blog(level: c_int, format: *const c_char, ...);
}

fn null_log(function: &'static CStr, parameter: &'static CStr) {
    // SAFETY: all strings are NUL terminated, with the format's two arguments.
    unsafe {
        blog(
            400,
            c"%s: Null '%s' parameter".as_ptr(),
            function.as_ptr(),
            parameter.as_ptr(),
        );
    }
}

/// # Safety
/// A non-null encoder must be live for the call, with its counter readable
/// under the same synchronization required by the C accessor.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_get_encoded_frames(encoder: *const c_void) -> u32 {
    // SAFETY: the C adapter only takes field addresses, or returns null fields.
    let fields = unsafe { obs_encoder_counter_fields(encoder) };
    // SAFETY: the field belongs to the retained encoder; null means no encoder.
    let value = unsafe { fields.encoded_frames.as_ref() }.copied();
    encoder_counters::encoded_frames(value, null_log)
}

/// # Safety
/// A non-null encoder must be live for the call, with its counter readable
/// under the same synchronization required by the C accessor.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_get_pause_offset(encoder: *const c_void) -> u64 {
    // SAFETY: the C adapter only takes field addresses, or returns null fields.
    let fields = unsafe { obs_encoder_counter_fields(encoder) };
    // SAFETY: the field belongs to the retained encoder; null means no encoder.
    encoder_counters::pause_offset(unsafe { fields.pause_offset.as_ref() }.copied())
}
