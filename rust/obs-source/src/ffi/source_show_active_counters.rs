//! C ABI of `obs_source_inc_showing()`, `obs_source_inc_active()`,
//! `obs_source_dec_showing()` and `obs_source_dec_active()`. The counters
//! themselves, and the walk of the source's active tree, are
//! `obs_source_activate()` / `obs_source_deactivate()` (`obs-internal.h`;
//! [`crate::ffi::source_activate`]), called through the C ABI.
use crate::show_active_counters::{Change, Direction, apply};
use core::ffi::{CStr, c_char, c_int, c_void};
use core::ptr::NonNull;

unsafe extern "C" {
    fn obs_source_activate(source: *mut c_void, view_type: u32);
    fn obs_source_deactivate(source: *mut c_void, view_type: u32);
    fn blog(level: c_int, format: *const c_char, ...);
}

/// `LOG_DEBUG`, the level of `obs_source_valid()`'s NULL line.
const LOG_DEBUG: c_int = 400;

fn run(source: *mut c_void, change: Change) {
    apply(
        change,
        NonNull::new(source),
        |source: NonNull<c_void>, direction, view_type| {
            // SAFETY: NULL was rejected above; the caller's source is live
            // under the libobs contract, which is all the C calls need.
            unsafe {
                match direction {
                    Direction::Activate => obs_source_activate(source.as_ptr(), view_type),
                    Direction::Deactivate => obs_source_deactivate(source.as_ptr(), view_type),
                }
            }
        },
        |name: &'static CStr| {
            // SAFETY: static strings match the two %s arguments of the format.
            unsafe {
                blog(
                    LOG_DEBUG,
                    c"%s: Null '%s' parameter".as_ptr(),
                    name.as_ptr(),
                    c"source".as_ptr(),
                );
            }
        },
    );
}

/// # Safety
/// A non-null source must be live under the original libobs ownership contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_inc_showing(source: *mut c_void) {
    run(source, Change::IncShowing);
}

/// # Safety
/// A non-null source must be live under the original libobs ownership contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_inc_active(source: *mut c_void) {
    run(source, Change::IncActive);
}

/// # Safety
/// A non-null source must be live under the original libobs ownership contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_dec_showing(source: *mut c_void) {
    run(source, Change::DecShowing);
}

/// # Safety
/// A non-null source must be live under the original libobs ownership contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_dec_active(source: *mut c_void) {
    run(source, Change::DecActive);
}
