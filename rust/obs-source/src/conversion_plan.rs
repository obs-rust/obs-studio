//! Safe core of `libobs/obs-source-conversion-plan.c.h`: the async video GPU
//! conversion plan of `obs-source.c`.
//!
//! An async source's frame is uploaded to one to four textures and turned
//! into RGBA by a technique of `libobs/data/format_conversion.effect`.
//! [`plan`] says how, from the frame's format, range, transfer and size:
//! - the convert type ([`convert_type`], `get_convert_type()`);
//! - the planes the frame is uploaded to: their count, sizes and texture
//!   formats ([`planes`], the `set_*_sizes()` of `init_gpu_conversion()`);
//!   the RGB limited plane takes `convert_video_format()`'s format
//!   ([`rgb_limited_format`]);
//! - the technique ([`technique`], `select_conversion_technique()`), a
//!   `&'static CStr` or `None`;
//! - whether the conversion renders into a linear sRGB framebuffer
//!   ([`linear`], `need_linear_output()`).
//!
//! `obs-source.c.h` keeps `set_async_texture_size()` (the change check, the
//! textures and texrenders, the graphics context), `upload_raw_frame()`,
//! `update_async_texrender()` and its shader parameters, the
//! `convert_video_format()` inline of `obs-internal.h` for its other callers,
//! and the two asserts for a frame that needs no conversion. It copies the
//! plan into the source's `async_convert_width`, `async_convert_height`,
//! `async_texture_formats` and `async_channel_count`.
//!
//! Frame formats and sizes come from plugins, capture devices and decoded
//! media, so any value can arrive. Formats and transfers are the C's raw
//! `int`s: a value outside the enum takes the C's `default:` path, and the
//! transfer is the frame's `uint8_t` field, so [`plan`] takes it modulo 256.
//! The sizes are `u32` arithmetic that wraps as the C's does near
//! `u32::MAX` (`(width + 1) / 2`, `width * 3`, `(width + 5) / 6 * 4`):
//! characterized, not endorsed (`test_async_conversion_plan`). Nothing here
//! panics, allocates, keeps state or takes a lock.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::as_conversions,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

use core::ffi::CStr;

/// `OBS_SOURCE_CONVERSION_PLAN_PLANES`: a plan's planes.
pub const PLANES: usize = 4;

/// `enum video_format` (`media-io/video-io.h`), as the C's `int`.
pub mod video_format {
    pub const NONE: i32 = 0;
    pub const I420: i32 = 1;
    pub const NV12: i32 = 2;
    pub const YVYU: i32 = 3;
    pub const YUY2: i32 = 4;
    pub const UYVY: i32 = 5;
    pub const RGBA: i32 = 6;
    pub const BGRA: i32 = 7;
    pub const BGRX: i32 = 8;
    pub const Y800: i32 = 9;
    pub const I444: i32 = 10;
    pub const BGR3: i32 = 11;
    pub const I422: i32 = 12;
    pub const I40A: i32 = 13;
    pub const I42A: i32 = 14;
    pub const YUVA: i32 = 15;
    pub const AYUV: i32 = 16;
    pub const I010: i32 = 17;
    pub const P010: i32 = 18;
    pub const I210: i32 = 19;
    pub const I412: i32 = 20;
    pub const YA2L: i32 = 21;
    pub const P216: i32 = 22;
    pub const P416: i32 = 23;
    pub const V210: i32 = 24;
    pub const R10L: i32 = 25;
}

/// `enum video_trc` (`media-io/video-io.h`), as the frame's `uint8_t`.
pub mod video_trc {
    pub const DEFAULT: u8 = 0;
    pub const SRGB: u8 = 1;
    pub const PQ: u8 = 2;
    pub const HLG: u8 = 3;
}

/// The `enum gs_color_format` values (`graphics/graphics.h`) a plan uses,
/// as the C's `int`.
pub mod gs_color_format {
    pub const UNKNOWN: i32 = 0;
    pub const R8: i32 = 2;
    pub const RGBA: i32 = 3;
    pub const BGRX: i32 = 4;
    pub const BGRA: i32 = 5;
    pub const R10G10B10A2: i32 = 6;
    pub const R16: i32 = 8;
    pub const RGBA16F: i32 = 9;
    pub const R8G8: i32 = 18;
    pub const BGRA_UNORM: i32 = 21;
    pub const RG16: i32 = 22;
}

