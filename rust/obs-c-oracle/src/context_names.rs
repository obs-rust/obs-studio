//! `oracle_*` build of `libobs/obs-context-names.c.h`
//! (oracle/context_names.c).
//!
//! The names it returns are `bmalloc`'d by the real `bmalloc` (obs-util's
//! under cargo test), so the caller `bfree`s them as the shims' names.

use core::ffi::{c_char, c_longlong, c_void};

/// `obs_context_name_exists_t`.
pub type OracleContextNameExists =
    Option<unsafe extern "C" fn(param: *mut c_void, name: *const c_char) -> bool>;

unsafe extern "C" {
    pub fn oracle_obs_context_name_dup(
        name: *const c_char,
        private: bool,
        unnamed_index: *mut c_longlong,
    ) -> *mut c_char;
    pub fn oracle_obs_context_name_deduplicate(
        name: *const c_char,
        exists: OracleContextNameExists,
        param: *mut c_void,
    ) -> *mut c_char;
}
