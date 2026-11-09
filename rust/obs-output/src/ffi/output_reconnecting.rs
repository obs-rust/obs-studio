//! Borrow the real C flag address and read it atomically.
use core::ffi::{CStr, c_char, c_int, c_void};
use core::sync::atomic::{AtomicBool, Ordering};

#[repr(C)]
pub struct Fields {
    pub reconnecting: *const bool,
}

unsafe extern "C" {
    fn obs_output_reconnecting_fields(output: *const c_void) -> Fields;
    fn blog(level: c_int, format: *const c_char, ...);
}

fn log(name: &'static CStr) {
    // SAFETY: literal format and static NUL-terminated function name.
    unsafe { blog(400, c"%s: Null 'output' parameter".as_ptr(), name.as_ptr()) }
}

/// # Safety
/// A non-null output must remain live, with its flag aligned for atomic
/// access. Concurrent C writers must use atomic operations on the same flag.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_reconnecting(output: *const c_void) -> bool {
    // SAFETY: the adapter borrows the flag address, or returns a null field.
    let fields = unsafe { obs_output_reconnecting_fields(output) };
    let value = if fields.reconnecting.is_null() {
        None
    } else {
        // SAFETY: live aligned C bool, shared only through atomic accesses.
        Some(unsafe { AtomicBool::from_ptr(fields.reconnecting.cast_mut()) }.load(Ordering::SeqCst))
    };
    crate::output_reconnecting::get(value, log)
}
