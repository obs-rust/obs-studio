//! Borrow the real C field only after rejecting NULL, then use the safe query.
use core::ffi::{CStr, c_char, c_int, c_void};

#[repr(C)]
pub struct Fields {
    pub flags: *const u32,
}

unsafe extern "C" {
    fn obs_output_flags_fields(output: *const c_void) -> Fields;
    fn blog(level: c_int, format: *const c_char, ...);
}

fn log(name: &'static CStr) {
    // SAFETY: static terminated format and function name.
    unsafe { blog(400, c"%s: Null 'output' parameter".as_ptr(), name.as_ptr()) }
}

/// # Safety
/// A non-null output must be live with readable flags under the C API contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_get_flags(output: *const c_void) -> u32 {
    let value = core::ptr::NonNull::new(output.cast_mut()).map(|output| {
        // SAFETY: non-null is checked before the C accessor takes a field address;
        // the caller keeps the output and flags live and readable.
        unsafe { *obs_output_flags_fields(output.as_ptr()).flags }
    });
    crate::output_flags::flags(value, log)
}
