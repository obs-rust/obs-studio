//! C ABI bridge borrowing addresses from the real private encoder layout.
use core::ffi::{c_int, c_void};

#[repr(C)]
pub struct PreferredRangeFields {
    pub kind: *const c_int,
    pub range: *const u32,
}

unsafe extern "C" {
    fn obs_encoder_preferred_range_fields(encoder: *const c_void) -> PreferredRangeFields;
}

/// # Safety
/// A non-null encoder must remain live with readable type and range fields.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_get_preferred_range(encoder: *const c_void) -> u32 {
    // SAFETY: the adapter borrows addresses from the live encoder, or returns nulls.
    let fields = unsafe { obs_encoder_preferred_range_fields(encoder) };
    // SAFETY: both field pointers are null or readable for the duration of this call.
    let values = unsafe {
        fields
            .kind
            .as_ref()
            .copied()
            .zip(fields.range.as_ref().copied())
    };
    crate::encoder_preferred_range::get(values)
}
