//! Safe core of `libobs/obs-set-video-levels.c.h`: `obs_set_video_levels()`,
//! which stores the core's SDR white level and HDR nominal peak level.
//!
//! With no core ([`levels_core`]) it logs one warning and returns before it
//! reads anything else (#3397). The core's graphics subsystem is required,
//! as the C `assert(video->graphics)` says: without one nothing is read,
//! written or logged. Otherwise each level is handled in turn, the SDR white
//! level first: a level that passes [`level_ok`] (finite and greater than 0,
//! #2209) is stored; a rejected one reads the stored level once and logs one
//! warning naming the rejected and the kept value, and the stored level
//! stays. Nothing else is read or written; the warning text (`%g`) is the
//! C adapter's.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

use crate::core_setup_rules::{level_ok, levels_core};

/// The core's video levels, its graphics field and the three warnings.
pub trait Levels {
    /// `obs != NULL`.
    fn core(&mut self) -> bool;
    /// `obs->video.graphics != NULL`.
    fn graphics(&mut self) -> bool;
    /// One read of `obs->video.sdr_white_level`.
    fn sdr_white_level(&mut self) -> f32;
    /// Stores `obs->video.sdr_white_level`.
    fn set_sdr_white_level(&mut self, level: f32);
    /// One read of `obs->video.hdr_nominal_peak_level`.
    fn hdr_nominal_peak_level(&mut self) -> f32;
    /// Stores `obs->video.hdr_nominal_peak_level`.
    fn set_hdr_nominal_peak_level(&mut self, level: f32);
    /// `obs_set_video_levels: no core, ignoring the levels`.
    fn warn_no_core(&mut self);
    /// `obs_set_video_levels: ignoring SDR white level %g, keeping %g`.
    fn warn_sdr_white_level(&mut self, rejected: f32, kept: f32);
    /// `obs_set_video_levels: ignoring HDR nominal peak level %g, keeping %g`.
    fn warn_hdr_nominal_peak_level(&mut self, rejected: f32, kept: f32);
}

/// `obs_set_video_levels(sdr_white_level, hdr_nominal_peak_level)`.
pub fn set_video_levels<L: Levels>(
    host: &mut L,
    sdr_white_level: f32,
    hdr_nominal_peak_level: f32,
) {
    if !levels_core(host.core()) {
        host.warn_no_core();
        return;
    }
    if !host.graphics() {
        return;
    }

    if level_ok(sdr_white_level) {
        host.set_sdr_white_level(sdr_white_level);
    } else {
        let kept = host.sdr_white_level();
        host.warn_sdr_white_level(sdr_white_level, kept);
    }

    if level_ok(hdr_nominal_peak_level) {
        host.set_hdr_nominal_peak_level(hdr_nominal_peak_level);
    } else {
        let kept = host.hdr_nominal_peak_level();
        host.warn_hdr_nominal_peak_level(hdr_nominal_peak_level, kept);
    }
}
