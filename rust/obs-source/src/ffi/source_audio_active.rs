//! C ABI borrowing the live source's atomic flag, without mirroring its layout.
use core::ffi::{c_char, c_int, c_void};
use core::sync::atomic::AtomicBool;

unsafe extern "C" {
    fn obs_source_audio_active_field(source: *const c_void) -> *const bool;
    fn blog(level: c_int, format: *const c_char, ...);
}

/// # Safety
/// A non-null source must be live and readable for the duration of the query.
/// Concurrent writers must use the same atomic access rules as libobs.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_audio_active(source: *const c_void) -> bool {
    let value = if source.is_null() {
        None
    } else {
        // SAFETY: NULL was checked before C forms a private field address.
        let field = unsafe { obs_source_audio_active_field(source) };
        // SAFETY: C bool and AtomicBool have the checked size/alignment; the
        // source owns this live bool and all concurrent access is atomic.
        Some(unsafe { AtomicBool::from_ptr(field.cast_mut()) })
    };
    crate::audio_active::active(value, || {
        // SAFETY: static format and NUL-terminated string arguments match %s.
        unsafe {
            blog(
                400,
                c"%s: Null '%s' parameter".as_ptr(),
                c"obs_source_audio_active".as_ptr(),
                c"source".as_ptr(),
            );
        }
    })
}
