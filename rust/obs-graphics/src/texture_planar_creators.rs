//! `gs_texture_create_nv12()` and `gs_texture_create_p010()` from
//! `libobs/graphics/texture-planar-creators.c.h`: a luma and a chroma
//! texture for one planar frame.
//!
//! [`create`] checks for a graphics context, then for even dimensions, then
//! offers the pair to the device's own planar creator when it has one. When
//! the device has none or declines, it makes the two planes itself: luma at
//! full size, chroma at half size, one level, no data, the caller's flags.
//! When either plane fails it destroys the one that was made and clears both
//! outputs. Every graphics call, log line and read or write of the outputs
//! goes through [`Host`], in the C order.
//!
//! Kept from the C (characterized, not endorsed):
//! - Zero dimensions are even, so they reach the device and the plane
//!   creators.
//! - When the device declines, the fallback overwrites whatever the device
//!   wrote to the outputs, without destroying it.
//! - Both planes are created before either is checked, so a failed luma
//!   plane still tries the chroma plane.
//! - The outputs are read back after both are written ([`Host::load`]), so
//!   two outputs that are the same slot see the chroma plane. The cleanup
//!   reads a set output again as the destroy's argument.

use crate::texture_render::ColorFormat;

/// `GS_R8`.
pub const GS_R8: ColorFormat = ColorFormat(2);
/// `GS_R16`.
pub const GS_R16: ColorFormat = ColorFormat(8);
/// `GS_R8G8`.
pub const GS_R8G8: ColorFormat = ColorFormat(18);
/// `GS_RG16`.
pub const GS_RG16: ColorFormat = ColorFormat(22);

/// Which of the two entry points runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Planar {
    /// `gs_texture_create_nv12()`: 8-bit planes.
    Nv12,
    /// `gs_texture_create_p010()`: 16-bit planes.
    P010,
}

impl Planar {
    /// The C function's name, as its context check logs it.
    #[must_use]
    pub const fn function(self) -> &'static str {
        match self {
            Self::Nv12 => "gs_texture_create_nv12",
            Self::P010 => "gs_texture_create_p010",
        }
    }

    /// The `LOG_ERROR` line for an odd width or height.
    #[must_use]
    pub const fn odd_size_error(self) -> &'static str {
        match self {
            Self::Nv12 => "NV12 textures must have dimensions divisible by 2.",
            Self::P010 => "P010 textures must have dimensions divisible by 2.",
        }
    }

    /// The luma plane's format.
    #[must_use]
    pub const fn luma_format(self) -> ColorFormat {
        match self {
            Self::Nv12 => GS_R8,
            Self::P010 => GS_R16,
        }
    }

    /// The chroma plane's format.
    #[must_use]
    pub const fn chroma_format(self) -> ColorFormat {
        match self {
            Self::Nv12 => GS_R8G8,
            Self::P010 => GS_RG16,
        }
    }
}

/// One of the two outputs, `tex_y` or `tex_uv`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plane {
    /// `*tex_y`, the luma plane.
    Y,
    /// `*tex_uv`, the chroma plane.
    Uv,
}

/// A line [`create`] logs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Line {
    /// `gs_valid()`'s `LOG_DEBUG` line: `<function>: called while not in a
    /// graphics context`.
    NoContext(Planar),
    /// The `LOG_ERROR` line for an odd width or height
    /// ([`Planar::odd_size_error`]).
    OddSize(Planar),
}

/// What [`create`] calls, reads and writes.
pub trait Host {
    /// A texture handle; [`None`] stands for a null one.
    type Texture: Copy;

    /// Whether the thread has a graphics context.
    fn has_context(&mut self) -> bool;

    /// Logs `line`.
    fn log(&mut self, line: Line);

    /// The device's own planar creator for `planar`: [`None`] when the
    /// device has none, else what it returned. It writes the outputs itself.
    fn device_create(
        &mut self,
        planar: Planar,
        width: u32,
        height: u32,
        flags: u32,
    ) -> Option<bool>;

    /// `gs_texture_create(width, height, format, 1, NULL, flags)`.
    fn texture_create(
        &mut self,
        width: u32,
        height: u32,
        format: ColorFormat,
        flags: u32,
    ) -> Option<Self::Texture>;

    /// `gs_texture_destroy(texture)`.
    fn texture_destroy(&mut self, texture: Self::Texture);

    /// Writes one output.
    fn store(&mut self, plane: Plane, texture: Option<Self::Texture>);

    /// Reads one output back.
    fn load(&mut self, plane: Plane) -> Option<Self::Texture>;
}

/// `gs_texture_create_nv12()` or `gs_texture_create_p010()`: whether the
/// outputs now hold a luma and a chroma texture.
pub fn create<H: Host>(host: &mut H, planar: Planar, width: u32, height: u32, flags: u32) -> bool {
    if !host.has_context() {
        host.log(Line::NoContext(planar));
        return false;
    }

    if width & 1 == 1 || height & 1 == 1 {
        host.log(Line::OddSize(planar));
        return false;
    }

    if host.device_create(planar, width, height, flags) == Some(true) {
        return true;
    }

    let luma = host.texture_create(width, height, planar.luma_format(), flags);
    host.store(Plane::Y, luma);
    let chroma = host.texture_create(width / 2, height / 2, planar.chroma_format(), flags);
    host.store(Plane::Uv, chroma);

    if host.load(Plane::Y).is_none() || host.load(Plane::Uv).is_none() {
        // `if (*tex_y) gs_texture_destroy(*tex_y);`: one read for the
        // condition, another for the argument.
        if host.load(Plane::Y).is_some()
            && let Some(luma) = host.load(Plane::Y)
        {
            host.texture_destroy(luma);
        }
        if host.load(Plane::Uv).is_some()
            && let Some(chroma) = host.load(Plane::Uv)
        {
            host.texture_destroy(chroma);
        }
        host.store(Plane::Y, None);
        host.store(Plane::Uv, None);
        return false;
    }

    true
}
