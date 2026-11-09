//! C ABI bridge borrowing addresses from the real private encoder layout.
use core::ffi::{c_int, c_void};

#[repr(C)]
pub struct PreferredColorSpaceFields {
    pub kind: *const c_int,
    pub space: *const u32,
}

unsafe extern "C" {
    fn obs_encoder_preferred_color_space_fields(
        encoder: *const c_void,
    ) -> PreferredColorSpaceFields;
}

/// # Safety
/// A non-null encoder must remain live with readable type and space fields.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_get_preferred_color_space(encoder: *const c_void) -> u32 {
    // SAFETY: the adapter borrows addresses from the live encoder, or returns nulls.
    let fields = unsafe { obs_encoder_preferred_color_space_fields(encoder) };
    // SAFETY: both field pointers are null or readable for the duration of this call.
    let values = unsafe {
        fields
            .kind
            .as_ref()
            .copied()
            .zip(fields.space.as_ref().copied())
    };
    crate::encoder_preferred_color_space::get(values)
}
