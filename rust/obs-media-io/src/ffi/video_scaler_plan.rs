//! C ABI for `libobs/media-io/video-scaler-plan.c.h`.
//!
//! `video-scaler-ffmpeg.c` calls these inside libobs. The C file has no
//! `EXPORT`, so the symbols stay hidden (`rust-hidden.d`). The five
//! functions keep the C signatures: `enum video_scale_type`, `enum
//! video_colorspace` and `enum video_range_type` cross as `int` (as everywhere
//! in this crate). A null pointer, which the C dereferences, is not read or
//! written: no heights are written without the heights and the component
//! planes, a null shift reads as 0, and a null size is not written;
//! `video-scaler-ffmpeg.c` always passes the scaler's own array, its
//! descriptor and its locals.

use core::ffi::c_int;

use crate::video_scaler_plan::{
    PlaneCopy, plane_copy, plane_heights, sws_colorspace, sws_flags, sws_range,
};

/// Same contract as C `video_scaler_sws_flags`.
#[unsafe(no_mangle)]
pub extern "C" fn video_scaler_sws_flags(scale_type: c_int) -> c_int {
    sws_flags(scale_type)
}

/// Same contract as C `video_scaler_sws_colorspace`.
#[unsafe(no_mangle)]
pub extern "C" fn video_scaler_sws_colorspace(color_space: c_int) -> c_int {
    sws_colorspace(color_space)
}

/// Same contract as C `video_scaler_sws_range`.
#[unsafe(no_mangle)]
pub extern "C" fn video_scaler_sws_range(range: c_int) -> c_int {
    sws_range(range)
}

/// Same contract as C `video_scaler_plane_heights`.
///
/// # Safety
///
/// `heights` is null or points to 4 `int`s the caller may write;
/// `comp_planes` is null or points to 4 readable `int`s; `log2_chroma_h` is
/// null or readable whenever a component is on plane 1 or 2.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn video_scaler_plane_heights(
    heights: *mut [c_int; 4],
    height: u32,
    comp_planes: *const [c_int; 4],
    log2_chroma_h: *const u8,
) {
    // SAFETY: null or 4 writable ints, as the caller promises.
    let Some(heights) = (unsafe { heights.as_mut() }) else {
        return;
    };
    // SAFETY: null or 4 readable ints, as the caller promises.
    let Some(&comp_planes) = (unsafe { comp_planes.as_ref() }) else {
        return;
    };
    // SAFETY: null or readable whenever the core asks (a format with plane 1
    // or 2), as the caller promises.
    let shift = || unsafe { log2_chroma_h.as_ref() }.copied().unwrap_or(0);
    for (slot, h) in heights
        .iter_mut()
        .zip(plane_heights(height, comp_planes, shift))
    {
        if let Some(h) = h {
            *slot = h;
        }
    }
}

/// Same contract as C `video_scaler_plane_copy`.
///
/// # Safety
///
/// `size` is null or points to a `size_t` the caller may write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn video_scaler_plane_copy(
    scaled_linesize: usize,
    plane_linesize: usize,
    height: usize,
    size: *mut usize,
) -> bool {
    let (whole, bytes) = match plane_copy(scaled_linesize, plane_linesize, height) {
        PlaneCopy::Whole { size } => (true, size),
        PlaneCopy::Rows { row } => (false, row),
    };
    // SAFETY: null or a writable size_t, as the caller promises.
    if let Some(out) = unsafe { size.as_mut() } {
        *out = bytes;
    }
    whole
}
