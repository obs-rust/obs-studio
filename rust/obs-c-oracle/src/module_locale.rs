//! `oracle_*` build of `libobs/obs-module-locale.c.h`
//! (oracle/module_locale.c).
//!
//! Its "same locale" decision is the original C `astrcmpi` (the dstr
//! oracle's), and its names are `bmalloc`'d by the real `bmalloc`, so the
//! caller `bfree`s them as the shims' names.

use core::ffi::c_char;

unsafe extern "C" {
    pub fn oracle_obs_module_locale_default_file(default_locale: *const c_char) -> *mut c_char;
    pub fn oracle_obs_module_locale_is_default(
        locale: *const c_char,
        default_locale: *const c_char,
    ) -> bool;
    pub fn oracle_obs_module_locale_file(locale: *const c_char) -> *mut c_char;
}