/// `enum obs_source_convert_type` (`obs-source-conversion-plan.h`), in
/// header order; the discriminants are the C's values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConvertType {
    None = 0,
    Nv12 = 1,
    I420 = 2,
    I420Pq = 3,
    I420A = 4,
    I422 = 5,
    I422P10le = 6,
    I422A = 7,
    I422Pack = 8,
    I444 = 9,
    I444P12le = 10,
    I444A = 11,
    I444P12leA = 12,
    I444APack = 13,
    Y800 = 14,
    RgbLimited = 15,
    Bgr3 = 16,
    I010 = 17,
    P010 = 18,
    V210 = 19,
    R10l = 20,
}

impl ConvertType {
    /// Every variant, in header order.
    pub const ALL: [Self; 21] = [
        Self::None,
        Self::Nv12,
        Self::I420,
        Self::I420Pq,
        Self::I420A,
        Self::I422,
        Self::I422P10le,
        Self::I422A,
        Self::I422Pack,
        Self::I444,
        Self::I444P12le,
        Self::I444A,
        Self::I444P12leA,
        Self::I444APack,
        Self::Y800,
        Self::RgbLimited,
        Self::Bgr3,
        Self::I010,
        Self::P010,
        Self::V210,
        Self::R10l,
    ];

    /// The C value of this convert type.
    #[allow(clippy::as_conversions)] // a fieldless enum's discriminant
    pub const fn to_c(self) -> i32 {
        self as i32
    }
}

/// One plane: its texture's size and `enum gs_color_format`.
/// [`Plane::NONE`] (all zero, `GS_UNKNOWN`) is no plane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Plane {
    pub width: u32,
    pub height: u32,
    pub format: i32,
}

impl Plane {
    /// No plane.
    pub const NONE: Self = Self {
        width: 0,
        height: 0,
        format: gs_color_format::UNKNOWN,
    };

    const fn new(width: u32, height: u32, format: i32) -> Self {
        Self {
            width,
            height,
            format,
        }
    }
}

/// `struct obs_source_conversion_plan`: the plan for one frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Plan {
    pub convert_type: ConvertType,
    /// `async_channel_count`: the planes set, 0 for [`ConvertType::None`].
    pub channel_count: usize,
    /// The planes; those from `channel_count` on are [`Plane::NONE`].
    pub planes: [Plane; PLANES],
    /// A technique of `format_conversion.effect`.
    pub technique: Option<&'static CStr>,
    /// Render into a linear sRGB framebuffer.
    pub linear: bool,
}

/// `get_convert_type()`.
pub fn convert_type(format: i32, full_range: bool, trc: u8) -> ConvertType {
    use video_format as f;
    match format {
        f::I420 => {
            if trc == video_trc::PQ {
                ConvertType::I420Pq
            } else {
                ConvertType::I420
            }
        }
        f::NV12 => ConvertType::Nv12,
        f::I444 => ConvertType::I444,
        f::I412 => ConvertType::I444P12le,
        f::I422 => ConvertType::I422,
        f::I210 => ConvertType::I422P10le,
        f::YVYU | f::YUY2 | f::UYVY => ConvertType::I422Pack,
        f::Y800 => ConvertType::Y800,
        f::NONE | f::RGBA | f::BGRA | f::BGRX => {
            if full_range {
                ConvertType::None
            } else {
                ConvertType::RgbLimited
            }
        }
        f::BGR3 => ConvertType::Bgr3,
        f::I40A => ConvertType::I420A,
        f::I42A => ConvertType::I422A,
        f::YUVA => ConvertType::I444A,
        f::YA2L => ConvertType::I444P12leA,
        f::AYUV => ConvertType::I444APack,
        f::I010 => ConvertType::I010,
        f::P010 => ConvertType::P010,
        f::V210 => ConvertType::V210,
        f::R10L => ConvertType::R10l,
        // P216 and P416 are unimplemented; any other value is outside the
        // enum
        _ => ConvertType::None,
    }
}

