//! Opaque borrowed source and signal handles; subscription policy is safe Rust.
use crate::audio_pause_callbacks;
use core::ffi::{CStr, c_char, c_int, c_void};

pub type Callback = Option<unsafe extern "C" fn(*mut c_void, *mut c_void)>;

unsafe extern "C" {
    fn obs_source_get_signal_handler(source: *const c_void) -> *mut c_void;
    fn signal_handler_connect(
        handler: *mut c_void,
        signal: *const c_char,
        callback: Callback,
        param: *mut c_void,
    );
    fn signal_handler_disconnect(
        handler: *mut c_void,
        signal: *const c_char,
        callback: Callback,
        param: *mut c_void,
    );
    fn blog(level: c_int, format: *const c_char, ...);
}

unsafe fn update(
    source: *mut c_void,
    callback: Callback,
    param: *mut c_void,
    adding: bool,
    change: unsafe extern "C" fn(*mut c_void, *const c_char, Callback, *mut c_void),
) {
    audio_pause_callbacks::update(
        !source.is_null(),
        adding,
        // SAFETY: the core rejects NULL before calling this; the caller keeps
        // the source and its borrowed signal handler alive throughout the call.
        || unsafe { obs_source_get_signal_handler(source) },
        |handler, signal| {
            // SAFETY: opaque handles, callback and parameter retain the original
            // C contract; both names are static and NUL-terminated.
            unsafe {
                change(*handler, signal.as_ptr(), callback, param);
            }
        },
        log,
    );
}

fn log(name: &'static CStr) {
    // SAFETY: the format and function name are static terminated strings.
    unsafe { blog(400, c"%s: Null 'source' parameter".as_ptr(), name.as_ptr()) }
}

/// # Safety
/// Non-null sources and callback parameters must obey the original libobs
/// lifetime contract; the callback must accept a live calldata pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_add_audio_pause_callback(
    source: *mut c_void,
    callback: Callback,
    param: *mut c_void,
) {
    // SAFETY: the caller supplies the original C source/callback contract.
    unsafe { update(source, callback, param, true, signal_handler_connect) }
}

/// # Safety
/// Non-null sources and callback parameters must obey the original libobs
/// lifetime contract, including synchronization with signal dispatch.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_remove_audio_pause_callback(
    source: *mut c_void,
    callback: Callback,
    param: *mut c_void,
) {
    // SAFETY: the caller supplies the original C source/callback contract.
    unsafe { update(source, callback, param, false, signal_handler_disconnect) }
}
