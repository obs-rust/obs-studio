//! Thin C ABI reads through private typed field pointers, never obs_core layout.
use crate::global_getters::borrowed;
use std::ffi::{c_char, c_void};

unsafe extern "C" {
    fn obs_core_global_locale_field() -> *const *mut c_char;
    fn obs_core_global_signals_field() -> *const *mut c_void;
    fn obs_core_global_procs_field() -> *const *mut c_void;
}

/// Return the core's borrowed locale pointer.
///
/// # Safety
/// The core must be initialized and its locale must not be concurrently changed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_locale() -> *const c_char {
    // SAFETY: the private adapter points to the initialized core's live field.
    unsafe { borrowed(&*obs_core_global_locale_field()).cast_const() }
}

/// Return the core's borrowed signal handler.
///
/// # Safety
/// The core must be initialized and must not be concurrently destroyed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_signal_handler() -> *mut c_void {
    // SAFETY: the private adapter points to the initialized core's live field.
    unsafe { borrowed(&*obs_core_global_signals_field()) }
}

/// Return the core's borrowed procedure handler.
///
/// # Safety
/// The core must be initialized and must not be concurrently destroyed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_proc_handler() -> *mut c_void {
    // SAFETY: the private adapter points to the initialized core's live field.
    unsafe { borrowed(&*obs_core_global_procs_field()) }
}
