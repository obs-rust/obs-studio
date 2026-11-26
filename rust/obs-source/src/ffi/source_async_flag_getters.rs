//! C ABI over borrowed fields, without mirroring the private source layout:
//! the async flag getters, the unbuffered setter and the last rendered async
//! timestamp (#5766).

use crate::async_flag_getters;
use core::ffi::{CStr, c_char, c_int, c_void};

#[repr(C)]
pub struct AsyncFlagFields {
    pub unbuffered: *const bool,
    pub decoupled: *const bool,
}

unsafe extern "C" {
    fn obs_source_async_flag_fields(source: *const c_void) -> AsyncFlagFields;
    fn obs_source_async_last_ts_field(source: *const c_void) -> *const u64;
    fn blog(level: c_int, format: *const c_char, ...);
}

fn null_log(function: &'static CStr, parameter: &'static CStr) {
    // SAFETY: the format and its two string arguments are NUL terminated.
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
/// A non-null source must be live, with its async unbuffered flag readable under the same
/// synchronization required by the original C getter.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_async_unbuffered(source: *const c_void) -> bool {
    // SAFETY: the adapter only returns borrowed addresses, or NULL fields.
    let fields = unsafe { obs_source_async_flag_fields(source) };
    // SAFETY: only the requested field is read; the source stays live.
    async_flag_getters::unbuffered(unsafe { fields.unbuffered.as_ref() }.copied(), null_log)
}

/// # Safety
/// A non-null source must be live, with its async decoupled flag readable under the
/// same synchronization required by the original C getter.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_async_decoupled(source: *const c_void) -> bool {
    // SAFETY: the adapter only returns borrowed addresses, or NULL fields.
    let fields = unsafe { obs_source_async_flag_fields(source) };
    // SAFETY: only the requested field is read; the source stays live.
    async_flag_getters::decoupled(unsafe { fields.decoupled.as_ref() }.copied(), null_log)
}

/// # Safety
/// A non-null source must be live, with its async unbuffered flag writable under the same
/// synchronization required by the original C setter (a plain store, no lock).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_set_async_unbuffered(source: *mut c_void, unbuffered: bool) {
    let field = if source.is_null() {
        None
    } else {
        // SAFETY: NULL is rejected first; the adapter returns the live source's
        // own field address. The source is not const, so the store through
        // that address is the original C store.
        let fields = unsafe { obs_source_async_flag_fields(source) };
        // SAFETY: the address is the source's aligned, initialized bool field,
        // written by no other party during this call under the C contract.
        unsafe { fields.unbuffered.cast_mut().as_mut() }
    };
    async_flag_getters::set_unbuffered(field, unbuffered, null_log);
}

/// `obs_source_get_last_async_ts(source)` (#5766), hidden in libobs as the C
/// was (`obs-internal.h`, no `EXPORT`; `rust-hidden.d`). The source profiler
/// calls it for an async video source each tick.
///
/// # Safety
/// `source` must be a live, non-null source: the C dereferences it with no
/// check, and the field is read under the same synchronization as the C (a
/// plain read, no lock).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_get_last_async_ts(source: *const c_void) -> u64 {
    // SAFETY: the caller's source is live and non-null; the adapter only forms
    // the address of its aligned, initialized timestamp field.
    let field = unsafe { &*obs_source_async_last_ts_field(source) };
    async_flag_getters::last_async_ts(field)
}
