//! Public ABI through borrowed fields of the real C encoder.
use crate::encoder_frame_rate_divisor;
use core::ffi::{c_char, c_int, c_void};

#[repr(C)]
pub struct DivisorFields {
    pub kind: *const c_int,
    pub divisor: *const u32,
    pub name: *const *mut c_char,
}

unsafe extern "C" {
    fn obs_encoder_frame_rate_divisor_fields(encoder: *const c_void) -> DivisorFields;
    fn blog(level: c_int, format: *const c_char, ...);
}

/// # Safety
/// Non-null encoders must remain live with their fields readable under the same
/// synchronization required by the original C getter.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_get_frame_rate_divisor(encoder: *const c_void) -> u32 {
    // SAFETY: the C adapter returns borrowed field addresses or nulls.
    let fields = unsafe { obs_encoder_frame_rate_divisor_fields(encoder) };
    // SAFETY: these fields belong to the caller's live encoder, or are null.
    let values = unsafe {
        fields
            .kind
            .as_ref()
            .copied()
            .zip(fields.divisor.as_ref().copied())
    };
    encoder_frame_rate_divisor::get(values, |level, format| {
        // SAFETY: only the warning uses the name; the null diagnostic ignores the
        // extra vararg. The borrowed name remains valid for this synchronous log.
        unsafe {
            let name = fields
                .name
                .as_ref()
                .copied()
                .unwrap_or(core::ptr::null_mut());
            blog(level, format.as_ptr(), name);
        }
    })
}
