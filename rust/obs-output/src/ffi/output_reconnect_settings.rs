//! Borrow reconnect settings through the private C layout adapter.
use core::ffi::{c_char, c_int, c_void};

#[repr(C)]
pub struct Fields {
    pub retry_max: *mut c_int,
    pub retry_sec: *mut c_int,
}

unsafe extern "C" {
    fn obs_output_reconnect_settings_fields(output: *mut c_void) -> Fields;
    fn blog(level: c_int, format: *const c_char, ...);
}

/// # Safety
/// A non-null output must be live and exclusively accessible during this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_set_reconnect_settings(
    output: *mut c_void,
    retry_count: c_int,
    retry_sec: c_int,
) {
    let fields = if output.is_null() {
        None
    } else {
        // SAFETY: null was checked before the adapter forms field addresses.
        let fields = unsafe { obs_output_reconnect_settings_fields(output) };
        // SAFETY: the adapter returns two distinct int fields of the live output.
        Some(unsafe { (&mut *fields.retry_max, &mut *fields.retry_sec) })
    };
    crate::output_reconnect_settings::set(fields, retry_count, retry_sec, || {
        // SAFETY: static NUL-terminated diagnostic without format arguments.
        unsafe {
            blog(
                400,
                c"obs_output_set_reconnect_settings: Null 'output' parameter".as_ptr(),
            )
        }
    });
}
