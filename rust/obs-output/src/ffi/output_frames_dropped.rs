//! C ABI over the real private output callback and context.
use crate::NullLog;
use core::ffi::{CStr, c_char, c_int, c_void};
use core::ptr::NonNull;

#[repr(C)]
pub struct Fields {
    pub callback: Option<unsafe extern "C" fn(*mut c_void) -> c_int>,
    pub data: *mut c_void,
}

unsafe extern "C" {
    fn obs_output_frames_dropped_fields(output: *const c_void) -> Fields;
    fn blog(level: c_int, format: *const c_char, ...);
}

struct CLog;
impl NullLog for CLog {
    fn null_output(&self, name: &'static CStr) {
        // SAFETY: static NUL-terminated format and name, matching %s.
        unsafe { blog(400, c"%s: Null 'output' parameter".as_ptr(), name.as_ptr()) }
    }
}

/// # Safety
/// Non-null output and its plugin context must remain live and synchronized as in C.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_get_frames_dropped(output: *const c_void) -> c_int {
    crate::output_frames_dropped::get(
        NonNull::new(output.cast_mut()),
        |output| {
            // SAFETY: the caller supplies the live output; NULL is rejected by the core.
            let fields = unsafe { obs_output_frames_dropped_fields(output.as_ptr()) };
            fields.callback.map(|callback| {
                // SAFETY: original plugin callback receives its original context exactly once.
                unsafe { callback(fields.data) }
            })
        },
        &CLog,
    )
}
