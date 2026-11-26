//! C ABI for `libobs/obs-source-show-preloaded-video.c.h`:
//! `obs_source_show_preloaded_video()` (`EXPORT` in `obs.h`,
//! `rust-exports.d`).
//!
//! Runs the safe core ([`crate::show_preloaded_video`]) over the C-owned
//! source: `destroying()`, the preload frame, the graphics context, the
//! async texture upload, the two flags, both mutexes, `last_frame_ts`, the
//! monitoring type, the clock and the two audio resets are reached through
//! the adapters in `obs-source-show-preloaded-video-fields.c.h` (and
//! `obs_source_caption_callback_destroying()`), each the line of the C it
//! stands for, so the atomics, the mutexes and the texture work stay the
//! C's own. A NULL source logs the `obs_source_valid()` line.
//!
//! No intentional differences from C.

use core::ffi::{CStr, c_char, c_int, c_void};

use crate::show_preloaded_video::{self, Host};

unsafe extern "C" {
    fn obs_source_caption_callback_destroying(source: *const c_void) -> bool;
    fn obs_source_preloaded_video_has_frame(source: *const c_void) -> bool;
    fn obs_source_preloaded_video_enter_graphics();
    fn obs_source_preloaded_video_set_texture_size(source: *mut c_void);
    fn obs_source_preloaded_video_update_textures(source: *mut c_void);
    fn obs_source_preloaded_video_mark_fresh(source: *mut c_void);
    fn obs_source_preloaded_video_mark_active(source: *mut c_void);
    fn obs_source_preloaded_video_leave_graphics();
    fn obs_source_preloaded_video_lock_async(source: *mut c_void);
    fn obs_source_preloaded_video_last_frame_ts(source: *const c_void) -> u64;
    fn obs_source_preloaded_video_unlock_async(source: *mut c_void);
    fn obs_source_preloaded_video_lock_audio(source: *mut c_void);
    fn obs_source_preloaded_video_monitor_only(source: *const c_void) -> bool;
    fn obs_source_preloaded_video_now() -> u64;
    fn obs_source_preloaded_video_reset_timing(source: *mut c_void, timestamp: u64, os_time: u64);
    fn obs_source_preloaded_video_reset_data(source: *mut c_void, os_time: u64);
    fn obs_source_preloaded_video_unlock_audio(source: *mut c_void);
    fn blog(level: c_int, format: *const c_char, ...);
}

/// `LOG_DEBUG` (`util/base.h`).
const LOG_DEBUG: c_int = 400;

/// The live libobs objects the caller hands over.
struct Libobs;

// SAFETY (every method): `source` is the caller's live, non-null source;
// the lock and unlock adapters pair as the core calls them, and the graphics
// adapters take no argument.
impl Host for Libobs {
    type Source = *mut c_void;
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
    fn destroying(&self, source: *mut c_void) -> bool {
        // SAFETY: a live, non-null source.
        unsafe { obs_source_caption_callback_destroying(source) }
    }
    fn has_preload_frame(&self, source: *mut c_void) -> bool {
        // SAFETY: a live, non-null source.
        unsafe { obs_source_preloaded_video_has_frame(source) }
    }
    fn enter_graphics(&self) {
        // SAFETY: no argument; obs_enter_graphics() checks the core.
        unsafe { obs_source_preloaded_video_enter_graphics() }
    }
    fn set_texture_size(&self, source: *mut c_void) {
        // SAFETY: a live source whose preload frame the core saw non-NULL.
        unsafe { obs_source_preloaded_video_set_texture_size(source) }
    }
    fn update_textures(&self, source: *mut c_void) {
        // SAFETY: a live source whose preload frame the core saw non-NULL.
        unsafe { obs_source_preloaded_video_update_textures(source) }
    }
    fn mark_textures_fresh(&self, source: *mut c_void) {
        // SAFETY: a live, non-null source.
        unsafe { obs_source_preloaded_video_mark_fresh(source) }
    }
    fn mark_active(&self, source: *mut c_void) {
        // SAFETY: a live, non-null source.
        unsafe { obs_source_preloaded_video_mark_active(source) }
    }
    fn leave_graphics(&self) {
        // SAFETY: pairs the enter above.
        unsafe { obs_source_preloaded_video_leave_graphics() }
    }
    fn lock_async(&self, source: *mut c_void) {
        // SAFETY: a live, non-null source.
        unsafe { obs_source_preloaded_video_lock_async(source) }
    }
    fn last_frame_ts(&self, source: *mut c_void) -> u64 {
        // SAFETY: a live source with its async_mutex held.
        unsafe { obs_source_preloaded_video_last_frame_ts(source) }
    }
    fn unlock_async(&self, source: *mut c_void) {
        // SAFETY: a live source whose async_mutex this call locked.
        unsafe { obs_source_preloaded_video_unlock_async(source) }
    }
    fn lock_audio(&self, source: *mut c_void) {
        // SAFETY: a live, non-null source.
        unsafe { obs_source_preloaded_video_lock_audio(source) }
    }
    fn monitor_only(&self, source: *mut c_void) -> bool {
        // SAFETY: a live source with its audio_buf_mutex held.
        unsafe { obs_source_preloaded_video_monitor_only(source) }
    }
    fn now(&self) -> u64 {
        // SAFETY: no argument.
        unsafe { obs_source_preloaded_video_now() }
    }
    fn reset_audio_timing(&self, source: *mut c_void, timestamp: u64, os_time: u64) {
        // SAFETY: a live source with its audio_buf_mutex held.
        unsafe { obs_source_preloaded_video_reset_timing(source, timestamp, os_time) }
    }
    fn reset_audio_data(&self, source: *mut c_void, os_time: u64) {
        // SAFETY: a live source with its audio_buf_mutex held.
        unsafe { obs_source_preloaded_video_reset_data(source, os_time) }
    }
    fn unlock_audio(&self, source: *mut c_void) {
        // SAFETY: a live source whose audio_buf_mutex this call locked.
        unsafe { obs_source_preloaded_video_unlock_audio(source) }
    }
}

/// `obs_source_show_preloaded_video(source)`.
///
/// # Safety
/// `source` is NULL or a live source.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_show_preloaded_video(source: *mut c_void) {
    show_preloaded_video::show_preloaded_video(&Libobs, (!source.is_null()).then_some(source));
}
