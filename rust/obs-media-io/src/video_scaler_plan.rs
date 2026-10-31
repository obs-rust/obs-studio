//! Safe core of `libobs/media-io/video-scaler-plan.c.h`: the decisions of
//! the swscale scaler (`video-scaler-ffmpeg.c`).
//!
//! - [`sws_flags`]: the swscale flags for an `enum video_scale_type`.
//! - [`sws_colorspace`]: the swscale coefficient set for an `enum
//!   video_colorspace`.
//! - [`sws_range`]: the swscale range for an `enum video_range_type`.
//! - [`plane_heights`]: the height of each plane the destination format has.
//! - [`plane_copy`]: how one scaled plane is copied to the output.
//!
//! `video-scaler-ffmpeg.c` keeps every swscale and libavutil call, the pixel
//! format table and the descriptor reads, the copies themselves and every
//! log. Nothing here keeps state, takes a lock or allocates, and nothing can
//! panic on any input.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable
)]

use core::ffi::c_int;

/// The swscale flags (`SWS_*`) and coefficient sets (`SWS_CS_*`) the scaler
/// uses, as `VIDEO_SCALER_SWS_*` gives them.
pub mod sws {
    use core::ffi::c_int;

    /// `SWS_FAST_BILINEAR`.
    pub const FAST_BILINEAR: c_int = 0x01;
    /// `SWS_BILINEAR`.
    pub const BILINEAR: c_int = 0x02;
    /// `SWS_BICUBIC`.
    pub const BICUBIC: c_int = 0x04;
    /// `SWS_POINT`.
    pub const POINT: c_int = 0x10;
    /// `SWS_AREA`.
    pub const AREA: c_int = 0x20;
    /// `SWS_CS_ITU709`.
    pub const CS_ITU709: c_int = 1;
    /// `SWS_CS_ITU601`.
    pub const CS_ITU601: c_int = 5;
    /// `SWS_CS_BT2020`.
    pub const CS_BT2020: c_int = 9;
}

/// `video_scaler_sws_flags()`: the swscale flags for the `enum
/// video_scale_type` value `scale_type`.
pub fn sws_flags(scale_type: c_int) -> c_int {
    let _ = scale_type;
    todo!()
}

/// `video_scaler_sws_colorspace()`: the swscale coefficient set for the `enum
/// video_colorspace` value `color_space`.
pub fn sws_colorspace(color_space: c_int) -> c_int {
    let _ = color_space;
    todo!()
}

/// `video_scaler_sws_range()`: the swscale range (1 for full) for the `enum
/// video_range_type` value `range`.
pub fn sws_range(range: c_int) -> c_int {
    let _ = range;
    todo!()
}

/// `video_scaler_plane_heights()`: the height of each plane the destination
/// format has, from the plane of each of its four components
/// (`AVComponentDescriptor.plane`) and its vertical chroma shift
/// (`AVPixFmtDescriptor.log2_chroma_h`), which is read (called) once for each
/// of planes 1 and 2 the format has, as the C reads it. `None` for a plane
/// the format does not have; plane 0 is always `Some`.
pub fn plane_heights(
    height: u32,
    comp_planes: [c_int; 4],
    log2_chroma_h: impl FnMut() -> u8,
) -> [Option<c_int>; 4] {
    let _ = (height, comp_planes, log2_chroma_h);
    todo!()
}

/// How a scaled plane is copied to an output plane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaneCopy {
    /// The line sizes match: one copy of `size` bytes.
    Whole {
        /// The scaled line size times the height.
        size: usize,
    },
    /// One copy of `row` bytes per row, each line size apart.
    Rows {
        /// The smaller of the two line sizes.
        row: usize,
    },
}

/// `video_scaler_plane_copy()`: the copy of a scaled plane of `height` rows
/// to an output plane.
pub fn plane_copy(scaled_linesize: usize, plane_linesize: usize, height: usize) -> PlaneCopy {
    let _ = (scaled_linesize, plane_linesize, height);
    todo!()
}
