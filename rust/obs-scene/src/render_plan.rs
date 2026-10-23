//! Safe core of `libobs/obs-scene-render-plan.c.h`: the per-item render
//! decisions of `render_item()` and `render_item_texture()` in
//! `obs-scene.c.h`.
//!
//! `obs-scene.c.h` keeps every `gs_*` call, `obs_source_video_render()`, the
//! texrender objects and `obs_get_video_sdr_white_level()`, and asks each
//! question where the C decided it, with the values it reads there:
//! - [`texture_enabled`]: the item is drawn through its texrender when it
//!   has a crop or a bounds crop, a scale filter, sRGB off, a blend mode other
//!   than Normal, or is a nested scene that is not a group;
//! - [`texrender_action`]: what happens to the texrender
//!   ([`Texrender`]);
//! - [`size_empty`]: a source with no size, for which only a running
//!   show/hide transition is rendered, at size 0 (#573);
//! - [`clear_white`]: the texrender is cleared to white for Darken, and to
//!   transparent black otherwise (#1406);
//! - [`child`]: the show transition, the hide transition or the source
//!   ([`Child`]);
//! - [`linear_srgb`]: linear sRGB unless the item is drawn through its
//!   texrender with sRGB off;
//! - [`texcoords_centered`]: the matrix is the identity but for a
//!   whole-number x and y translation, compared bit for bit, as the C's
//!   `memcmp`;
//! - [`effect`]: the texture's effect, point sampler, base dimension
//!   parameters and Area's upscale ([`EffectPlan`]);
//! - [`multiplier`]: the SDR white multiplier;
//! - [`technique`]: the technique ([`Technique`]).
//!
//! Enum values from the item (`scale_filter`, `blend_method`, `blend_type`
//! and the colour spaces) are `i32`, compared as they are: a value outside
//! its enum takes the C's fallthrough. Nothing here keeps state, takes a
//! lock or calls libobs.
//!
//! Characterized, not endorsed (`test_scene_render_plan_steps`), and kept: a
//! white level of 0 gives `80 / 0`, +inf; a NaN output scale is never close
//! to 1 nor below 0.5, so the filter's own effect is used; a quiet NaN or
//! infinite translation counts as centred, and a -0.0 anywhere but the
//! translation does not.
//!
//! No intentional differences.

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

use obs_graphics::matrix4::Matrix4;

/// `OBS_SCALE_DISABLE` (`enum obs_scale_type`).
pub const SCALE_DISABLE: i32 = 0;
/// `OBS_SCALE_POINT`.
pub const SCALE_POINT: i32 = 1;
/// `OBS_SCALE_BICUBIC`.
pub const SCALE_BICUBIC: i32 = 2;
/// `OBS_SCALE_BILINEAR`: not Point, so it draws with the default effect.
pub const SCALE_BILINEAR: i32 = 3;
/// `OBS_SCALE_LANCZOS`.
pub const SCALE_LANCZOS: i32 = 4;
/// `OBS_SCALE_AREA`.
pub const SCALE_AREA: i32 = 5;
/// `OBS_BLEND_METHOD_SRGB_OFF` (`enum obs_blending_method`).
pub const BLEND_METHOD_SRGB_OFF: i32 = 1;
/// `OBS_BLEND_NORMAL` (`enum obs_blending_type`).
pub const BLEND_NORMAL: i32 = 0;
/// `OBS_BLEND_DARKEN`.
pub const BLEND_DARKEN: i32 = 6;
/// `GS_CS_SRGB` (`enum gs_color_space`).
pub const CS_SRGB: i32 = 0;
/// `GS_CS_SRGB_16F`.
pub const CS_SRGB_16F: i32 = 1;
/// `GS_CS_709_EXTENDED`.
pub const CS_709_EXTENDED: i32 = 2;
/// `GS_CS_709_SCRGB`.
pub const CS_709_SCRGB: i32 = 3;
/// `EPSILON` (`graphics/math-defs.h`): how close to 1 an output scale is 1.
pub const EPSILON: f32 = 1e-4;

/// `struct obs_scene_render_triggers`: the item's fields that decide
/// whether it is drawn through a texture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Triggers {
    pub crop_left: i32,
    pub crop_top: i32,
    pub crop_right: i32,
    pub crop_bottom: i32,
    pub bounds_crop_left: i32,
    pub bounds_crop_top: i32,
    pub bounds_crop_right: i32,
    pub bounds_crop_bottom: i32,
    /// `item->scale_filter`, an `enum obs_scale_type`.
    pub scale_filter: i32,
    /// `item->blend_method`, an `enum obs_blending_method`.
    pub blend_method: i32,
    /// `item->blend_type`, an `enum obs_blending_type`.
    pub blend_type: i32,
    /// The item's source is a scene.
    pub is_scene: bool,
    /// `item->is_group`.
    pub is_group: bool,
}

