//! Safe core of `libobs/obs-render-canvas-texture.c.h`: the canvas texture
//! draw entry points `obs_render_main_texture()`,
//! `obs_render_main_texture_src_color_only()`, `obs_render_canvas_texture()`
//! and `obs_render_canvas_texture_src_color_only()`, and the
//! `obs_render_canvas_texture_internal()` they share.
//!
//! The draw makes the C's calls in the C's order, each once, under the C's
//! conditions, over [`Graphics`]: the canvas's mix, then (only with a mix)
//! its `texture_rendered` flag; with no mix or an unrendered texture nothing
//! else. Otherwise the mix's `render_space`, `gs_get_color_space()`, the SDR
//! white level only into scRGB, the framebuffer sRGB flag, sRGB on, the mix's
//! `render_texture` (read once), the default base effect, its `"image"`
//! param set to the texture in sRGB and its `"multiplier"` param set to the
//! multiplier, the blend state pushed and set to the variant's factors, the
//! technique's passes each drawing the texture as a sprite of its own size,
//! the blend state popped and the sRGB flag put back. The decisions (skip,
//! technique, white level, multiplier) are [`crate::canvas_draw`]'s.
//!
//! One intentional difference: a NULL canvas (or a NULL main canvas, before
//! `obs_startup()` or after `obs_shutdown()`) draws nothing, where the C reads
//! `canvas->mix` through NULL and crashes.

use core::ffi::{CStr, c_int};

use crate::canvas_draw::Technique;

/// `GS_BLEND_ZERO` (`enum gs_blend_type`, `graphics/graphics.h`).
pub const GS_BLEND_ZERO: u32 = 0;
/// `GS_BLEND_ONE`.
pub const GS_BLEND_ONE: u32 = 1;
/// `GS_BLEND_INVSRCALPHA`.
pub const GS_BLEND_INVSRCALPHA: u32 = 5;

/// `OBS_EFFECT_DEFAULT` (`enum obs_base_effect`, `obs.h`).
pub const OBS_EFFECT_DEFAULT: c_int = 0;

/// The four factors of `gs_blend_function_separate()`, `enum gs_blend_type`
/// values as `u32`, as obs-graphics takes them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Blend {
    /// `src_c`.
    pub src_color: u32,
    /// `dest_c`.
    pub dest_color: u32,
    /// `src_a`.
    pub src_alpha: u32,
    /// `dest_a`.
    pub dest_alpha: u32,
}

/// `obs_render_main_texture()` and `obs_render_canvas_texture()`: the
/// texture over the target (ONE, INVSRCALPHA for colour and alpha).
pub const OVER: Blend = Blend {
    src_color: GS_BLEND_ONE,
    dest_color: GS_BLEND_INVSRCALPHA,
    src_alpha: GS_BLEND_ONE,
    dest_alpha: GS_BLEND_INVSRCALPHA,
};

/// The `_src_color_only` variants: the target's colour dropped (ONE, ZERO),
/// alpha as [`OVER`].
pub const SRC_COLOR_ONLY: Blend = Blend {
    src_color: GS_BLEND_ONE,
    dest_color: GS_BLEND_ZERO,
    src_alpha: GS_BLEND_ONE,
    dest_alpha: GS_BLEND_INVSRCALPHA,
};

/// A param of `default.effect` the draw sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Param {
    /// `"image"`: the texture, in sRGB.
    Image,
    /// `"multiplier"`: the white level multiplier.
    Multiplier,
}

impl Param {
    /// The param's name in `default.effect`.
    #[must_use]
    pub const fn name(self) -> &'static CStr {
        match self {
            Param::Image => c"image",
            Param::Multiplier => c"multiplier",
        }
    }
}

/// The canvas and mix fields the draw reads and the libobs and graphics
/// calls it makes, one method each.
pub trait Graphics {
    /// An `obs_canvas_t *` that is not NULL.
    type Canvas: Copy;
    /// A `struct obs_core_video_mix *` that is not NULL.
    type Mix: Copy;
    /// A `gs_texture_t *` as the mix holds it, NULL included.
    type Texture: Copy;
    /// A `gs_effect_t *` as `obs_get_base_effect()` gives it.
    type Effect: Copy;
    /// A `gs_eparam_t *` as the lookup gives it.
    type Param: Copy;

    /// `obs->data.main_canvas`, `None` when NULL.
    fn main_canvas(&mut self) -> Option<Self::Canvas>;
    /// `canvas->mix`, `None` when NULL.
    fn mix(&mut self, canvas: Self::Canvas) -> Option<Self::Mix>;
    /// `os_atomic_load_bool(&mix->texture_rendered)`.
    fn texture_rendered(&mut self, mix: Self::Mix) -> bool;
    /// `(int)mix->render_space`.
    fn render_space(&mut self, mix: Self::Mix) -> c_int;
    /// `mix->render_texture`.
    fn render_texture(&mut self, mix: Self::Mix) -> Self::Texture;
    /// `(int)gs_get_color_space()`.
    fn color_space(&mut self) -> c_int;
    /// `obs_get_video_sdr_white_level()`.
    fn sdr_white_level(&mut self) -> f32;
    /// `gs_framebuffer_srgb_enabled()`.
    fn framebuffer_srgb_enabled(&mut self) -> bool;
    /// `gs_enable_framebuffer_srgb(enable)`.
    fn enable_framebuffer_srgb(&mut self, enable: bool);
    /// `obs_get_base_effect(OBS_EFFECT_DEFAULT)`.
    fn default_effect(&mut self) -> Self::Effect;
    /// `gs_effect_get_param_by_name(effect, param.name())`.
    fn param(&mut self, effect: Self::Effect, param: Param) -> Self::Param;
    /// `gs_effect_set_texture_srgb(param, texture)`.
    fn set_texture_srgb(&mut self, param: Self::Param, texture: Self::Texture);
    /// `gs_effect_set_float(param, value)`.
    fn set_float(&mut self, param: Self::Param, value: f32);
    /// `gs_blend_state_push()`.
    fn blend_state_push(&mut self);
    /// `gs_blend_function_separate(...)` with the four factors.
    fn blend_function_separate(&mut self, blend: Blend);
    /// `gs_effect_loop(effect, technique's name)`.
    fn effect_loop(&mut self, effect: Self::Effect, technique: Technique) -> bool;
    /// `gs_draw_sprite(texture, 0, 0, 0)`.
    fn draw_sprite(&mut self, texture: Self::Texture);
    /// `gs_blend_state_pop()`.
    fn blend_state_pop(&mut self);
}

/// `obs_render_main_texture()` ([`OVER`]) and
/// `obs_render_main_texture_src_color_only()` ([`SRC_COLOR_ONLY`]): the
/// main canvas, read once, drawn as [`render_canvas`] draws it.
pub fn render_main<G: Graphics>(graphics: &mut G, blend: Blend) {
    let _ = (graphics, blend);
    todo!("obs_render_main_texture")
}

/// `obs_render_canvas_texture()` ([`OVER`]),
/// `obs_render_canvas_texture_src_color_only()` ([`SRC_COLOR_ONLY`]) and
/// the `obs_render_canvas_texture_internal()` they share.
pub fn render_canvas<G: Graphics>(graphics: &mut G, canvas: Option<G::Canvas>, blend: Blend) {
    let _ = (graphics, canvas, blend);
    todo!("obs_render_canvas_texture")
}
