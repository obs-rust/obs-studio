//! Safe core of `libobs/obs-video-plan.c.h`: the video output plan.
//!
//! `obs_reset_video()` checks the base and output sizes with
//! [`size_valid`] and [`output_size_valid`], aligns the output size with
//! [`align_output_size`] and logs the scale type by [`scale_type_name`].
//! For each video mix, `obs.c` then asks [`output_plan`] what the output
//! format needs: the GPU conversion (the techniques of
//! `libobs/data/format_conversion.effect` for each plane and the texel sizes
//! they get), the size and texture format of each conversion plane (the
//! conversion textures and the copy surfaces), and the format and color
//! space of the render textures. `obs.c` creates the textures from it and
//! copies the rest into the mix.
//!
//! The module is pure: no graphics device and no libobs state. The sizes
//! come from the profile (`basic.ini`), obs-websocket `SetVideoSettings` and
//! scripts, so nothing here panics, indexes or overflows for any input. A
//! zero size gives an infinite texel size, as the C's `1.f / (float)0`
//! does; `obs_reset_video()` refuses those sizes before it asks for a plan,
//! but a canvas's mix is not checked.
//!
//! A format or color space outside its enum is `None`, the C's `default:`
//! branch: no conversion, and the SDR texture format and space.

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

use core::ffi::{CStr, c_int};

use obs_media_io::video_io::{VideoColorspace, VideoFormat};

/// `OBS_SIZE_MIN`: the smallest width or height `obs_reset_video()` takes.
pub const SIZE_MIN: u32 = 2;
/// `OBS_SIZE_MAX`: the largest width or height `obs_reset_video()` takes.
pub const SIZE_MAX: u32 = 32 * 1024;

/// `NUM_CHANNELS` of `obs-internal.h`: the planes of a conversion.
pub const NUM_CHANNELS: usize = 3;

/// `enum gs_color_format` (`libobs/graphics/graphics.h`), in header order.
/// The discriminants are the header's values; the C ABI passes the enum as
/// a `c_int`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GsColorFormat {
    Unknown = 0,
    A8 = 1,
    R8 = 2,
    Rgba = 3,
    Bgrx = 4,
    Bgra = 5,
    R10g10b10a2 = 6,
    Rgba16 = 7,
    R16 = 8,
    Rgba16f = 9,
    Rgba32f = 10,
    Rg16f = 11,
    Rg32f = 12,
    R16f = 13,
    R32f = 14,
    Dxt1 = 15,
    Dxt3 = 16,
    Dxt5 = 17,
    R8g8 = 18,
    RgbaUnorm = 19,
    BgrxUnorm = 20,
    BgraUnorm = 21,
    Rg16 = 22,
}

impl GsColorFormat {
    /// Every variant, in header order.
    pub const ALL: [Self; 23] = [
        Self::Unknown,
        Self::A8,
        Self::R8,
        Self::Rgba,
        Self::Bgrx,
        Self::Bgra,
        Self::R10g10b10a2,
        Self::Rgba16,
        Self::R16,
        Self::Rgba16f,
        Self::Rgba32f,
        Self::Rg16f,
        Self::Rg32f,
        Self::R16f,
        Self::R32f,
        Self::Dxt1,
        Self::Dxt3,
        Self::Dxt5,
        Self::R8g8,
        Self::RgbaUnorm,
        Self::BgrxUnorm,
        Self::BgraUnorm,
        Self::Rg16,
    ];

    /// The C value of this format.
    pub const fn to_c(self) -> c_int {
        self as c_int
    }
}

/// `enum gs_color_space` (`libobs/graphics/graphics.h`), in header order.
/// The discriminants are the header's values; the C ABI passes the enum as
/// a `c_int`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GsColorSpace {
    /// SDR.
    Srgb = 0,
    /// High-precision SDR.
    Srgb16f = 1,
    /// Canvas, Mac EDR (HDR).
    Cs709Extended = 2,
    /// 1.0 = 80 nits, Windows/Linux HDR.
    Cs709Scrgb = 3,
}

impl GsColorSpace {
    /// Every variant, in header order.
    pub const ALL: [Self; 4] = [
        Self::Srgb,
        Self::Srgb16f,
        Self::Cs709Extended,
        Self::Cs709Scrgb,
    ];

    /// The C value of this color space.
    pub const fn to_c(self) -> c_int {
        self as c_int
    }
}

/// `enum obs_scale_type` (`libobs/obs.h`), in header order. The
/// discriminants are the header's values; the C ABI passes the enum as a
/// `c_int`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObsScaleType {
    Disable = 0,
    Point = 1,
    Bicubic = 2,
    Bilinear = 3,
    Lanczos = 4,
    Area = 5,
}

impl ObsScaleType {
    /// Every variant, in header order.
    pub const ALL: [Self; 6] = [
        Self::Disable,
        Self::Point,
        Self::Bicubic,
        Self::Bilinear,
        Self::Lanczos,
        Self::Area,
    ];

    /// The C value of this scale type.
    pub const fn to_c(self) -> c_int {
        self as c_int
    }

