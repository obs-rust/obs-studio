//! Scene rendering follows the main video's color space, ignoring preferences.

pub const GS_CS_SRGB: i32 = 0;
pub const GS_CS_709_EXTENDED: i32 = 2;
pub const VIDEO_CS_SRGB: i32 = 3;
pub const VIDEO_CS_2100_PQ: i32 = 4;
pub const VIDEO_CS_2100_HLG: i32 = 5;

/// Missing video information and non-HDR video use sRGB.
pub fn video_color_space(video: Option<i32>) -> i32 {
    match video {
        Some(VIDEO_CS_2100_PQ | VIDEO_CS_2100_HLG) => GS_CS_709_EXTENDED,
        _ => GS_CS_SRGB,
    }
}
