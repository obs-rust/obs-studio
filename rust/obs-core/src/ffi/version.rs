//! `obs_get_version` and `obs_get_version_string`.
use crate::version::LIBOBS_API_VER;
use core::ffi::c_char;

unsafe extern "C" {
    /// Address of libobs' `OBS_VERSION` global (`obsversion.c`), from
    /// `libobs/obs-version-fields.c.h`.
    fn obs_version_string_field() -> *const *const c_char;
}

/// The libobs API version, `LIBOBS_API_VER`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_get_version() -> u32 {
    LIBOBS_API_VER
}

/// The `OBS_VERSION` pointer itself, not a copy.
///
/// # Safety
/// Nothing may write the `OBS_VERSION` global concurrently; libobs never does.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_version_string() -> *const c_char {
    // SAFETY: the adapter returns the address of the live `OBS_VERSION`
    // global, defined before startup and after shutdown. Only the pointer is
    // read; its string is never dereferenced here.
    unsafe { *obs_version_string_field() }
}
