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

    #[allow(dead_code)] // used by the plan
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
pub fn size_valid(_width: u32, _height: u32) -> bool {
    todo!()
}

/// `obs_video_align_output_size`: the width down to a multiple of 4 and
/// the height down to a multiple of 2 (multiple-of-two and SSE alignment).
pub fn align_output_size(_width: u32, _height: u32) -> (u32, u32) {
    todo!()
}

/// `obs_video_output_size_valid`: valid as given, and still valid once
/// aligned. The alignment makes an output width of 2 or 3 into 0.
pub fn output_size_valid(_width: u32, _height: u32) -> bool {
    todo!()
}

/// `obs_video_scale_type_name`: the name `obs_reset_video()` logs; `""`
/// for a value outside the enum (`None`).
pub fn scale_type_name(_scale_type: Option<ObsScaleType>) -> &'static CStr {
    todo!()
}

/// `obs_video_output_plan`: the plan for `format` in `colorspace` at
/// `width` x `height`. Formats without a GPU conversion get no techniques
/// and no planes, only the render texture format and space. Odd sizes round
/// the chroma planes down.
pub fn output_plan(
    _format: Option<VideoFormat>,
    _colorspace: Option<VideoColorspace>,
    _width: u32,
    _height: u32,
) -> OutputPlan {
    todo!()
}
