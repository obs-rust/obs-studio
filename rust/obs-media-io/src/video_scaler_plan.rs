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
//!
//! Intentional difference from the C: a shifted chroma plane's height rounds
//! up (`AV_CEIL_RSHIFT`), as `av_image_alloc()` and `sws_scale()` size the
//! scaled plane and `video_frame_init()` allocates the output's. The C
//! rounded down, so at an odd height a 4:2:0 output's last chroma row was
//! never written.

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

use core::ffi::c_int;

use crate::video_io::{
    VideoColorspace, VideoRangeType, VideoScaleType, video_colorspace_from_c,
    video_range_type_from_c, video_scale_type_from_c,
};

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
    match video_scale_type_from_c(scale_type) {
        Some(VideoScaleType::Default | VideoScaleType::FastBilinear) => sws::FAST_BILINEAR,
        Some(VideoScaleType::Point) | None => sws::POINT,
        Some(VideoScaleType::Bilinear) => sws::BILINEAR | sws::AREA,
        Some(VideoScaleType::Bicubic) => sws::BICUBIC,
    }
}

/// `video_scaler_sws_colorspace()`: the swscale coefficient set for the `enum
/// video_colorspace` value `color_space`.
pub fn sws_colorspace(color_space: c_int) -> c_int {
    match video_colorspace_from_c(color_space) {
        Some(VideoColorspace::Cs601) => sws::CS_ITU601,
        Some(VideoColorspace::Cs2100Pq | VideoColorspace::Cs2100Hlg) => sws::CS_BT2020,
        Some(VideoColorspace::Default | VideoColorspace::Cs709 | VideoColorspace::Srgb) | None => {
            sws::CS_ITU709
        }
    }
}

/// `video_scaler_sws_range()`: the swscale range (1 for full) for the `enum
/// video_range_type` value `range`.
pub fn sws_range(range: c_int) -> c_int {
    match video_range_type_from_c(range) {
        Some(VideoRangeType::Full) => 1,
        Some(VideoRangeType::Default | VideoRangeType::Partial) | None => 0,
    }
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
    mut log2_chroma_h: impl FnMut() -> u8,
) -> [Option<c_int>; 4] {
    // A plane number outside 0 to 3 is ignored (the C would index past
    // has_plane[]; no libavutil descriptor has one).
    let mut has_plane = [false; 4];
    for plane in comp_planes {
        if let Some(slot) = usize::try_from(plane)
            .ok()
            .and_then(|plane| has_plane.get_mut(plane))
        {
            *slot = true;
        }
    }

    let mut heights = [None; 4];
    for (plane, (slot, has)) in heights.iter_mut().zip(has_plane).enumerate() {
        if plane == 0 {
            *slot = Some(to_int(height));
        } else if has {
            let shift = if plane == 1 || plane == 2 {
                log2_chroma_h()
            } else {
                0
            };
            *slot = Some(to_int(ceil_rshift(height, shift)));
        }
    }
    heights
}

/// `AV_CEIL_RSHIFT(height, shift)`: `height / 2^shift`, rounded up. A shift
/// of 32 or more (libavutil's largest is 2) leaves 1 of any height but 0.
fn ceil_rshift(height: u32, shift: u8) -> u32 {
    let shift = u32::from(shift);
    if shift >= u32::BITS {
        return u32::from(height != 0);
    }
    // (height + 2^shift - 1) >> shift, in 64 bits so the sum cannot wrap
    let round = 1u64.wrapping_shl(shift).wrapping_sub(1);
    let up = u64::from(height).wrapping_add(round).wrapping_shr(shift);
    u32::try_from(up).unwrap_or(u32::MAX)
}

/// The C's `(int)` cast of a `uint32_t`: the same bits.
fn to_int(value: u32) -> c_int {
    c_int::from_ne_bytes(value.to_ne_bytes())
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
    if scaled_linesize == plane_linesize {
        // size_t wraps
        PlaneCopy::Whole {
            size: scaled_linesize.wrapping_mul(height),
        }
    } else {
        PlaneCopy::Rows {
            row: scaled_linesize.min(plane_linesize),
        }
    }
}