/// `obs_scene_render_texture_enabled`.
#[must_use]
pub const fn texture_enabled(t: &Triggers) -> bool {
    let crop = t.crop_left != 0 || t.crop_right != 0 || t.crop_top != 0 || t.crop_bottom != 0;
    let bounds_crop = t.bounds_crop_left != 0
        || t.bounds_crop_right != 0
        || t.bounds_crop_top != 0
        || t.bounds_crop_bottom != 0;
    crop || bounds_crop
        || t.scale_filter != SCALE_DISABLE
        || t.blend_method == BLEND_METHOD_SRGB_OFF
        || t.blend_type != BLEND_NORMAL
        || (t.is_scene && !t.is_group)
}

/// `enum obs_scene_render_texrender`: what `render_item()` does with the
/// item's texrender.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Texrender {
    /// None is there and none is wanted.
    None,
    /// Keep the one there.
    Keep,
    /// Destroy it.
    Destroy,
    /// Create one.
    Create,
    /// Destroy it and create one in the source's format.
    Recreate,
}

/// `obs_scene_render_texrender_action`: the texrender is destroyed when it
/// is there and either not wanted or not in the source's format, then one
/// is created when it is wanted and none is left.
#[must_use]
pub const fn texrender_action(
    has_texrender: bool,
    use_texrender: bool,
    format_matches: bool,
) -> Texrender {
    let destroy = has_texrender && (!use_texrender || !format_matches);
    let left = has_texrender && !destroy;
    let create = !left && use_texrender;
    match (destroy, create, left) {
        (true, true, _) => Texrender::Recreate,
        (true, false, _) => Texrender::Destroy,
        (false, true, _) => Texrender::Create,
        (false, false, true) => Texrender::Keep,
        (false, false, false) => Texrender::None,
    }
}

/// `obs_scene_render_size_empty`: the source's width or height is 0 (#573).
#[must_use]
pub const fn size_empty(width: u32, height: u32) -> bool {
    width == 0 || height == 0
}

/// `obs_scene_render_clear_white`: Darken draws with `GS_BLEND_OP_MIN`,
/// which ignores the blend factors, so a transparent clear would draw
/// black; white leaves what is under it (#1406).
#[must_use]
pub const fn clear_white(blend_type: i32) -> bool {
    blend_type == BLEND_DARKEN
}

/// `enum obs_scene_render_child_id`: what `render_item()` draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Child {
    /// The source.
    Source,
    /// `item->show_transition`.
    Show,
    /// `item->hide_transition`.
    Hide,
}

/// `obs_scene_render_child`: a visible item with a running show
/// transition draws it, a hidden item with a running hide transition draws
/// that, and any other draws the source.
#[must_use]
pub const fn child(user_visible: bool, show_active: bool, hide_active: bool) -> Child {
    if user_visible && show_active {
        Child::Show
    } else if !user_visible && hide_active {
        Child::Hide
    } else {
        Child::Source
    }
}

/// `obs_scene_render_linear_srgb`.
#[must_use]
pub const fn linear_srgb(has_texrender: bool, blend_method: i32) -> bool {
    !has_texrender || blend_method != BLEND_METHOD_SRGB_OFF
}

/// `obs_scene_texcoords_centered`: `m` is bit for bit the identity with its
/// x and y translation replaced by their floors.
#[must_use]
pub fn texcoords_centered(m: &Matrix4) -> bool {
    let rows = [m.x, m.y, m.z, m.t];
    let expected: [[f32; 4]; 4] = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [m.t.x.floor(), m.t.y.floor(), 0.0, 1.0],
    ];
    rows.iter().zip(expected.iter()).all(|(row, want)| {
        [row.x, row.y, row.z, row.w]
            .iter()
            .zip(want.iter())
            .all(|(got, want)| got.to_bits() == want.to_bits())
    })
}

/// `enum obs_scene_render_effect_id`: the `obs->video` effect the texture is
/// drawn with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// `default_effect`.
    Default,
    /// `bilinear_lowres_effect`.
    BilinearLowres,
    /// `bicubic_effect`.
    Bicubic,
    /// `lanczos_effect`.
    Lanczos,
    /// `area_effect`.
    Area,
}

/// `struct obs_scene_render_effect_plan`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectPlan {
    pub effect: Effect,
    /// Set `obs->video.point_sampler` on the effect's `image`.
    pub point_sampler: bool,
    /// Set `base_dimension` and `base_dimension_i` from the texture's size.
    pub base_dimension: bool,
    /// Area, scaled up on both axes: the `DrawUpscale` techniques.
    pub upscale: bool,
}

