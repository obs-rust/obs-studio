//! Borrow the callback and plugin data through the real C layout.
use core::ffi::{c_char, c_int, c_void};
#[repr(C)]
pub struct Fields {
    pub data: *mut c_void,
    pub callback: Option<unsafe extern "C" fn(*mut c_void) -> c_int>,
}
unsafe extern "C" {
    fn obs_output_connect_time_fields(output: *const c_void) -> Fields;
    fn blog(level: c_int, format: *const c_char, ...);
}
/// # Safety
/// Non-null output and its plugin data must remain live for the callback.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_get_connect_time_ms(output: *mut c_void) -> c_int {
    crate::output_connect_time::get(
        (!output.is_null()).then_some(output),
        |output| {
            // SAFETY: the caller provides a live output; the adapter borrows its fields.
            let fields = unsafe { obs_output_connect_time_fields(output) };
            fields.callback.map(|callback| {
                // SAFETY: callback and data are borrowed together from the live output.
                unsafe { callback(fields.data) }
            })
        },
        || {
            // SAFETY: static NUL-terminated diagnostic with no format arguments.
            unsafe {
                blog(
                    400,
                    c"obs_output_get_connect_time_ms: Null 'output' parameter".as_ptr(),
                )
            }
        },
    )
}
