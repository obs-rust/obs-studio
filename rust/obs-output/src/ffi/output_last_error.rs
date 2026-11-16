//! Own libobs allocations while the safe core replaces the stored error.
use crate::output_last_error;
use core::ffi::{CStr, c_char, c_int, c_void};
use obs_util::ffi::bmem::bmemdup;
use obs_util::ffi::darray::bfree;

unsafe extern "C" {
    fn obs_output_last_error_fields(output: *mut c_void) -> *mut *mut c_char;
    fn blog(level: c_int, format: *const c_char, ...);
}

struct Message(*mut c_char);
impl Drop for Message {
    fn drop(&mut self) {
        // SAFETY: Message exclusively owns an allocation from libobs's allocator.
        unsafe { bfree(self.0.cast()) };
    }
}

/// # Safety
/// A non-null output must be live and exclusively accessible. A non-null message
/// must be NUL-terminated and must not alias the output's old error allocation.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_set_last_error(output: *mut c_void, message: *const c_char) {
    if output.is_null() {
        // SAFETY: static NUL-terminated format with no conversion arguments.
        unsafe {
            blog(
                400,
                c"obs_output_set_last_error: Null 'output' parameter".as_ptr(),
            )
        };
        return;
    }
    // SAFETY: output is non-null and live; the real adapter returns its field address.
    let slot = unsafe { &mut *obs_output_last_error_fields(output) };
    let mut owned = if slot.is_null() {
        None
    } else {
        Some(Message(*slot))
    };
    let bytes = if message.is_null() {
        None
    } else {
        // SAFETY: caller supplies an independent readable NUL-terminated string.
        Some(unsafe { CStr::from_ptr(message) }.to_bytes_with_nul())
    };
    output_last_error::replace(&mut owned, bytes, |bytes| {
        // SAFETY: copy includes the terminating NUL and uses the libobs allocator.
        Message(unsafe { bmemdup(bytes.as_ptr().cast(), bytes.len()) }.cast())
    });
    *slot = owned.map_or(core::ptr::null_mut(), |owned| {
        let pointer = owned.0;
        core::mem::forget(owned);
        pointer
    });
}
