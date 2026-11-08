//! C ABI over borrowed source fields without mirroring the private source layout.
use crate::state_getters;
use core::ffi::{CStr, c_char, c_int, c_void};

#[repr(C)]
pub struct StateGetterFields {
    pub flags: *const u32,
    pub enabled: *const bool,
    pub last_obs_version: *const u32,
}

unsafe extern "C" {
    fn obs_source_state_getter_fields(source: *const c_void) -> StateGetterFields;
    fn blog(level: c_int, format: *const c_char, ...);
}

fn null_log(function: &'static CStr, parameter: &'static CStr) {
    // SAFETY: the format and both arguments are NUL-terminated static strings.
    unsafe {
        blog(
            400,
            c"%s: Null '%s' parameter".as_ptr(),
            function.as_ptr(),
            parameter.as_ptr(),
        );
    }
}

/// # Safety
/// A non-null source must be live, with the requested field readable under the
/// same synchronization required by the original C getter.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_get_flags(source: *const c_void) -> u32 {
    let value = if source.is_null() {
        None
    } else {
        // SAFETY: NULL was checked before forming any private field addresses.
        let fields = unsafe { obs_source_state_getter_fields(source) };
        // SAFETY: the live source owns this readable field; only it is read.
        Some(unsafe { *fields.flags })
    };
    state_getters::flags(value, null_log)
}

/// # Safety
/// A non-null source must be live, with the requested field readable under the
/// same synchronization required by the original C getter.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_enabled(source: *const c_void) -> bool {
    let value = if source.is_null() {
        None
    } else {
        // SAFETY: NULL was checked before forming any private field addresses.
        let fields = unsafe { obs_source_state_getter_fields(source) };
        // SAFETY: the live source owns this readable field; only it is read.
        Some(unsafe { *fields.enabled })
    };
    state_getters::enabled(value, null_log)
}

/// # Safety
/// A non-null source must be live, with the requested field readable under the
/// same synchronization required by the original C getter.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_get_last_obs_version(source: *const c_void) -> u32 {
    let value = if source.is_null() {
        None
    } else {
        // SAFETY: NULL was checked before forming any private field addresses.
        let fields = unsafe { obs_source_state_getter_fields(source) };
        // SAFETY: the live source owns this readable field; only it is read.
        Some(unsafe { *fields.last_obs_version })
    };
    state_getters::last_obs_version(value, null_log)
}
