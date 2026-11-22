//! `obs_set_video_levels()` over C-owned state: the core, its graphics
//! subsystem, the two video levels and the warnings with the original's
//! `%g` format strings, behind the private adapters of
//! `libobs/obs-set-video-levels-fields.c.h`. What is read, written and
//! logged is the safe core's ([`crate::set_video_levels`]).

unsafe extern "C" {
    fn obs_set_video_levels_has_core() -> bool;
    fn obs_set_video_levels_has_graphics() -> bool;
    fn obs_set_video_levels_sdr_white_level() -> f32;
    fn obs_set_video_levels_store_sdr_white_level(level: f32);
    fn obs_set_video_levels_hdr_nominal_peak_level() -> f32;
    fn obs_set_video_levels_store_hdr_nominal_peak_level(level: f32);
    fn obs_set_video_levels_warn_no_core();
    fn obs_set_video_levels_warn_sdr_white_level(rejected: f32, kept: f32);
    fn obs_set_video_levels_warn_hdr_nominal_peak_level(rejected: f32, kept: f32);
}

/// The current core. The safe core asks for a field only after
/// [`Levels::core`](crate::set_video_levels::Levels::core) found one, and
/// for a level only after the graphics check.
struct Core;

impl crate::set_video_levels::Levels for Core {
    fn core(&mut self) -> bool {
        // SAFETY: reads only the current core pointer.
        unsafe { obs_set_video_levels_has_core() }
    }
    fn graphics(&mut self) -> bool {
        // SAFETY: the core is non-NULL and live (checked just before).
        unsafe { obs_set_video_levels_has_graphics() }
    }
    fn sdr_white_level(&mut self) -> f32 {
        // SAFETY: as above.
        unsafe { obs_set_video_levels_sdr_white_level() }
    }
    fn set_sdr_white_level(&mut self, level: f32) {
        // SAFETY: as above.
        unsafe { obs_set_video_levels_store_sdr_white_level(level) }
    }
    fn hdr_nominal_peak_level(&mut self) -> f32 {
        // SAFETY: as above.
        unsafe { obs_set_video_levels_hdr_nominal_peak_level() }
    }
    fn set_hdr_nominal_peak_level(&mut self, level: f32) {
        // SAFETY: as above.
        unsafe { obs_set_video_levels_store_hdr_nominal_peak_level(level) }
    }
    fn warn_no_core(&mut self) {
        // SAFETY: logs a static message; reads no core.
        unsafe { obs_set_video_levels_warn_no_core() }
    }
    fn warn_sdr_white_level(&mut self, rejected: f32, kept: f32) {
        // SAFETY: logs the two values; reads no core.
        unsafe { obs_set_video_levels_warn_sdr_white_level(rejected, kept) }
    }
    fn warn_hdr_nominal_peak_level(&mut self, rejected: f32, kept: f32) {
        // SAFETY: as above.
        unsafe { obs_set_video_levels_warn_hdr_nominal_peak_level(rejected, kept) }
    }
}

/// Sets the core's SDR white level and HDR nominal peak level, keeping the
/// stored one (with a warning) for a level that is not finite and greater
/// than 0.
///
/// # Safety
/// As for the C function: the current core, when not NULL, is live, and so
/// is its graphics subsystem.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_set_video_levels(sdr_white_level: f32, hdr_nominal_peak_level: f32) {
    crate::set_video_levels::set_video_levels(&mut Core, sdr_white_level, hdr_nominal_peak_level);
}
