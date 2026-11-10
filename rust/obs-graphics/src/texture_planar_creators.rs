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
//!   two outputs that are the same slot see the chroma plane.

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
        todo!()
    }

    /// The `LOG_ERROR` line for an odd width or height.
    #[must_use]
    pub const fn odd_size_error(self) -> &'static str {
        todo!()
    }

    /// The luma plane's format.
    #[must_use]
    pub const fn luma_format(self) -> ColorFormat {
        todo!()
    }

    /// The chroma plane's format.
    #[must_use]
    pub const fn chroma_format(self) -> ColorFormat {
        todo!()
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
    let _ = (host, planar, width, height, flags);
    todo!()
}
