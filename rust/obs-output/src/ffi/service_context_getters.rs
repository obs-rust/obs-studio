//! Hidden borrowed getters over real private C field addresses.
use crate::service_context_getters;
use core::ffi::{c_char, c_void};

unsafe extern "C" {
    fn obs_service_context_name_field(service: *const c_void) -> *const *mut c_char;
    fn obs_service_context_settings_field(service: *const c_void) -> *const *mut c_void;
}

/// Borrow the context name without copying or acquiring a reference.
/// # Safety
/// `service` must be non-null, live and synchronized, as required by C.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_service_get_context_name(service: *const c_void) -> *const c_char {
    // SAFETY: the caller guarantees the original C preconditions. The adapter
    // exposes only a field address; its nullable value is borrowed unchanged.
    unsafe { service_context_getters::name(&*obs_service_context_name_field(service)).cast_const() }
}

/// Borrow the settings without acquiring a reference.
/// # Safety
/// `service` must be non-null, live and synchronized, as required by C.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_service_get_context_settings(service: *const c_void) -> *mut c_void {
    // SAFETY: the caller guarantees the original C preconditions and field lifetime.
    unsafe { service_context_getters::settings(&*obs_service_context_settings_field(service)) }
}
