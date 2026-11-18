//! Thin ABI bridge borrowing only the private default_flags field.
use core::ffi::{c_char, c_int, c_void};

unsafe extern "C" {
    fn obs_source_default_flags_field(source: *mut c_void) -> *mut u32;
    fn blog(level: c_int, format: *const c_char, ...);
}

/// # Safety
/// Non-null source must be live and its default flags writable under the same
/// ownership and synchronization rules as the original C setter.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_set_default_flags(source: *mut c_void, flags: u32) {
    let current = if source.is_null() {
        None
    } else {
        // SAFETY: NULL is rejected before forming the member address. The
        // caller provides a live, aligned, exclusively writable uint32 field.
        Some(unsafe { &mut *obs_source_default_flags_field(source) })
    };
    crate::default_flags::set_default_flags(current, flags, || {
        // SAFETY: literal format and NUL-terminated argument match %s.
        unsafe {
            blog(
                400,
                c"%s: Null 'source' parameter".as_ptr(),
                c"obs_source_set_default_flags".as_ptr(),
            );
        }
    });
}
