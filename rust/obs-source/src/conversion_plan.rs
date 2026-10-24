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
    pub const fn to_c(self) -> i32 {
        let _ = self;
        todo!()
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
    let _ = (format, full_range, trc);
    todo!()
}

/// `convert_video_format()` (`obs-internal.h`) for the formats that reach
/// the RGB limited plane (NONE, RGBA, BGRA and BGRX): `GS_RGBA16F` for PQ
/// and HLG, else `GS_RGBA` for RGBA, `GS_BGRA` for BGRA and `GS_BGRX` for
/// the rest. The inline's other formats never get here.
pub fn rgb_limited_format(format: i32, trc: u8) -> i32 {
    let _ = (format, trc);
    todo!()
}

/// `(n + 1) / 2` in `u32`: 0 at `u32::MAX`.
pub const fn half(n: u32) -> u32 {
    let _ = n;
    todo!()
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
    let _ = (convert_type, format, trc, width, height);
    todo!()
}

/// `select_conversion_technique()`: `None` for full-range RGB (where
/// `obs-source.c` asserts), P216, P416 and any value outside the enum.
pub fn technique(format: i32, full_range: bool, trc: u8) -> Option<&'static CStr> {
    let _ = (format, full_range, trc);
    todo!()
}

/// `need_linear_output()`: I010, P010, I210, I412 and YA2L.
pub fn linear(format: i32) -> bool {
    let _ = format;
    todo!()
}

/// `obs_source_conversion_plan_fill()`: the plan for a frame. `trc` is taken
/// modulo 256, as the frame's `uint8_t` field holds it.
pub fn plan(format: i32, full_range: bool, trc: i32, width: u32, height: u32) -> Plan {
    let _ = (format, full_range, trc, width, height);
    todo!()
}