/// `convert_video_format()` (`obs-internal.h`) for the formats that reach
/// the RGB limited plane (NONE, RGBA, BGRA and BGRX): `GS_RGBA16F` for PQ
/// and HLG, else `GS_RGBA` for RGBA, `GS_BGRA` for BGRA and `GS_BGRX` for
/// the rest. The inline's other formats never get here.
pub fn rgb_limited_format(format: i32, trc: u8) -> i32 {
    match trc {
        video_trc::PQ | video_trc::HLG => gs_color_format::RGBA16F,
        _ => match format {
            video_format::RGBA => gs_color_format::RGBA,
            video_format::BGRA => gs_color_format::BGRA,
            _ => gs_color_format::BGRX,
        },
    }
}

/// `(n + 1) / 2` in `u32`: 0 at `u32::MAX`.
pub const fn half(n: u32) -> u32 {
    n.wrapping_add(1).wrapping_div(2)
}

/// The planes of a convert type and their count: the `set_*_sizes()` of
/// `init_gpu_conversion()`. `format` and `trc` matter only to
/// [`ConvertType::RgbLimited`] ([`rgb_limited_format`]).
pub fn planes(
    convert_type: ConvertType,
    format: i32,
    trc: u8,
    width: u32,
    height: u32,
) -> (usize, [Plane; PLANES]) {
    use gs_color_format as gs;
    const N: Plane = Plane::NONE;
    let full = |format| Plane::new(width, height, format);
    let half_width = half(width);
    let half_height = half(height);
    match convert_type {
        ConvertType::I422Pack => (1, [Plane::new(half_width, height, gs::BGRA), N, N, N]),
        ConvertType::I420 | ConvertType::I420Pq => {
            let chroma = Plane::new(half_width, half_height, gs::R8);
            (3, [full(gs::R8), chroma, chroma, N])
        }
        ConvertType::I422 => {
            let chroma = Plane::new(half_width, height, gs::R8);
            (3, [full(gs::R8), chroma, chroma, N])
        }
        ConvertType::I422P10le => {
            let chroma = Plane::new(half_width, height, gs::R16);
            (3, [full(gs::R16), chroma, chroma, N])
        }
        ConvertType::Nv12 => (
            2,
            [
                full(gs::R8),
                Plane::new(half_width, half_height, gs::R8G8),
                N,
                N,
            ],
        ),
        ConvertType::I444 => (3, [full(gs::R8), full(gs::R8), full(gs::R8), N]),
        ConvertType::I444P12le => (3, [full(gs::R16), full(gs::R16), full(gs::R16), N]),
        ConvertType::Y800 => (1, [full(gs::R8), N, N, N]),
        ConvertType::RgbLimited => (1, [full(rgb_limited_format(format, trc)), N, N, N]),
        ConvertType::Bgr3 => (
            1,
            [Plane::new(width.wrapping_mul(3), height, gs::R8), N, N, N],
        ),
        ConvertType::I420A => {
            let chroma = Plane::new(half_width, half_height, gs::R8);
            (4, [full(gs::R8), chroma, chroma, full(gs::R8)])
        }
        ConvertType::I422A => {
            let chroma = Plane::new(half_width, height, gs::R8);
            (4, [full(gs::R8), chroma, chroma, full(gs::R8)])
        }
        ConvertType::I444A => (4, [full(gs::R8); PLANES]),
        ConvertType::I444P12leA => (4, [full(gs::R16); PLANES]),
        ConvertType::I444APack => (1, [full(gs::BGRA), N, N, N]),
        ConvertType::I010 => {
            let chroma = Plane::new(half_width, half_height, gs::R16);
            (3, [full(gs::R16), chroma, chroma, N])
        }
        ConvertType::P010 => (
            2,
            [
                full(gs::R16),
                Plane::new(half_width, half_height, gs::RG16),
                N,
                N,
            ],
        ),
        ConvertType::V210 => {
            let adjusted_width = width.wrapping_add(5).wrapping_div(6).wrapping_mul(4);
            (
                1,
                [Plane::new(adjusted_width, height, gs::R10G10B10A2), N, N, N],
            )
        }
        ConvertType::R10l => (1, [full(gs::BGRA_UNORM), N, N, N]),
        // obs-source.c asserts
        ConvertType::None => (0, [N; PLANES]),
    }
}