/// C's `close_float(f, 1.0f, EPSILON)`: `fabsf(f - 1) <= EPSILON`, false
/// for NaN.
fn close_to_one(f: f32) -> bool {
    (f - 1.0).abs() <= EPSILON
}

/// `obs_scene_render_effect`: Disable draws as it is; Point sets the point
/// sampler at any scale; any other filter, at an output scale not within
/// `EPSILON` of 1 on both axes, sets the base dimension and draws with the
/// low-resolution bilinear effect below 0.5 on either axis, else Bicubic,
/// Lanczos or Area (which upscales at 1 and over on both axes), else the
/// default effect.
#[must_use]
pub fn effect(scale_filter: i32, scale_x: f32, scale_y: f32) -> EffectPlan {
    let mut plan = EffectPlan {
        effect: Effect::Default,
        point_sampler: false,
        base_dimension: false,
        upscale: false,
    };
    if scale_filter == SCALE_DISABLE {
        return plan;
    }
    if scale_filter == SCALE_POINT {
        plan.point_sampler = true;
        return plan;
    }
    if close_to_one(scale_x) && close_to_one(scale_y) {
        return plan;
    }
    if scale_x < 0.5 || scale_y < 0.5 {
        plan.effect = Effect::BilinearLowres;
    } else if scale_filter == SCALE_BICUBIC {
        plan.effect = Effect::Bicubic;
    } else if scale_filter == SCALE_LANCZOS {
        plan.effect = Effect::Lanczos;
    } else if scale_filter == SCALE_AREA {
        plan.effect = Effect::Area;
        plan.upscale = scale_x >= 1.0 && scale_y >= 1.0;
    }
    plan.base_dimension = true;
    plan
}

/// sRGB, sRGB 16F or 709 extended.
const fn sdr_or_extended(space: i32) -> bool {
    matches!(space, CS_SRGB | CS_SRGB_16F | CS_709_EXTENDED)
}

/// `obs_scene_render_multiplier`: SDR or extended drawn into scRGB is
/// multiplied by `white / 80`, scRGB drawn into SDR or extended by
/// `80 / white` (+inf for a white level of 0), and anything else by 1.
#[must_use]
pub fn multiplier(current_space: i32, source_space: i32, sdr_white_level: f32) -> f32 {
    if current_space == CS_709_SCRGB && sdr_or_extended(source_space) {
        sdr_white_level / 80.0
    } else if source_space == CS_709_SCRGB && sdr_or_extended(current_space) {
        80.0 / sdr_white_level
    } else {
        1.0
    }
}

/// `enum obs_scene_render_technique_id`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Technique {
    /// `"Draw"`.
    Draw,
    /// `"DrawMultiply"`.
    DrawMultiply,
    /// `"DrawTonemap"`.
    DrawTonemap,
    /// `"DrawMultiplyTonemap"`.
    DrawMultiplyTonemap,
    /// `"DrawUpscale"`.
    DrawUpscale,
    /// `"DrawUpscaleMultiply"`.
    DrawUpscaleMultiply,
    /// `"DrawUpscaleTonemap"`.
    DrawUpscaleTonemap,
    /// `"DrawUpscaleMultiplyTonemap"`.
    DrawUpscaleMultiplyTonemap,
}

/// `obs_scene_render_technique`: SDR into scRGB and extended into scRGB
/// multiply; extended into SDR tone maps; scRGB into SDR multiplies and
/// tone maps, scRGB into extended multiplies; anything else, a value
/// outside the enum included, draws as it is. With `upscale`, the
/// `DrawUpscale` variant of each.
#[must_use]
pub const fn technique(upscale: bool, current_space: i32, source_space: i32) -> Technique {
    let sdr_target = matches!(current_space, CS_SRGB | CS_SRGB_16F);
    let (multiply, tonemap) = match source_space {
        CS_SRGB | CS_SRGB_16F => (current_space == CS_709_SCRGB, false),
        CS_709_EXTENDED => (current_space == CS_709_SCRGB, sdr_target),
        CS_709_SCRGB => (sdr_target || current_space == CS_709_EXTENDED, sdr_target),
        _ => (false, false),
    };
    match (upscale, multiply, tonemap) {
        (false, false, false) => Technique::Draw,
        (false, true, false) => Technique::DrawMultiply,
        (false, false, true) => Technique::DrawTonemap,
        (false, true, true) => Technique::DrawMultiplyTonemap,
        (true, false, false) => Technique::DrawUpscale,
        (true, true, false) => Technique::DrawUpscaleMultiply,
        (true, false, true) => Technique::DrawUpscaleTonemap,
        (true, true, true) => Technique::DrawUpscaleMultiplyTonemap,
    }
}
