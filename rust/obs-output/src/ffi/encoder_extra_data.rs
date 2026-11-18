//! C boundary for extra-data dispatch; the encoder layout remains C-owned.
use crate::encoder_extra_data::get_extra_data;
use core::ffi::{c_char, c_void};

pub type ExtraDataFn = unsafe extern "C" fn(*mut c_void, *mut *mut u8, *mut usize) -> bool;

/// Borrowed active callback and context, measured against the real private header.
#[repr(C)]
pub struct ExtraDataFields {
    pub callback: Option<ExtraDataFn>,
    pub data: *mut c_void,
}

unsafe extern "C" {
    fn obs_encoder_extra_data_fields(encoder: *const c_void) -> ExtraDataFields;
    fn blog(level: i32, format: *const c_char, ...);
}

/// # Safety
/// A non-NULL encoder must be live. Output pointers must satisfy the installed
/// callback's contract; they may be NULL if that callback tolerates NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_get_extra_data(
    encoder: *const c_void,
    extra_data: *mut *mut u8,
    size: *mut usize,
) -> bool {
    let encoder = if encoder.is_null() {
        None
    } else {
        // SAFETY: NULL was checked before the accessor; caller supplies a live encoder.
        let fields = unsafe { obs_encoder_extra_data_fields(encoder) };
        Some((
            !fields.data.is_null(),
            fields.callback.map(|callback| {
                move || {
                    // SAFETY: the callback and context belong to this live encoder;
                    // the caller supplies outputs accepted by the callback unchanged.
                    unsafe { callback(fields.data, extra_data, size) }
                }
            }),
        ))
    };
    get_extra_data(encoder, || {
        // SAFETY: fixed diagnostic format has no variadic arguments.
        unsafe {
            blog(
                400,
                c"obs_encoder_get_extra_data: Null 'encoder' parameter".as_ptr(),
            )
        };
    })
}
