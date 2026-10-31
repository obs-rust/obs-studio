//! Safe core of `libobs/graphics/graphics-ffmpeg-host-pq.c.h`: the JPEG XR
//! PQ-to-scRGB conversion of the image loader's Windows host
//! (`graphics-ffmpeg-host-windows.c`), which WIC uses for the PQ (HDR10)
//! JPEG XR images FFmpeg cannot decode, such as Windows HDR screenshots, in
//! the Image Source, the Image Slide Show and the other users of
//! `gs_image_file`.
//!
//! - [`pq_to_linear`] (`pq_to_linear()`): the SMPTE ST 2084 inverse EOTF,
//!   from a code value in 0 to 1 to linear light, 1 being 10 000 nits;
//! - [`half_from_f32`] (`half.h`'s `half_from_float()`): an `f32` to the
//!   bits of the nearest half, ties to even;
//! - [`pq10_to_scrgb`] (`convert_pq_to_cccs()`): each 4-byte
//!   `32bppBGR101010` texel (blue in bits 0-9, green in 10-19, red in
//!   20-29, bits 30-31 unused) through the curve, the BT.2020-to-709 matrix
//!   and a scale of 125 (10 000 nits over the 80-nit scRGB reference) into
//!   four native-endian halves, alpha 1.0.
//!
//! `graphics-ffmpeg-host-windows.c` keeps every WIC call, the pixel-format
//! test, the sizes, both allocations and the stores afterwards. These keep
//! no state and call nothing but `f32::powf`, which calls the platform's
//! C library `powf` as the C does.
//!
//! The pixels come from the image file, so nothing here can panic on any
//! input.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable
)]

/// Bytes of one `32bppBGR101010` source texel.
pub const SRC_TEXEL_BYTES: usize = 4;

/// Bytes of one destination texel: four halves.
pub const DST_TEXEL_BYTES: usize = 8;

/// The bits of the half 1.0, every texel's alpha.
pub const ALPHA_ONE: u16 = 0x3c00;

/// `pq_to_linear()`: the SMPTE ST 2084 inverse EOTF of the code value `u`
/// (0 to 1), as linear light (1 is 10 000 nits).
#[must_use]
pub fn pq_to_linear(u: f32) -> f32 {
    let _ = u;
    todo!("pq_to_linear")
}

/// `half_from_float()`: the bits of the half nearest `f`, ties to even;
/// infinity past the largest half, and the quiet NaN `0x7fff` (with `f`'s
/// sign) for any NaN.
#[must_use]
pub fn half_from_f32(f: f32) -> u16 {
    let _ = f;
    todo!("half_from_f32")
}

/// `convert_pq_to_cccs()`: converts each 4-byte texel of `src` into one
/// 8-byte texel of `dst`, in order, in native byte order. Stops at the end
/// of whichever runs out first, and leaves the rest of `dst` and a short
/// tail of `src` alone.
pub fn pq10_to_scrgb(dst: &mut [u8], src: &[u8]) {
    let _ = (dst, src);
    todo!("pq10_to_scrgb")
}
