//! C ABI for `libobs/obs-source-get-release-frame.c.h`:
//! `obs_source_get_frame()` and `obs_source_release_frame()` (`EXPORT` in
//! `obs.h`, `rust-exports.d`).
//!
//! Runs the safe core ([`crate::get_release_frame`]) over the C-owned
//! source and frame: `async_mutex`, `cur_async_frame`, the frame's `refs`,
//! `obs_source_frame_destroy()` and `remove_async_frame()` are reached
//! through the adapters in `obs-source-get-release-frame-fields.c.h`, each
//! the line of the C it stands for, so the atomics and the mutex stay the
//! C's own. A NULL source logs the `obs_source_valid()` line.
//!
//! No intentional differences from C.

use core::ffi::{CStr, c_char, c_int, c_void};

use crate::get_release_frame::{self, Host};

unsafe extern "C" {
    fn obs_source_async_frame_lock(source: *mut c_void);
    fn obs_source_async_frame_unlock(source: *mut c_void);
    fn obs_source_async_frame_take_current(source: *mut c_void) -> *mut c_void;
    fn obs_source_async_frame_addref(frame: *mut c_void);
    fn obs_source_async_frame_release_ref(frame: *mut c_void) -> bool;
    fn obs_source_async_frame_destroy(frame: *mut c_void);
    fn obs_source_async_frame_return(source: *mut c_void, frame: *mut c_void);
    fn blog(level: c_int, format: *const c_char, ...);
}

/// `LOG_DEBUG` (`util/base.h`).
const LOG_DEBUG: c_int = 400;

/// The live libobs objects the caller hands over.
struct Libobs;

impl Host for Libobs {
    type Source = *mut c_void;
    type Frame = *mut c_void;
    fn null_source(&self, func: &'static CStr) {
        // SAFETY: static strings match the two %s arguments of the format.
        unsafe {
            blog(
                LOG_DEBUG,
                c"%s: Null '%s' parameter".as_ptr(),
                func.as_ptr(),
                c"source".as_ptr(),
            );
        }
    }
    fn lock(&self, source: *mut c_void) {
        // SAFETY: a live, non-null source.
        unsafe { obs_source_async_frame_lock(source) }
    }
    fn unlock(&self, source: *mut c_void) {
        // SAFETY: a live source whose async_mutex this call locked.
        unsafe { obs_source_async_frame_unlock(source) }
    }
    fn take_current(&self, source: *mut c_void) -> Option<*mut c_void> {
        // SAFETY: a live source with its async_mutex held.
        let frame = unsafe { obs_source_async_frame_take_current(source) };
        (!frame.is_null()).then_some(frame)
    }
    fn addref(&self, frame: *mut c_void) {
        // SAFETY: the non-null frame the source held, under its mutex.
        unsafe { obs_source_async_frame_addref(frame) }
    }
    fn release_ref(&self, frame: *mut c_void) -> bool {
        // SAFETY: the caller's live, non-null frame.
        unsafe { obs_source_async_frame_release_ref(frame) }
    }
    fn destroy(&self, frame: *mut c_void) {
        // SAFETY: the caller's live, non-null frame, given up here.
        unsafe { obs_source_async_frame_destroy(frame) }
    }
    fn return_to_cache(&self, source: *mut c_void, frame: *mut c_void) {
        // SAFETY: a live source with its async_mutex held and the caller's
        // live frame.
        unsafe { obs_source_async_frame_return(source, frame) }
    }
}

/// `obs_source_get_frame(source)`.
///
/// # Safety
/// `source` is NULL or a live source.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_get_frame(source: *mut c_void) -> *mut c_void {
    get_release_frame::get_frame(&Libobs, (!source.is_null()).then_some(source))
        .unwrap_or(core::ptr::null_mut())
}

/// `obs_source_release_frame(source, frame)`.
///
/// # Safety
/// `source` is NULL or a live source; `frame` is NULL or a live frame the
/// caller holds a reference to (or owns, with a NULL source).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_release_frame(source: *mut c_void, frame: *mut c_void) {
    get_release_frame::release_frame(
        &Libobs,
        (!source.is_null()).then_some(source),
        (!frame.is_null()).then_some(frame),
    );
}
