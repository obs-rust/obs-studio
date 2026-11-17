//! ABI bridge borrowing the real C atomic field and existing signal primitive.
use crate::monitoring::Active;
use core::ffi::{c_char, c_int, c_void};
use core::sync::atomic::AtomicBool;

unsafe extern "C" {
    fn obs_source_audio_active_field(source: *const c_void) -> *const bool;
    fn obs_source_audio_active_setter_signal(
        source: *mut c_void,
        global: *const c_char,
        local: *const c_char,
    );
    fn blog(level: c_int, format: *const c_char, ...);
}

/// # Safety
/// A non-null source must remain live throughout this call and its callbacks.
/// All concurrent accesses to its activity flag must follow libobs atomic rules.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_set_audio_active(source: *mut c_void, active: bool) {
    let field = if source.is_null() {
        None
    } else {
        // SAFETY: NULL was checked before forming the private field address.
        let field = unsafe { obs_source_audio_active_field(source) };
        // SAFETY: source is live; C bool/AtomicBool size and alignment are
        // checked against the real header. All concurrent flag accesses are atomic.
        Some(unsafe { AtomicBool::from_ptr(field.cast_mut()) })
    };
    crate::audio_active_setter::set_active(
        field,
        active,
        || {
            // SAFETY: static NUL-terminated strings match the variadic format.
            unsafe {
                blog(
                    400,
                    c"%s: Null '%s' parameter".as_ptr(),
                    c"obs_source_set_audio_active".as_ptr(),
                    c"source".as_ptr(),
                )
            };
        },
        |signal| {
            let (global, local) = match signal {
                Active::Activate => (c"source_audio_activate", c"audio_activate"),
                Active::Deactivate => (c"source_audio_deactivate", c"audio_deactivate"),
                Active::None => return,
            };
            // SAFETY: the safe core signals only a live source after exchanging
            // the flag. The shared primitive delivers stack calldata synchronously.
            unsafe {
                obs_source_audio_active_setter_signal(source, global.as_ptr(), local.as_ptr())
            };
        },
    );
}
