//! C ABI borrowing only the live source's compensation atomic.
use core::ffi::{c_char, c_int, c_void};
use core::sync::atomic::AtomicBool;

unsafe extern "C" {
    fn obs_source_async_compensation_field(source: *mut c_void) -> *mut bool;
    fn blog(level: c_int, format: *const c_char, ...);
}

/// # Safety
/// A non-null source must remain live throughout this call. Concurrent access
/// to its compensation flag must follow libobs's atomic access rules.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_set_async_compensation(source: *mut c_void, compensate: bool) {
    let value = if source.is_null() {
        None
    } else {
        // SAFETY: NULL is checked before C forms a private field address.
        let field = unsafe { obs_source_async_compensation_field(source) };
        // SAFETY: the live C bool has the checked AtomicBool size/alignment;
        // all overlapping accesses use compatible atomics, never plain loads.
        Some(unsafe { AtomicBool::from_ptr(field) })
    };
    crate::async_compensation::set(value, compensate, || {
        // SAFETY: static format and NUL-terminated string arguments match %s.
        unsafe {
            blog(
                400,
                c"%s: Null '%s' parameter".as_ptr(),
                c"obs_source_set_async_compensation".as_ptr(),
                c"source".as_ptr(),
            );
        }
    });
}
