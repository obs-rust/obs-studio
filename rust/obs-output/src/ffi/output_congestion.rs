//! Borrow the plugin callback from the real C output layout.
use core::ffi::{c_char, c_int, c_void};

#[repr(C)]
pub struct Fields {
    pub callback: Option<unsafe extern "C" fn(*mut c_void) -> f32>,
    pub data: *mut c_void,
}

unsafe extern "C" {
    fn obs_output_congestion_fields(output: *const c_void) -> Fields;
    fn blog(level: c_int, format: *const c_char, ...);
}

/// # Safety
/// A non-null output and its plugin callback data must remain live for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_get_congestion(output: *mut c_void) -> f32 {
    crate::output_congestion::get(
        core::ptr::NonNull::new(output),
        |output| {
            // SAFETY: output was checked for NULL and the caller keeps it live.
            let fields = unsafe { obs_output_congestion_fields(output.as_ptr()) };
            fields.callback.map(|callback| {
                // SAFETY: plugin callback and its borrowed data belong to the live output.
                unsafe { callback(fields.data) }
            })
        },
        || {
            // SAFETY: static NUL-terminated format, with no variadic arguments.
            unsafe {
                blog(
                    400,
                    c"obs_output_get_congestion: Null 'output' parameter".as_ptr(),
                )
            }
        },
    )
}
