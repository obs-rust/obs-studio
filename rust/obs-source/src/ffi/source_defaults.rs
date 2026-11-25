//! The type registry and the callback ABI stay at the C boundary.
use crate::defaults::{Callback, callback, defaults};
use core::ffi::{c_char, c_void};

use super::source_properties::Fields;

unsafe extern "C" {
    /// The private `obs_source_info` read, shared with the properties getter.
    fn obs_source_properties_info_fields(info: *const c_void) -> Fields;
    fn get_source_info(id: *const c_char) -> *const c_void;
    fn obs_data_create() -> *mut c_void;
}

/// # Safety
/// `id` follows the existing C lookup contract and remains valid during the
/// call; a registered type's callbacks and type data are live.
unsafe fn source_defaults(id: *const c_char) -> *mut c_void {
    // SAFETY: the caller provides the original lookup contract.
    let info = unsafe { get_source_info(id) };
    let fields = (!info.is_null()).then(|| {
        // SAFETY: NULL was rejected before C member access; the registry owns the info.
        unsafe { obs_source_properties_info_fields(info) }
    });
    let found = fields
        .as_ref()
        .map(|f| callback(f.defaults2.is_some(), f.defaults.is_some()));
    defaults(
        found,
        // SAFETY: obs_data_create has no preconditions; the caller owns the reference.
        || unsafe { obs_data_create() },
        |kind, &settings| {
            let Some(f) = fields.as_ref() else { return };
            // SAFETY: the safe core selects only a present callback; it fills
            // the object just created, with the type's own data.
            unsafe {
                match kind {
                    Callback::Modern => f.defaults2.map(|cb| cb(f.type_data, settings)),
                    Callback::Legacy => f.defaults.map(|cb| cb(settings)),
                    Callback::Neither => None,
                };
            }
        },
    )
    .unwrap_or(core::ptr::null_mut())
}

/// # Safety
/// As [`obs_get_source_defaults`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_settings(id: *const c_char) -> *mut c_void {
    // SAFETY: the caller provides the original lookup contract.
    unsafe { source_defaults(id) }
}

/// # Safety
/// `id` follows the existing C lookup contract and remains valid during the
/// call, as required by the original C API.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_source_defaults(id: *const c_char) -> *mut c_void {
    // SAFETY: the caller provides the original lookup contract.
    unsafe { source_defaults(id) }
}
