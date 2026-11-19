//! Video SDR white and HDR nominal peak level queries (#5187).
//!
//! Without a core or graphics the no-video defaults are returned; otherwise
//! each stored level is returned with its exact bits, with no validation,
//! clamping or arithmetic.

/// SDR white level without a core or graphics: 300.0 (`0x43960000`).
pub const DEFAULT_SDR_WHITE_LEVEL: f32 = 300.0;
/// HDR nominal peak level without a core or graphics: 1000.0 (`0x447a0000`).
pub const DEFAULT_HDR_NOMINAL_PEAK_LEVEL: f32 = 1000.0;

/// The core video fields the queries read, borrowed from a live core.
#[derive(Clone, Copy, Debug)]
pub struct VideoLevels {
    /// Whether the core has a graphics subsystem.
    pub graphics: bool,
    pub sdr_white_level: f32,
    pub hdr_nominal_peak_level: f32,
}

/// `obs_get_video_sdr_white_level`: `None` is a NULL core.
pub fn sdr_white_level(levels: Option<VideoLevels>) -> f32 {
    let _ = levels;
    todo!("#5187")
}

/// `obs_get_video_hdr_nominal_peak_level`: `None` is a NULL core.
pub fn hdr_nominal_peak_level(levels: Option<VideoLevels>) -> f32 {
    let _ = levels;
    todo!("#5187")
}
