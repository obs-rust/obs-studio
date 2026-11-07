//! Borrow field addresses from the real private header, without mirroring the encoder.
use crate::encoder_roi_state;
use core::ffi::{c_char, c_int, c_void};

#[repr(C)]
pub struct RoiStateFields {
    pub count: *const usize,
    pub increment: *const u32,
}

unsafe extern "C" {
    fn obs_encoder_roi_state_fields(encoder: *const c_void) -> RoiStateFields;
    fn blog(level: c_int, format: *const c_char, ...);
}

/// # Safety
/// A non-null encoder must remain live and its ROI count readable during this
/// call, with the same synchronization the original C getter requires.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_has_roi(encoder: *const c_void) -> bool {
    // SAFETY: the adapter only borrows field addresses, or returns null fields.
    let fields = unsafe { obs_encoder_roi_state_fields(encoder) };
    // SAFETY: the caller keeps the encoder live and the requested field readable.
    encoder_roi_state::has_roi(unsafe { fields.count.as_ref() }.copied(), || {
        // SAFETY: static NUL-terminated format without conversion arguments.
        unsafe {
            blog(
                400,
                c"obs_encoder_has_roi: Null 'encoder' parameter".as_ptr(),
            )
        };
    })
}

/// # Safety
/// A non-null encoder must remain live and its ROI increment readable during
/// this call, with the original C getter's synchronization requirements.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_get_roi_increment(encoder: *const c_void) -> u32 {
    // SAFETY: the adapter only borrows field addresses, or returns null fields.
    let fields = unsafe { obs_encoder_roi_state_fields(encoder) };
    // SAFETY: the caller keeps the encoder live and the requested field readable.
    encoder_roi_state::roi_increment(unsafe { fields.increment.as_ref() }.copied(), || {
        // SAFETY: static NUL-terminated format without conversion arguments.
        unsafe {
            blog(
                400,
                c"obs_encoder_get_roi_increment: Null 'encoder' parameter".as_ptr(),
            )
        };
    })
}
