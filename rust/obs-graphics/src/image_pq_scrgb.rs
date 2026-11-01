//! Safe core of `libobs/graphics/graphics-ffmpeg-host-pq.c.h` and
//! `libobs/graphics/graphics-ffmpeg-host-wic-checks.c.h`: the JPEG XR
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
//!   four native-endian halves, alpha 1.0;
//! - [`fallback_refuses`] (`gs_image_host_fallback_refuses()`): whether
//!   `gs_image_host_fallback_load()` refuses a name before it tries WIC;
//! - [`wic_kind`] (`gs_image_wic_kind()`): what `wic_image_init_internal()`
//!   does with a decoded frame's pixel format ([`WicKind`]).
//!
//! `graphics-ffmpeg-host-windows.c` keeps every WIC call, the name's
//! `strlen()`, both pixel-format GUID compares, the sizes, both
//! allocations, the stores afterwards and every log line. These keep
//! no state and call nothing but `f32::powf`, which calls the platform's
//! C library `powf` as the C does.
//!
//! The pixels and the name come from the user's files, so nothing here can
//! panic on any input. [`pq10_to_scrgb`] works on slices: it converts as many whole
//! texels as both hold.
//!
//! Intentional difference: `half_from_float()` shifts a `uint32_t` by
//! `113 - exponent`, which is 32 or more for a float below 2^-45 in
//! magnitude (biased exponent 81 or less). C leaves that undefined, and x86
//! masks the shift into a non-zero half. [`half_from_f32`] gives zero with
//! the float's sign there, which is what the rounding gives for every shift
//! from 24 to 31. A texel's 709 value reaches it when it lands that close to
//! zero without being zero.
//!
//! Intentional difference: for a name shorter than 4 bytes,
//! `gs_image_host_fallback_refuses()` compares from before the start of the
//! name. [`fallback_refuses`] refuses those names; none can end in `.jxr`.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
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
#[allow(
    clippy::excessive_precision,
    reason = "the C's literals, spelled as the C spells them; each rounds to the same f32"
)]
pub fn pq_to_linear(u: f32) -> f32 {
    // The C's constants and its order of operations: 1 / m2, c1, c2, c3 and
    // 1 / m1 as f32 divisions and literals.
    let common = u.powf(1.0_f32 / 78.843_75_f32);
    let numerator = common - 0.835_937_5_f32;
    // windows.h's max(numerator, 0.f): a NaN gives 0, as there.
    let clamped = if numerator > 0.0 { numerator } else { 0.0 };
    (clamped / (18.851_562_5_f32 - 18.6875_f32 * common))
        .abs()
        .powf(1.0_f32 / 0.159_301_757_8_f32)
}

/// `half_from_float()`: the bits of the half nearest `f`, ties to even;
/// infinity past the largest half, and the quiet NaN `0x7fff` (with `f`'s
/// sign) for any NaN.
#[must_use]
pub fn half_from_f32(f: f32) -> u16 {
    let bits = f.to_bits();
    let sign = (bits & 0x8000_0000) >> 16;
    let mut value = bits & 0x7fff_ffff;
    let result = if value > 0x477f_e000 {
        // Too large for a half: infinity, or NaN for a NaN.
        if value & 0x7f80_0000 == 0x7f80_0000 && value & 0x007f_ffff != 0 {
            0x7fff
        } else {
            0x7c00
        }
    } else if value == 0 {
        0
    } else {
        if value < 0x3880_0000 {
            // Too small for a normal half: denormalize. A shift of 32 or
            // more leaves nothing (the intentional difference).
            let shift = 113_u32.wrapping_sub(value >> 23);
            value = (0x0080_0000 | (value & 0x007f_ffff))
                .checked_shr(shift)
                .unwrap_or(0);
        } else {
            // Rebias the exponent.
            value = value.wrapping_add(0xc800_0000);
        }
        (value.wrapping_add(0x0fff).wrapping_add((value >> 13) & 1) >> 13) & 0x7fff
    };
    // Sign in bit 15 and at most 15 bits of result: always fits.
    u16::try_from(result | sign).unwrap_or(u16::MAX)
}

/// One 10-bit channel of a texel as the C reads it: `(float)c / 1023.f`.
fn channel(rgb10: u32, shift: u32) -> f32 {
    let code = u16::try_from((rgb10 >> shift) & 0x3ff).unwrap_or(0);
    f32::from(code) / 1023.0
}