    /// The scale type whose C value is `value`, or `None` outside the enum.
    pub fn from_c(value: c_int) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.to_c() == value)
    }
}

/// `struct obs_video_plane`: one conversion plane. [`VideoPlane::NONE`]
/// (all zero, `GS_UNKNOWN`) is no plane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VideoPlane {
    pub width: u32,
    pub height: u32,
    pub format: GsColorFormat,
}

impl VideoPlane {
    /// No plane: the zeroed C struct.
    pub const NONE: Self = Self {
        width: 0,
        height: 0,
        format: GsColorFormat::Unknown,
    };

    const fn new(width: u32, height: u32, format: GsColorFormat) -> Self {
        Self {
            width,
            height,
            format,
        }
    }
}

/// `struct obs_video_output_plan`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OutputPlan {
    /// The output format is converted on the GPU.
    pub conversion_needed: bool,
    /// The technique of `format_conversion.effect` for each plane; `None`
    /// (`NULL`) for no plane. `obs-video.c` looks them up by name, so they
    /// are `'static` and NUL-terminated.
    pub conversion_techs: [Option<&'static CStr>; NUM_CHANNELS],
    /// `1 / width` for the formats whose techniques sample by texel, else 0.
    pub conversion_width_i: f32,
    /// `1 / height` for the formats whose techniques sample by texel, else 0.
    pub conversion_height_i: f32,
    /// The conversion textures and the copy surfaces of each plane.
    pub planes: [VideoPlane; NUM_CHANNELS],
    /// The render and output textures, and the copy surface without GPU
    /// conversion.
    pub texture_format: GsColorFormat,
    /// The color space of the render textures.
    pub render_space: GsColorSpace,
}

impl OutputPlan {
    /// The zeroed plan the C starts from (`= {0}`).
    pub const ZERO: Self = Self {
        conversion_needed: false,
        conversion_techs: [None; NUM_CHANNELS],
        conversion_width_i: 0.0,
        conversion_height_i: 0.0,
        planes: [VideoPlane::NONE; NUM_CHANNELS],
        texture_format: GsColorFormat::Unknown,
        render_space: GsColorSpace::Srgb,
    };
}

/// `obs_video_size_valid`: both sizes within [`SIZE_MIN`]..=[`SIZE_MAX`].
pub fn size_valid(width: u32, height: u32) -> bool {
    (SIZE_MIN..=SIZE_MAX).contains(&width) && (SIZE_MIN..=SIZE_MAX).contains(&height)
}

/// `obs_video_align_output_size`: the width down to a multiple of 4 and
/// the height down to a multiple of 2 (multiple-of-two and SSE alignment).
pub fn align_output_size(width: u32, height: u32) -> (u32, u32) {
    (width & 0xFFFF_FFFC, height & 0xFFFF_FFFE)
}

/// `obs_video_output_size_valid`: valid as given, and still valid once
/// aligned. The alignment makes an output width of 2 or 3 into 0.
pub fn output_size_valid(width: u32, height: u32) -> bool {
    if !size_valid(width, height) {
        return false;
    }
    let (width, height) = align_output_size(width, height);
    size_valid(width, height)
}

/// `obs_video_scale_type_name`: the name `obs_reset_video()` logs; `""`
/// for a value outside the enum (`None`).
pub fn scale_type_name(scale_type: Option<ObsScaleType>) -> &'static CStr {
    match scale_type {
        Some(ObsScaleType::Disable) => c"Disabled",
        Some(ObsScaleType::Point) => c"Point",
        Some(ObsScaleType::Bicubic) => c"Bicubic",
        Some(ObsScaleType::Bilinear) => c"Bilinear",
        Some(ObsScaleType::Lanczos) => c"Lanczos",
        Some(ObsScaleType::Area) => c"Area",
        None => c"",
    }
}

/// The technique set of a high-precision format for the color space: PQ,
/// HLG, or the SDR set for every other space (and for `None`).
fn by_transfer<T: Copy>(colorspace: Option<VideoColorspace>, pq: T, hlg: T, sdr: T) -> T {
    match colorspace {
        Some(VideoColorspace::Cs2100Pq) => pq,
        Some(VideoColorspace::Cs2100Hlg) => hlg,
        _ => sdr,
    }
}

/// `1.f / (float)size`, the C's texel size: the size rounded to the nearest
/// `f32`, then the IEEE quotient (infinite for 0).
fn texel(size: u32) -> f32 {
    // `as` rounds to nearest, as the C conversion does.
    1.0 / size as f32
}

