//! Opaque-pointer conversion; the decisions and their order live in the
//! safe core.
use crate::output_update::update;
use core::ffi::{c_char, c_int, c_void};

unsafe extern "C" {
    fn obs_output_update_settings(output: *const c_void) -> *mut c_void;
    fn obs_output_update_has_callback(output: *const c_void) -> bool;
    fn obs_output_update_call(output: *mut c_void);
    fn obs_data_apply(target: *mut c_void, apply_data: *mut c_void);
    fn blog(level: c_int, format: *const c_char, ...);
}

/// # Safety
/// A non-NULL output must be live, with the settings object and update
/// callback its type gave it; `settings` is NULL or a live data object.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_update(output: *mut c_void, settings: *mut c_void) {
    update(
        !output.is_null(),
        || {
            // SAFETY: the core calls this only for a non-NULL output; both
            // objects may be NULL, which obs_data_apply() accepts.
            unsafe { obs_data_apply(obs_output_update_settings(output), settings) }
        },
        || {
            // SAFETY: as above.
            unsafe { obs_output_update_has_callback(output) }
        },
        || {
            // SAFETY: as above, and only when the callback is set. Nothing
            // of the output is held across the plugin's code.
            unsafe { obs_output_update_call(output) }
        },
        |name| {
            // SAFETY: both strings are static and NUL terminated.
            unsafe { blog(400, c"%s: Null 'output' parameter".as_ptr(), name.as_ptr()) }
        },
    );
}