/// The four halves of one texel, in native byte order.
#[allow(
    clippy::excessive_precision,
    reason = "the C's literals, spelled as the C spells them; each rounds to the same f32"
)]
fn convert_texel(rgb10: u32) -> [u8; DST_TEXEL_BYTES] {
    let red2020 = pq_to_linear(channel(rgb10, 20));
    let green2020 = pq_to_linear(channel(rgb10, 10));
    let blue2020 = pq_to_linear(channel(rgb10, 0));
    // The BT.2020-to-709 matrix, each row evaluated left to right as the
    // C does (Rust never fuses a multiply and an add).
    let red709 = 1.660_491_002_108_434_5_f32 * red2020
        - 0.587_641_138_788_549_51_f32 * green2020
        - 0.072_849_863_319_884_883_f32 * blue2020;
    let green709 = -0.124_550_474_521_590_74_f32 * red2020
        + 1.132_899_897_125_960_3_f32 * green2020
        - 0.008_349_422_604_369_476_8_f32 * blue2020;
    let blue709 = -0.018_150_763_354_905_303_f32 * red2020
        - 0.100_578_898_008_007_39_f32 * green2020
        + 1.118_729_661_362_912_7_f32 * blue2020;
    let halves = [
        half_from_f32(red709 * 125.0),
        half_from_f32(green709 * 125.0),
        half_from_f32(blue709 * 125.0),
        ALPHA_ONE,
    ];
    let mut out = [0_u8; DST_TEXEL_BYTES];
    for (bytes, half) in out.as_chunks_mut::<2>().0.iter_mut().zip(halves) {
        *bytes = half.to_ne_bytes();
    }
    out
}

/// `convert_pq_to_cccs()`: converts each 4-byte texel of `src` into one
/// 8-byte texel of `dst`, in order, in native byte order. Stops at the end
/// of whichever runs out first, and leaves the rest of `dst` and a short
/// tail of `src` alone.
pub fn pq10_to_scrgb(dst: &mut [u8], src: &[u8]) {
    let (dst_texels, _) = dst.as_chunks_mut::<DST_TEXEL_BYTES>();
    let (src_texels, _) = src.as_chunks::<SRC_TEXEL_BYTES>();
    for (out, input) in dst_texels.iter_mut().zip(src_texels) {
        *out = convert_texel(u32::from_ne_bytes(*input));
    }
}

/// The name `gs_image_host_fallback_load()` sends to WIC ends in this, in
/// any ASCII case.
pub const JXR_SUFFIX: &[u8; 4] = b".jxr";

/// `enum gs_image_wic_kind`: what `wic_image_init_internal()` does with a
/// decoded frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum WicKind {
    /// `GS_IMAGE_WIC_UNSUPPORTED`: any other pixel format, logged, and the
    /// image fails.
    Unsupported = 0,
    /// `GS_IMAGE_WIC_COPY`: `GUID_WICPixelFormat64bppRGBAHalf`, copied as
    /// it is.
    Copy = 1,
    /// `GS_IMAGE_WIC_PQ10`: `GUID_WICPixelFormat32bppBGR101010`, through
    /// [`pq10_to_scrgb`].
    Pq10 = 2,
}

/// `gs_image_host_fallback_refuses()`: whether
/// `gs_image_host_fallback_load()` refuses the file `name` (its bytes, no
/// terminator) before it tries WIC.
///
/// The C tests `len <= 4 && astrcmpi(file + len - 4, ".jxr") != 0`, so only
/// a name of exactly 4 bytes is compared with [`JXR_SUFFIX`], in any ASCII
/// case (`astrcmpi()`'s `toupper()` in the C locale), and every longer name
/// goes to WIC (characterized, not endorsed). A name shorter than 4 bytes,
/// which the C compares from before its start, is refused: none can end in
/// `.jxr` (the intentional difference).
#[must_use]
pub fn fallback_refuses(name: &[u8]) -> bool {
    match name.len() {
        0..4 => true,
        4 => !name.eq_ignore_ascii_case(JXR_SUFFIX),
        _ => false,
    }
}

/// `gs_image_wic_kind()`: the kind for a frame whose pixel format is
/// `GUID_WICPixelFormat64bppRGBAHalf` (`scrgb`) or
/// `GUID_WICPixelFormat32bppBGR101010` (`pq10`).
/// `pq10` wins when both are set, which two different GUIDs never are.
#[must_use]
pub fn wic_kind(scrgb: bool, pq10: bool) -> WicKind {
    if scrgb || pq10 {
        if pq10 { WicKind::Pq10 } else { WicKind::Copy }
    } else {
        WicKind::Unsupported
    }
}
