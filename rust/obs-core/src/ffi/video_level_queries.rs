//! Thin C ABI shims for the video level queries (#5187).
use core::ffi::c_void;

use crate::video_level_queries::{VideoLevels, hdr_nominal_peak_level, sdr_white_level};

/// Private adapter layout, checked against the real C declaration.
#[repr(C)]
pub struct Fields {
    pub graphics: *const c_void,
    pub sdr_white_level: f32,
    pub hdr_nominal_peak_level: f32,
}

unsafe extern "C" {
    fn obs_video_level_queries_core() -> *const c_void;
    fn obs_video_level_queries_fields(core: *const c_void) -> Fields;
}

/// Read the current core's fields, or `None` for a NULL core. The core is
/// checked before its video fields are touched.
fn current_levels() -> Option<VideoLevels> {
    // SAFETY: the adapter only returns the current global core pointer.
    let core = unsafe { obs_video_level_queries_core() };
    if core.is_null() {
        return None;
    }
    // SAFETY: `core` is the non-NULL current core, which the caller keeps
    // alive, as the original query requires.
    let fields = unsafe { obs_video_level_queries_fields(core) };
    Some(VideoLevels {
        graphics: !fields.graphics.is_null(),
        sdr_white_level: fields.sdr_white_level,
        hdr_nominal_peak_level: fields.hdr_nominal_peak_level,
    })
}

/// # Safety
/// The current core, when not NULL, must be live for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_video_sdr_white_level() -> f32 {
    // SAFETY: forwarded unchanged; the caller upholds the same contract.
    unsafe { obs_video_level_queries_sdr_white_level_request() }
}

/// # Safety
/// The current core, when not NULL, must be live for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_video_hdr_nominal_peak_level() -> f32 {
    // SAFETY: forwarded unchanged; the caller upholds the same contract.
    unsafe { obs_video_level_queries_hdr_nominal_peak_level_request() }
}

/// Explicit static entry for fixtures with their own private core. Hidden
/// from libobs's ABI, so a macOS fixture cannot bind it to libobs.dylib's
/// copy, which reads the dylib's own core (two-level namespace).
///
/// # Safety
/// The current core, when not NULL, must be live for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_video_level_queries_sdr_white_level_request() -> f32 {
    sdr_white_level(current_levels())
}

/// Explicit static entry; see `obs_video_level_queries_sdr_white_level_request`.
///
/// # Safety
/// The current core, when not NULL, must be live for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_video_level_queries_hdr_nominal_peak_level_request() -> f32 {
    hdr_nominal_peak_level(current_levels())
}
