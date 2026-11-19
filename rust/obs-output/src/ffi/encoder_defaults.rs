//! C boundary for fresh defaults; private encoder fields remain C-owned.
use crate::encoder_defaults::{Host, query};
use core::ffi::{c_char, c_void};
use obs_util::ffi::encoder_type_lookup::find_encoder;

type DefaultsFn = unsafe extern "C" fn(*mut c_void);
type Defaults2Fn = unsafe extern "C" fn(*mut c_void, *mut c_void);

/// Borrowed active callbacks, measured against the real private header.
#[repr(C)]
pub struct DefaultsFields {
    pub legacy: Option<DefaultsFn>,
    pub modern: Option<Defaults2Fn>,
    pub type_data: *mut c_void,
}
unsafe extern "C" {
    fn obs_encoder_defaults_type_fields(info: *const c_void) -> DefaultsFields;
    fn obs_encoder_defaults_instance_fields(encoder: *const c_void) -> DefaultsFields;
    fn obs_data_create() -> *mut c_void;
    fn blog(level: i32, format: *const c_char, ...);
}

// Only the unsafe exports construct this borrowed callback host.
struct CDefaults(DefaultsFields);
impl Host for CDefaults {
    type Settings = *mut c_void;
    type TypeData = *mut c_void;
    fn create_settings(&self) -> Self::Settings {
        // SAFETY: returns a fresh owned obs_data reference for the caller.
        unsafe { obs_data_create() }
    }
    fn has_legacy(&self) -> bool {
        self.0.legacy.is_some()
    }
    fn has_modern(&self) -> bool {
        self.0.modern.is_some()
    }
    fn type_data(&self) -> Self::TypeData {
        self.0.type_data
    }
    fn legacy(&self, settings: &Self::Settings) {
        // SAFETY: the core checks presence; settings and the borrowed table are live.
        unsafe { self.0.legacy.expect("selected legacy callback")(*settings) };
    }
    fn modern(&self, settings: &Self::Settings, type_data: Self::TypeData) {
        // SAFETY: the core checks presence and forwards the table's original type_data.
        unsafe { self.0.modern.expect("selected modern callback")(*settings, type_data) };
    }
}

/// # Safety
/// `id` must be a valid NUL-terminated encoder ID, as in the C API contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_defaults(id: *const c_char) -> *mut c_void {
    // SAFETY: the caller supplies a valid ID; lookup returns a borrowed live table.
    let info = unsafe { find_encoder(id) };
    let host = if info.is_null() {
        None
    } else {
        // SAFETY: checked for NULL before accessing the table.
        Some(CDefaults(unsafe {
            obs_encoder_defaults_type_fields(info.cast())
        }))
    };
    query(host.as_ref(), || {}).unwrap_or(core::ptr::null_mut())
}

/// # Safety
/// A non-NULL encoder must remain live throughout synchronous callback dispatch.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_get_defaults(encoder: *const c_void) -> *mut c_void {
    let host = if encoder.is_null() {
        None
    } else {
        // SAFETY: checked for NULL before the private C field accessor.
        Some(CDefaults(unsafe {
            obs_encoder_defaults_instance_fields(encoder)
        }))
    };
    query(host.as_ref(), || {
        // SAFETY: the fixed diagnostic has no variadic arguments.
        unsafe {
            blog(
                400,
                c"obs_encoder_defaults: Null 'encoder' parameter".as_ptr(),
            )
        };
    })
    .unwrap_or(core::ptr::null_mut())
}
