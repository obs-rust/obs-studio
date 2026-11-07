//! Borrowed addresses from the real private encoder layout; no ownership changes.
use crate::encoder_metadata;
use core::ffi::{CStr, c_char, c_int, c_void};

#[repr(C)]
pub struct MetadataFields {
    pub codec: *const *const c_char,
    pub kind: *const c_int,
    pub id: *const *const c_char,
    pub type_data: *const *mut c_void,
    pub caps: *const u32,
}

unsafe extern "C" {
    fn obs_encoder_metadata_fields(encoder: *const c_void) -> MetadataFields;
    fn blog(level: c_int, format: *const c_char, ...);
}

fn null_log(function: &'static CStr, parameter: &'static CStr) {
    // SAFETY: NUL-terminated strings match the two format arguments.
    unsafe {
        blog(
            400,
            c"%s: Null '%s' parameter".as_ptr(),
            function.as_ptr(),
            parameter.as_ptr(),
        )
    };
}

/// # Safety
/// Non-null encoders must remain live, with metadata readable for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_get_codec(encoder: *const c_void) -> *const c_char {
    // SAFETY: the adapter takes field addresses, or returns null for no encoder.
    let fields = unsafe { obs_encoder_metadata_fields(encoder) };
    // SAFETY: the caller retains the encoder and its readable codec field.
    encoder_metadata::codec(unsafe { fields.codec.as_ref() }.copied(), null_log)
}

/// # Safety
/// Non-null encoders must remain live, with metadata readable for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_get_type(encoder: *const c_void) -> c_int {
    // SAFETY: the adapter takes field addresses, or returns null for no encoder.
    let fields = unsafe { obs_encoder_metadata_fields(encoder) };
    // SAFETY: the enum's representation is checked against the real C header.
    encoder_metadata::kind(unsafe { fields.kind.as_ref() }.copied(), null_log)
}

/// # Safety
/// Non-null encoders must remain live, with metadata readable for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_get_id(encoder: *const c_void) -> *const c_char {
    // SAFETY: the adapter takes field addresses, or returns null for no encoder.
    let fields = unsafe { obs_encoder_metadata_fields(encoder) };
    // SAFETY: the caller retains the encoder and its readable original ID field.
    encoder_metadata::id(unsafe { fields.id.as_ref() }.copied(), null_log)
}

/// # Safety
/// Non-null encoders must remain live, with metadata readable for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_get_type_data(encoder: *mut c_void) -> *mut c_void {
    // SAFETY: the adapter takes field addresses, or returns null for no encoder.
    let fields = unsafe { obs_encoder_metadata_fields(encoder) };
    // SAFETY: only the stored pointer is copied; its pointee is never accessed.
    encoder_metadata::type_data(unsafe { fields.type_data.as_ref() }.copied(), null_log)
}

/// # Safety
/// Non-null encoders must remain live, with metadata readable for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_get_caps(encoder: *const c_void) -> u32 {
    // SAFETY: the adapter takes field addresses, or returns null for no encoder.
    let fields = unsafe { obs_encoder_metadata_fields(encoder) };
    // SAFETY: the caller retains the encoder and its readable original caps field.
    encoder_metadata::caps(unsafe { fields.caps.as_ref() }.copied(), null_log)
}