/// One technique per transfer: PQ, HLG, and any other.
const fn by_trc(
    trc: u8,
    pq: &'static CStr,
    hlg: &'static CStr,
    other: &'static CStr,
) -> &'static CStr {
    match trc {
        video_trc::PQ => pq,
        video_trc::HLG => hlg,
        _ => other,
    }
}

/// `select_conversion_technique()`: `None` for full-range RGB (where
/// `obs-source.c` asserts), P216, P416 and any value outside the enum.
pub fn technique(format: i32, full_range: bool, trc: u8) -> Option<&'static CStr> {
    use video_format as f;
    let range = |full: &'static CStr, limited: &'static CStr| {
        if full_range { full } else { limited }
    };
    Some(match format {
        f::UYVY => c"UYVY_Reverse",
        f::YUY2 => by_trc(
            trc,
            c"YUY2_PQ_Reverse",
            c"YUY2_HLG_Reverse",
            c"YUY2_Reverse",
        ),
        f::YVYU => c"YVYU_Reverse",
        f::I420 => by_trc(
            trc,
            c"I420_PQ_Reverse",
            c"I420_HLG_Reverse",
            c"I420_Reverse",
        ),
        f::NV12 => by_trc(
            trc,
            c"NV12_PQ_Reverse",
            c"NV12_HLG_Reverse",
            c"NV12_Reverse",
        ),
        f::I444 => c"I444_Reverse",
        f::I412 => by_trc(
            trc,
            c"I412_PQ_Reverse",
            c"I412_HLG_Reverse",
            c"I412_Reverse",
        ),
        f::Y800 => range(c"Y800_Full", c"Y800_Limited"),
        f::BGR3 => range(c"BGR3_Full", c"BGR3_Limited"),
        f::I422 => c"I422_Reverse",
        f::I210 => by_trc(
            trc,
            c"I210_PQ_Reverse",
            c"I210_HLG_Reverse",
            c"I210_Reverse",
        ),
        f::I40A => c"I40A_Reverse",
        f::I42A => c"I42A_Reverse",
        f::YUVA => c"YUVA_Reverse",
        f::YA2L => c"YA2L_Reverse",
        f::AYUV => c"AYUV_Reverse",
        f::I010 => by_trc(
            trc,
            c"I010_PQ_2020_709_Reverse",
            c"I010_HLG_2020_709_Reverse",
            c"I010_SRGB_Reverse",
        ),
        f::P010 => by_trc(
            trc,
            c"P010_PQ_2020_709_Reverse",
            c"P010_HLG_2020_709_Reverse",
            c"P010_SRGB_Reverse",
        ),
        f::V210 => by_trc(
            trc,
            c"V210_PQ_2020_709_Reverse",
            c"V210_HLG_2020_709_Reverse",
            c"V210_SRGB_Reverse",
        ),
        f::R10L => by_trc(
            trc,
            range(
                c"R10L_PQ_2020_709_Full_Reverse",
                c"R10L_PQ_2020_709_Limited_Reverse",
            ),
            range(
                c"R10L_HLG_2020_709_Full_Reverse",
                c"R10L_HLG_2020_709_Limited_Reverse",
            ),
            range(c"R10L_SRGB_Full_Reverse", c"R10L_SRGB_Limited_Reverse"),
        ),
        f::BGRA | f::BGRX | f::RGBA | f::NONE => {
            if full_range {
                // obs-source.c asserts
                return None;
            }
            c"RGB_Limited"
        }
        // P216 and P416 are unimplemented; any other value is outside the
        // enum
        _ => return None,
    })
}

/// `need_linear_output()`: I010, P010, I210, I412 and YA2L.
pub fn linear(format: i32) -> bool {
    use video_format as f;
    matches!(format, f::I010 | f::P010 | f::I210 | f::I412 | f::YA2L)
}

/// `obs_source_conversion_plan_fill()`: the plan for a frame. `trc` is taken
/// modulo 256, as the frame's `uint8_t` field holds it.
pub fn plan(format: i32, full_range: bool, trc: i32, width: u32, height: u32) -> Plan {
    let [trc, ..] = trc.to_le_bytes();
    let convert_type = convert_type(format, full_range, trc);
    let (channel_count, planes) = planes(convert_type, format, trc, width, height);
    Plan {
        convert_type,
        channel_count,
        planes,
        technique: technique(format, full_range, trc),
        linear: linear(format),
    }
}
