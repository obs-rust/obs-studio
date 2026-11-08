//! C ABI over a borrowed pointer to the current info flags. No private layout mirror.
use crate::{NullLog, output_can_pause};
use std::ffi::{CStr, c_char, c_int, c_void};
use std::ptr::NonNull;

unsafe extern "C" {
    fn obs_output_can_pause_flags(output: *const c_void) -> *const u32;
    fn blog(level: c_int, format: *const c_char, ...);
}

struct CLog;
impl NullLog for CLog {
    fn null_output(&self, name: &'static CStr) {
        // SAFETY: static format and name are NUL-terminated; arguments match %s.
        unsafe { blog(400, c"%s: Null 'output' parameter".as_ptr(), name.as_ptr()) }
    }
}

/// Query pause capability without acquiring a reference or calling a plugin.
/// # Safety
/// A non-NULL output must be live, with its flags synchronized as in C.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_can_pause(output: *const c_void) -> bool {
    let flags = NonNull::new(output.cast_mut()).map(|output| {
        // SAFETY: the caller supplies a live output and synchronized flags.
        unsafe { *obs_output_can_pause_flags(output.as_ptr()) }
    });
    output_can_pause::can_pause(flags, &CLog)
}
