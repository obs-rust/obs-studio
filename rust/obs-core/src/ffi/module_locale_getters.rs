//! Opaque module callback access; C measures the real field address.
use std::ffi::{c_char, c_void};

type Callback = unsafe extern "C" fn(*const c_char, *mut *const c_char) -> bool;
unsafe extern "C" {
    fn obs_module_locale_getters_callback_field(module: *const c_void) -> *const Option<Callback>;
}

/// Dispatch a plugin's locale lookup without interpreting its arguments or result.
///
/// # Safety
/// `module` is null or a live, synchronized module. The callback's requirements
/// for `key` and `out` must hold. A null module asserts in debug builds.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_module_get_locale_string(
    module: *const c_void,
    key: *const c_char,
    out: *mut *const c_char,
) -> bool {
    debug_assert!(!module.is_null());
    // SAFETY: the C accessor returns this live module's actual callback field.
    let callback = unsafe {
        obs_module_locale_getters_callback_field(module)
            .as_ref()
            .copied()
            .flatten()
    };
    crate::module_locale_getters::get_string(callback.map(|callback| {
        move || {
            // SAFETY: callback inputs meet the plugin's contract per the caller.
            unsafe { callback(key, out) }
        }
    }))
}

/// Return the callback's output, initially the input pointer, even on failure.
///
/// # Safety
/// `module` and `text` meet the module callback's requirements; returned storage
/// is borrowed from the input or plugin. A null module asserts in debug builds.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_module_get_locale_text(
    module: *const c_void,
    text: *const c_char,
) -> *const c_char {
    crate::module_locale_getters::get_text(
        text,
        Some(|out: &mut *const c_char| {
            // SAFETY: the caller supplies a live module and valid callback input.
            unsafe { obs_module_get_locale_string(module, text, out) }
        }),
    )
}
