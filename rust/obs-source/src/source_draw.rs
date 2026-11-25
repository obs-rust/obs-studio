//! Safe core of `libobs/obs-source-draw.c.h`: the exported draw helpers
//! `obs_source_draw_set_color_matrix()` and `obs_source_draw()`.
//!
//! Neither touches a source: each is a fixed sequence of graphics calls on
//! the current effect, made here in the C's order, each once, under the C's
//! conditions, over [`Graphics`].
//!
//! - [`set_color_matrix`]: the current effect, or a warning and nothing
//!   else; then a NULL matrix logs `obs_ptr_valid()`'s debug line and stops;
//!   then the `"color_matrix"`, `"color_range_min"` and `"color_range_max"`
//!   params, looked up in that order, are set to the matrix and the two
//!   ranges (12 bytes each), a NULL range as `(0, 0, 0)` / `(1, 1, 1)`.
//! - [`draw`]: a NULL texture logs `obs_ptr_valid()`'s debug line and stops
//!   (before the effect is read); then the current effect, or a warning and
//!   nothing else; then linear sRGB is read, the framebuffer sRGB flag saved
//!   and set to it, `"image"` set to the texture (as sRGB when linear sRGB
//!   is on), the matrix pushed and translated by `(x, y, 0)` only when `x`
//!   or `y` is not 0, the sprite drawn (`GS_FLIP_V` when flipped), the
//!   matrix popped when it was pushed and the sRGB flag put back.
//!
//! No intentional differences from C.

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

/// `GS_FLIP_V` (`graphics/graphics.h`).
pub const GS_FLIP_V: u32 = 1 << 1;

/// The range `obs_source_draw_set_color_matrix()` sets for a NULL
/// `color_range_min`.
pub const DEFAULT_RANGE_MIN: [f32; 3] = [0.0, 0.0, 0.0];
/// The range it sets for a NULL `color_range_max`.
pub const DEFAULT_RANGE_MAX: [f32; 3] = [1.0, 1.0, 1.0];

/// Which of the two functions logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Function {
    /// `obs_source_draw_set_color_matrix`.
    SetColorMatrix,
    /// `obs_source_draw`.
    Draw,
}

impl Function {
    /// The function's name, as its log lines give it.
    #[must_use]
    pub const fn name(self) -> &'static CStr {
        match self {
            Function::SetColorMatrix => c"obs_source_draw_set_color_matrix",
            Function::Draw => c"obs_source_draw",
        }
    }

    /// Its whole "no active effect" warning.
    #[must_use]
    pub const fn no_effect_warning(self) -> &'static CStr {
        match self {
            Function::SetColorMatrix => c"obs_source_draw_set_color_matrix: no active effect!",
            Function::Draw => c"obs_source_draw: no active effect!",
        }
    }

    /// The parameter `obs_ptr_valid()` names when it is NULL.
    #[must_use]
    pub const fn checked_param(self) -> &'static CStr {
        match self {
            Function::SetColorMatrix => c"color_matrix",
            Function::Draw => c"texture",
        }
    }
}

/// A param of the current effect the functions set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Param {
    /// `"color_matrix"`.
    ColorMatrix,
    /// `"color_range_min"`.
    ColorRangeMin,
    /// `"color_range_max"`.
    ColorRangeMax,
    /// `"image"`.
    Image,
}

impl Param {
    /// The param's name.
    #[must_use]
    pub const fn name(self) -> &'static CStr {
        match self {
            Param::ColorMatrix => c"color_matrix",
            Param::ColorRangeMin => c"color_range_min",
            Param::ColorRangeMax => c"color_range_max",
            Param::Image => c"image",
        }
    }
}

/// The libobs and graphics calls of the two functions, one method each.
pub trait Graphics {
    /// A `gs_effect_t *` that is not NULL.
    type Effect: Copy;
    /// A `gs_eparam_t *` as the lookup gives it, NULL included.
    type Param: Copy;
    /// A `gs_texture_t *` that is not NULL.
    type Texture: Copy;

    /// `gs_get_effect()`, `None` when NULL.
    fn current_effect(&mut self) -> Option<Self::Effect>;
    /// `blog(LOG_WARNING, "<function>: no active effect!")`.
    fn warn_no_effect(&mut self, function: Function);
    /// `obs_ptr_valid()`'s line for a NULL parameter:
    /// `blog(LOG_DEBUG, "%s: Null '%s' parameter", name, param)`.
    fn log_null(&mut self, function: Function);
    /// `gs_effect_get_param_by_name(effect, param.name())`.
    fn param(&mut self, effect: Self::Effect, param: Param) -> Self::Param;
    /// `gs_effect_set_matrix4(param, matrix)`.
    fn set_matrix4(&mut self, param: Self::Param, matrix: &[f32; 16]);
    /// `gs_effect_set_val(param, range, sizeof(float) * 3)`.
    fn set_range(&mut self, param: Self::Param, range: &[f32; 3]);
    /// `gs_get_linear_srgb()`.
    fn linear_srgb(&mut self) -> bool;
    /// `gs_framebuffer_srgb_enabled()`.
    fn framebuffer_srgb_enabled(&mut self) -> bool;
    /// `gs_enable_framebuffer_srgb(enable)`.
    fn enable_framebuffer_srgb(&mut self, enable: bool);
    /// `gs_effect_set_texture_srgb(param, texture)`.
    fn set_texture_srgb(&mut self, param: Self::Param, texture: Self::Texture);
    /// `gs_effect_set_texture(param, texture)`.
    fn set_texture(&mut self, param: Self::Param, texture: Self::Texture);
    /// `gs_matrix_push()`.
    fn matrix_push(&mut self);
    /// `gs_matrix_translate3f(x, y, z)`.
    fn matrix_translate3f(&mut self, x: f32, y: f32, z: f32);
    /// `gs_draw_sprite(texture, flip, width, height)`.
    fn draw_sprite(&mut self, texture: Self::Texture, flip: u32, width: u32, height: u32);
    /// `gs_matrix_pop()`.
    fn matrix_pop(&mut self);
}

/// `obs_source_draw_set_color_matrix(color_matrix, color_range_min,
/// color_range_max)`, `None` for a NULL pointer.
#[expect(clippy::todo, reason = "RED: the port is not written yet")]
pub fn set_color_matrix<G: Graphics>(
    graphics: &mut G,
    color_matrix: Option<&[f32; 16]>,
    color_range_min: Option<&[f32; 3]>,
    color_range_max: Option<&[f32; 3]>,
) {
    let _ = (graphics, color_matrix, color_range_min, color_range_max);
    todo!()
}

/// `obs_source_draw(texture, x, y, cx, cy, flip)`, `None` for a NULL
/// texture.
#[expect(clippy::todo, reason = "RED: the port is not written yet")]
pub fn draw<G: Graphics>(
    graphics: &mut G,
    texture: Option<G::Texture>,
    x: i32,
    y: i32,
    cx: u32,
    cy: u32,
    flip: bool,
) {
    let _ = (graphics, texture, x, y, cx, cy, flip);
    todo!()
}
