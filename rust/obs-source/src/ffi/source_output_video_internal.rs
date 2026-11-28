//! C ABI for `libobs/obs-source-output-video-internal.c.h`:
//! `obs_source_output_video_internal()` (declared in `obs-internal.h`
//! without `EXPORT`, so the shim stays hidden: `rust-hidden.d`), the worker
//! every `obs_source_output_video*()` entry ends in.
//!
//! Runs the safe core ([`crate::output_video_internal`]) over the C-owned
//! source and frame. The two mutexes, the field writes, the atomics, the
//! profiler call, `cache_video()`, `free_async_cache()`, the reference
//! drop, `obs_source_frame_destroy()` and the `async_frames` push are the
//! adapters of `obs-source-output-video-internal-fields.c.h`, each the line
//! of the C it stands for, so the cache, the copy and the queue stay the
//! C's own. A NULL source logs the `obs_source_valid()` line of the public
//! entry, `obs_source_output_video`.
//!
//! No intentional differences from C.

use core::ffi::{CStr, c_char, c_int, c_long, c_void};

use crate::output_video_internal::{self, Host};

unsafe extern "C" {
    fn obs_source_output_internal_lock_audio_buf(source: *mut c_void);
    fn obs_source_output_internal_frame_timestamp(frame: *const c_void) -> u64;
    fn obs_source_output_internal_set_last_output_frame_ts(source: *mut c_void, timestamp: u64);
    fn obs_source_output_internal_unlock_audio_buf(source: *mut c_void);
    fn obs_source_output_internal_lock_async(source: *mut c_void);
    fn obs_source_output_internal_set_async_active(source: *mut c_void, active: bool);
    fn obs_source_output_internal_mark_stale(source: *mut c_void);
    fn obs_source_output_internal_clear_last_frame_ts(source: *mut c_void);
    fn obs_source_output_internal_free_cache(source: *mut c_void);
    fn obs_source_output_internal_unlock_async(source: *mut c_void);
    fn obs_source_output_internal_frame_received(source: *mut c_void);
    fn obs_source_output_internal_cache_video(
        source: *mut c_void,
        frame: *const c_void,
    ) -> *mut c_void;
    fn obs_source_output_internal_release_ref(frame: *mut c_void) -> c_long;
    fn obs_source_output_internal_destroy_frame(frame: *mut c_void);
    fn obs_source_output_internal_push_frame(source: *mut c_void, frame: *mut c_void);
    fn blog(level: c_int, format: *const c_char, ...);
}

/// `LOG_DEBUG` (`util/base.h`).
const LOG_DEBUG: c_int = 400;

/// The live libobs objects the caller hands over.
struct Libobs;

// SAFETY (every method): `source` is the caller's live, non-null source,
// `frame` its live, non-null frame and `cached` a non-null frame
// cache_video() returned for this call; the lock adapters pair as the core
// calls them.
impl Host for Libobs {
    type Source = *mut c_void;
    type Frame = *const c_void;
    type Cached = *mut c_void;
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
    fn lock_audio_buf(&self, source: *mut c_void) {
        // SAFETY: a live, non-null source.
        unsafe { obs_source_output_internal_lock_audio_buf(source) }
    }
    fn frame_timestamp(&self, frame: *const c_void) -> u64 {
        // SAFETY: a live, non-null frame.
        unsafe { obs_source_output_internal_frame_timestamp(frame) }
    }
    fn set_last_output_frame_ts(&self, source: *mut c_void, timestamp: u64) {
        // SAFETY: a live source with its audio_buf_mutex held.
        unsafe { obs_source_output_internal_set_last_output_frame_ts(source, timestamp) }
    }
    fn unlock_audio_buf(&self, source: *mut c_void) {
        // SAFETY: a live source whose audio_buf_mutex this call locked.
        unsafe { obs_source_output_internal_unlock_audio_buf(source) }
    }
    fn lock_async(&self, source: *mut c_void) {
        // SAFETY: a live, non-null source.
        unsafe { obs_source_output_internal_lock_async(source) }
    }
    fn set_async_active(&self, source: *mut c_void, active: bool) {
        // SAFETY: a live source with its async_mutex held.
        unsafe { obs_source_output_internal_set_async_active(source, active) }
    }
    fn mark_textures_stale(&self, source: *mut c_void) {
        // SAFETY: a live source with its async_mutex held.
        unsafe { obs_source_output_internal_mark_stale(source) }
    }
    fn clear_last_frame_ts(&self, source: *mut c_void) {
        // SAFETY: a live source with its async_mutex held.
        unsafe { obs_source_output_internal_clear_last_frame_ts(source) }
    }
    fn free_async_cache(&self, source: *mut c_void) {
        // SAFETY: a live source with its async_mutex held.
        unsafe { obs_source_output_internal_free_cache(source) }
    }
    fn unlock_async(&self, source: *mut c_void) {
        // SAFETY: a live source whose async_mutex this call locked.
        unsafe { obs_source_output_internal_unlock_async(source) }
    }
    fn frame_received(&self, source: *mut c_void) {
        // SAFETY: a live, non-null source.
        unsafe { obs_source_output_internal_frame_received(source) }
    }
    fn cache_video(&self, source: *mut c_void, frame: *const c_void) -> Option<*mut c_void> {
        // SAFETY: a live source, no lock held (cache_video() takes
        // async_mutex itself), and a live frame.
        let cached = unsafe { obs_source_output_internal_cache_video(source, frame) };
        (!cached.is_null()).then_some(cached)
    }
    fn release_ref(&self, cached: *mut c_void) -> c_long {
        // SAFETY: a frame cache_video() returned, with the reference it added.
        unsafe { obs_source_output_internal_release_ref(cached) }
    }
    fn destroy_frame(&self, cached: *mut c_void) {
        // SAFETY: a frame whose last reference the core just dropped.
        unsafe { obs_source_output_internal_destroy_frame(cached) }
    }
    fn push_frame(&self, source: *mut c_void, cached: *mut c_void) {
        // SAFETY: a live source with its async_mutex held, and a frame
        // cache_video() returned.
        unsafe { obs_source_output_internal_push_frame(source, cached) }
    }
}

/// `obs_source_output_video_internal(source, frame)`.
///
/// # Safety
/// `source` is NULL or a live source; `frame` is NULL or a live frame.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_output_video_internal(
    source: *mut c_void,
    frame: *const c_void,
) {
    output_video_internal::output_video_internal(
        &Libobs,
        (!source.is_null()).then_some(source),
        (!frame.is_null()).then_some(frame),
    );
}
