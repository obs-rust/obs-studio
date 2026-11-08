//! Public ABI through borrowed fields of the real private C encoder.
use crate::encoder_gpu_scaling_getters;
use core::ffi::{CStr, c_char, c_int, c_void};

#[repr(C)]
pub struct GpuScalingFields {
    pub kind: *const c_int,
    pub scale: *const c_int,
    pub name: *const *mut c_char,
}

unsafe extern "C" {
    fn obs_encoder_gpu_scaling_getters_fields(encoder: *const c_void) -> GpuScalingFields;
    fn blog(level: c_int, format: *const c_char, ...);
}

impl GpuScalingFields {
    unsafe fn values(&self) -> Option<(i32, i32)> {
        // SAFETY: both addresses belong to the caller's live encoder, or are null.
        unsafe {
            self.kind
                .as_ref()
                .copied()
                .zip(self.scale.as_ref().copied())
        }
    }

    fn log(&self, level: c_int, format: &'static CStr) {
        // SAFETY: only the warning uses the name; the null diagnostic ignores the
        // extra vararg. The borrowed char * remains valid for this synchronous log.
        unsafe {
            let name = self.name.as_ref().copied().unwrap_or(core::ptr::null_mut());
            blog(level, format.as_ptr(), name);
        }
    }
}

/// # Safety
/// Non-null encoders must remain live with fields readable under the same
/// synchronization required by the original C getter.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_gpu_scaling_enabled(encoder: *mut c_void) -> bool {
    // SAFETY: the C adapter only borrows addresses from the caller's live encoder.
    let fields = unsafe { obs_encoder_gpu_scaling_getters_fields(encoder) };
    // SAFETY: the borrowed field addresses remain readable for the call.
    encoder_gpu_scaling_getters::enabled(unsafe { fields.values() }, |level, format| {
        fields.log(level, format)
    })
}

/// # Safety
/// Non-null encoders must remain live with fields readable under the same
/// synchronization required by the original C getter.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_get_scale_type(encoder: *mut c_void) -> c_int {
    // SAFETY: the C adapter only borrows addresses from the caller's live encoder.
    let fields = unsafe { obs_encoder_gpu_scaling_getters_fields(encoder) };
    // SAFETY: the borrowed field addresses remain readable for the call.
    encoder_gpu_scaling_getters::scale_type(unsafe { fields.values() }, |level, format| {
        fields.log(level, format)
    })
}