/// `calc_gpu_conversion_sizes`: the techniques and texel sizes.
fn conversion_sizes(
    plan: &mut OutputPlan,
    format: Option<VideoFormat>,
    colorspace: Option<VideoColorspace>,
    width: u32,
    height: u32,
) {
    use VideoFormat as F;
    let Some(format) = format else {
        return;
    };
    let (techs, width_i, height_i): ([Option<&'static CStr>; NUM_CHANNELS], bool, bool) =
        match format {
            F::I420 => (
                [
                    Some(c"Planar_Y"),
                    Some(c"Planar_U_Left"),
                    Some(c"Planar_V_Left"),
                ],
                true,
                false,
            ),
            F::Nv12 => ([Some(c"NV12_Y"), Some(c"NV12_UV"), None], true, false),
            F::I444 => (
                [Some(c"Planar_Y"), Some(c"Planar_U"), Some(c"Planar_V")],
                false,
                false,
            ),
            F::I010 => (
                by_transfer(
                    colorspace,
                    [Some(c"I010_PQ_Y"), Some(c"I010_PQ_U"), Some(c"I010_PQ_V")],
                    [
                        Some(c"I010_HLG_Y"),
                        Some(c"I010_HLG_U"),
                        Some(c"I010_HLG_V"),
                    ],
                    [
                        Some(c"I010_SRGB_Y"),
                        Some(c"I010_SRGB_U"),
                        Some(c"I010_SRGB_V"),
                    ],
                ),
                true,
                true,
            ),
            F::P010 => (
                by_transfer(
                    colorspace,
                    [Some(c"P010_PQ_Y"), Some(c"P010_PQ_UV"), None],
                    [Some(c"P010_HLG_Y"), Some(c"P010_HLG_UV"), None],
                    [Some(c"P010_SRGB_Y"), Some(c"P010_SRGB_UV"), None],
                ),
                true,
                true,
            ),
            F::P216 => (
                by_transfer(
                    colorspace,
                    [Some(c"P216_PQ_Y"), Some(c"P216_PQ_UV"), None],
                    [Some(c"P216_HLG_Y"), Some(c"P216_HLG_UV"), None],
                    [Some(c"P216_SRGB_Y"), Some(c"P216_SRGB_UV"), None],
                ),
                true,
                true,
            ),
            F::P416 => (
                by_transfer(
                    colorspace,
                    [Some(c"P416_PQ_Y"), Some(c"P416_PQ_UV"), None],
                    [Some(c"P416_HLG_Y"), Some(c"P416_HLG_UV"), None],
                    [Some(c"P416_SRGB_Y"), Some(c"P416_SRGB_UV"), None],
                ),
                true,
                true,
            ),
            _ => return,
        };
    plan.conversion_needed = true;
    plan.conversion_techs = techs;
    if width_i {
        plan.conversion_width_i = texel(width);
    }
    if height_i {
        plan.conversion_height_i = texel(height);
    }
}

/// `calc_gpu_conversion_planes`: the conversion textures and copy surfaces.
fn conversion_planes(plan: &mut OutputPlan, format: Option<VideoFormat>, width: u32, height: u32) {
    use GsColorFormat as G;
    use VideoFormat as F;
    let none = VideoPlane::NONE;
    let full = |format| VideoPlane::new(width, height, format);
    let half = |format| VideoPlane::new(width / 2, height / 2, format);
    plan.planes = match format {
        Some(F::I420) => [full(G::R8), half(G::R8), half(G::R8)],
        Some(F::Nv12) => [full(G::R8), half(G::R8g8), none],
        Some(F::I444) => [full(G::R8), full(G::R8), full(G::R8)],
        Some(F::I010) => [full(G::R16), half(G::R16), half(G::R16)],
        Some(F::P010) => [full(G::R16), half(G::Rg16), none],
        Some(F::P216) => [
            full(G::R16),
            VideoPlane::new(width / 2, height, G::Rg16),
            none,
        ],
        Some(F::P416) => [full(G::R16), full(G::Rg16), none],
        _ => return,
    };
}

/// `calc_render_textures`: the render texture format and color space.
fn render_textures(
    plan: &mut OutputPlan,
    format: Option<VideoFormat>,
    colorspace: Option<VideoColorspace>,
) {
    use VideoFormat as F;
    plan.texture_format = match format {
        Some(F::I010 | F::P010 | F::I210 | F::I412 | F::Ya2l | F::P216 | F::P416) => {
            GsColorFormat::Rgba16f
        }
        _ => GsColorFormat::Bgra,
    };
    plan.render_space = match colorspace {
        Some(VideoColorspace::Cs2100Pq | VideoColorspace::Cs2100Hlg) => GsColorSpace::Cs709Extended,
        _ => match format {
            Some(F::I010 | F::P010 | F::P216 | F::P416) => GsColorSpace::Srgb16f,
            _ => GsColorSpace::Srgb,
        },
    };
}

/// `obs_video_output_plan`: the plan for `format` in `colorspace` at
/// `width` x `height`. Formats without a GPU conversion get no techniques
/// and no planes, only the render texture format and space. Odd sizes round
/// the chroma planes down.
pub fn output_plan(
    format: Option<VideoFormat>,
    colorspace: Option<VideoColorspace>,
    width: u32,
    height: u32,
) -> OutputPlan {
    let mut plan = OutputPlan::ZERO;
    conversion_sizes(&mut plan, format, colorspace, width, height);
    conversion_planes(&mut plan, format, width, height);
    render_textures(&mut plan, format, colorspace);
    plan
}
