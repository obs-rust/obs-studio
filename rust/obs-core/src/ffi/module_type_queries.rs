//! Module type ABI over a private real-header field address.
use std::ffi::{c_uint, c_void};

unsafe extern "C" {
    fn obs_module_type_query_field(module: *mut c_void) -> *const c_uint;
}

/// Classify a core module, returning false for a NULL handle.
///
/// # Safety
/// `module` is NULL or a live, externally synchronized `obs_module_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_is_core_module(module: *mut c_void) -> bool {
    // SAFETY: C provides the real field address for the caller's live module.
    unsafe { crate::module_type_queries::is_core(obs_module_type_query_field(module).as_ref().copied()) }
}

/// Classify a legacy plugin, returning false for a NULL handle.
///
/// # Safety
/// `module` is NULL or a live, externally synchronized `obs_module_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_is_legacy_module(module: *mut c_void) -> bool {
    // SAFETY: C provides the real field address for the caller's live module.
    unsafe { crate::module_type_queries::is_legacy(obs_module_type_query_field(module).as_ref().copied()) }
}
