//! C ABI for `libobs/obs-scene-render-plan.c.h`, the per-item render
//! decisions of `render_item()` and `render_item_texture()`, declared in
//! `libobs/obs-scene-render-plan.h` without `EXPORT`.
//!
//! `obs-scene.c.h` (still C) reads the item's fields where it made each
//! decision and passes them by value, [`obs_scene_render_triggers`] and
//! `&item->draw_transform` by pointer; it keeps every graphics call, the
//! source and transition renders, the texrender objects and the SDR white
//! level lookup, and maps the effect and technique values to `obs->video.*`
//! and to the technique names. So `struct obs_scene_item` is never
//! mirrored, no source pointer crosses, and these shims take no lock and
//! call nothing in libobs. All stay hidden (`rust-hidden.txt`).
//!
//! No intentional differences. A null pointer is not checked: libobs passes
//! a local or the item's own field.

use std::os::raw::c_int;
use std::ptr;

use obs_graphics::ffi::matrix4::matrix4;
use obs_graphics::matrix4::Matrix4;

use crate::render_plan::{self as core, Child, Effect, Technique, Texrender, Triggers};

/// `struct obs_scene_render_triggers` (`obs-scene-render-plan.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(non_camel_case_types)]
pub struct obs_scene_render_triggers {
    pub crop_left: c_int,
    pub crop_top: c_int,
    pub crop_right: c_int,
    pub crop_bottom: c_int,
    pub bounds_crop_left: c_int,
    pub bounds_crop_top: c_int,
    pub bounds_crop_right: c_int,
    pub bounds_crop_bottom: c_int,
    pub scale_filter: c_int,
    pub blend_method: c_int,
    pub blend_type: c_int,
    pub is_scene: bool,
    pub is_group: bool,
}

/// `struct obs_scene_render_effect_plan` (`obs-scene-render-plan.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(non_camel_case_types)]
pub struct obs_scene_render_effect_plan {
    /// An `enum obs_scene_render_effect_id` value.
    pub effect: c_int,
    pub point_sampler: bool,
    pub base_dimension: bool,
    pub upscale: bool,
}

/// `enum obs_scene_render_texrender`, `enum obs_scene_render_child_id` and
/// `enum obs_scene_render_technique_id`, as the C's enums are returned (an
/// `int` or `unsigned int`; every value fits both).
#[allow(non_camel_case_types)]
pub type obs_scene_render_enum = u32;

/// `OBS_SCENE_RENDER_TEXRENDER_NONE`.
pub const OBS_SCENE_RENDER_TEXRENDER_NONE: obs_scene_render_enum = 0;
/// `OBS_SCENE_RENDER_TEXRENDER_KEEP`.
pub const OBS_SCENE_RENDER_TEXRENDER_KEEP: obs_scene_render_enum = 1;
/// `OBS_SCENE_RENDER_TEXRENDER_DESTROY`.
pub const OBS_SCENE_RENDER_TEXRENDER_DESTROY: obs_scene_render_enum = 2;
/// `OBS_SCENE_RENDER_TEXRENDER_CREATE`.
pub const OBS_SCENE_RENDER_TEXRENDER_CREATE: obs_scene_render_enum = 3;
/// `OBS_SCENE_RENDER_TEXRENDER_RECREATE`.
pub const OBS_SCENE_RENDER_TEXRENDER_RECREATE: obs_scene_render_enum = 4;

/// `OBS_SCENE_RENDER_CHILD_SOURCE`.
pub const OBS_SCENE_RENDER_CHILD_SOURCE: obs_scene_render_enum = 0;
/// `OBS_SCENE_RENDER_CHILD_SHOW`.
pub const OBS_SCENE_RENDER_CHILD_SHOW: obs_scene_render_enum = 1;
/// `OBS_SCENE_RENDER_CHILD_HIDE`.
pub const OBS_SCENE_RENDER_CHILD_HIDE: obs_scene_render_enum = 2;

/// `OBS_SCENE_RENDER_EFFECT_DEFAULT`.
pub const OBS_SCENE_RENDER_EFFECT_DEFAULT: c_int = 0;
/// `OBS_SCENE_RENDER_EFFECT_BILINEAR_LOWRES`.
pub const OBS_SCENE_RENDER_EFFECT_BILINEAR_LOWRES: c_int = 1;
/// `OBS_SCENE_RENDER_EFFECT_BICUBIC`.
pub const OBS_SCENE_RENDER_EFFECT_BICUBIC: c_int = 2;
/// `OBS_SCENE_RENDER_EFFECT_LANCZOS`.
pub const OBS_SCENE_RENDER_EFFECT_LANCZOS: c_int = 3;
/// `OBS_SCENE_RENDER_EFFECT_AREA`.
pub const OBS_SCENE_RENDER_EFFECT_AREA: c_int = 4;

/// `OBS_SCENE_RENDER_TECH_DRAW` .. `OBS_SCENE_RENDER_TECH_DRAW_UPSCALE_MULTIPLY_TONEMAP`
/// are 0 to 7, in the order of [`Technique`].
pub const OBS_SCENE_RENDER_TECH_DRAW: obs_scene_render_enum = 0;
/// `OBS_SCENE_RENDER_TECH_DRAW_MULTIPLY`.
pub const OBS_SCENE_RENDER_TECH_DRAW_MULTIPLY: obs_scene_render_enum = 1;
/// `OBS_SCENE_RENDER_TECH_DRAW_TONEMAP`.
pub const OBS_SCENE_RENDER_TECH_DRAW_TONEMAP: obs_scene_render_enum = 2;
/// `OBS_SCENE_RENDER_TECH_DRAW_MULTIPLY_TONEMAP`.
pub const OBS_SCENE_RENDER_TECH_DRAW_MULTIPLY_TONEMAP: obs_scene_render_enum = 3;
/// `OBS_SCENE_RENDER_TECH_DRAW_UPSCALE`.
pub const OBS_SCENE_RENDER_TECH_DRAW_UPSCALE: obs_scene_render_enum = 4;
/// `OBS_SCENE_RENDER_TECH_DRAW_UPSCALE_MULTIPLY`.
pub const OBS_SCENE_RENDER_TECH_DRAW_UPSCALE_MULTIPLY: obs_scene_render_enum = 5;
/// `OBS_SCENE_RENDER_TECH_DRAW_UPSCALE_TONEMAP`.
pub const OBS_SCENE_RENDER_TECH_DRAW_UPSCALE_TONEMAP: obs_scene_render_enum = 6;
/// `OBS_SCENE_RENDER_TECH_DRAW_UPSCALE_MULTIPLY_TONEMAP`.
pub const OBS_SCENE_RENDER_TECH_DRAW_UPSCALE_MULTIPLY_TONEMAP: obs_scene_render_enum = 7;

impl From<obs_scene_render_triggers> for Triggers {
    fn from(t: obs_scene_render_triggers) -> Self {
        Self {
            crop_left: t.crop_left,
            crop_top: t.crop_top,
            crop_right: t.crop_right,
            crop_bottom: t.crop_bottom,
            bounds_crop_left: t.bounds_crop_left,
            bounds_crop_top: t.bounds_crop_top,
            bounds_crop_right: t.bounds_crop_right,
            bounds_crop_bottom: t.bounds_crop_bottom,
            scale_filter: t.scale_filter,
            blend_method: t.blend_method,
            blend_type: t.blend_type,
            is_scene: t.is_scene,
            is_group: t.is_group,
        }
    }
}

const fn texrender_value(action: Texrender) -> obs_scene_render_enum {
    match action {
        Texrender::None => OBS_SCENE_RENDER_TEXRENDER_NONE,
        Texrender::Keep => OBS_SCENE_RENDER_TEXRENDER_KEEP,
        Texrender::Destroy => OBS_SCENE_RENDER_TEXRENDER_DESTROY,
        Texrender::Create => OBS_SCENE_RENDER_TEXRENDER_CREATE,
        Texrender::Recreate => OBS_SCENE_RENDER_TEXRENDER_RECREATE,
    }
}

const fn child_value(child: Child) -> obs_scene_render_enum {
    match child {
        Child::Source => OBS_SCENE_RENDER_CHILD_SOURCE,
        Child::Show => OBS_SCENE_RENDER_CHILD_SHOW,
        Child::Hide => OBS_SCENE_RENDER_CHILD_HIDE,
    }
}

const fn effect_value(effect: Effect) -> c_int {
    match effect {
        Effect::Default => OBS_SCENE_RENDER_EFFECT_DEFAULT,
        Effect::BilinearLowres => OBS_SCENE_RENDER_EFFECT_BILINEAR_LOWRES,
        Effect::Bicubic => OBS_SCENE_RENDER_EFFECT_BICUBIC,
        Effect::Lanczos => OBS_SCENE_RENDER_EFFECT_LANCZOS,
        Effect::Area => OBS_SCENE_RENDER_EFFECT_AREA,
    }
}

const fn technique_value(tech: Technique) -> obs_scene_render_enum {
    match tech {
        Technique::Draw => OBS_SCENE_RENDER_TECH_DRAW,
        Technique::DrawMultiply => OBS_SCENE_RENDER_TECH_DRAW_MULTIPLY,
        Technique::DrawTonemap => OBS_SCENE_RENDER_TECH_DRAW_TONEMAP,
        Technique::DrawMultiplyTonemap => OBS_SCENE_RENDER_TECH_DRAW_MULTIPLY_TONEMAP,
        Technique::DrawUpscale => OBS_SCENE_RENDER_TECH_DRAW_UPSCALE,
        Technique::DrawUpscaleMultiply => OBS_SCENE_RENDER_TECH_DRAW_UPSCALE_MULTIPLY,
        Technique::DrawUpscaleTonemap => OBS_SCENE_RENDER_TECH_DRAW_UPSCALE_TONEMAP,
        Technique::DrawUpscaleMultiplyTonemap => {
            OBS_SCENE_RENDER_TECH_DRAW_UPSCALE_MULTIPLY_TONEMAP
        }
    }
}

/// `bool obs_scene_render_texture_enabled(const struct
/// obs_scene_render_triggers *triggers)`.
///
/// # Safety
///
/// `triggers` points to an initialized, aligned struct that nothing writes
/// during the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_scene_render_texture_enabled(
    triggers: *const obs_scene_render_triggers,
) -> bool {
    // SAFETY: the caller guarantees `triggers` is readable.
    let t = unsafe { ptr::read(triggers) };
    core::texture_enabled(&Triggers::from(t))
}

/// `enum obs_scene_render_texrender obs_scene_render_texrender_action(bool
/// has_texrender, bool use_texrender, bool format_matches)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_scene_render_texrender_action(
    has_texrender: bool,
    use_texrender: bool,
    format_matches: bool,
) -> obs_scene_render_enum {
    texrender_value(core::texrender_action(
        has_texrender,
        use_texrender,
        format_matches,
    ))
}

/// `bool obs_scene_render_size_empty(uint32_t width, uint32_t height)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_scene_render_size_empty(width: u32, height: u32) -> bool {
    core::size_empty(width, height)
}

/// `bool obs_scene_render_clear_white(int blend_type)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_scene_render_clear_white(blend_type: c_int) -> bool {
    core::clear_white(blend_type)
}

/// `enum obs_scene_render_child_id obs_scene_render_child(bool user_visible,
/// bool show_active, bool hide_active)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_scene_render_child(
    user_visible: bool,
    show_active: bool,
    hide_active: bool,
) -> obs_scene_render_enum {
    child_value(core::child(user_visible, show_active, hide_active))
}

/// `bool obs_scene_render_linear_srgb(bool has_texrender, int
/// blend_method)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_scene_render_linear_srgb(has_texrender: bool, blend_method: c_int) -> bool {
    core::linear_srgb(has_texrender, blend_method)
}

/// `bool obs_scene_texcoords_centered(const struct matrix4 *m)`.
///
/// # Safety
///
/// `m` points to an initialized, aligned `struct matrix4` that nothing
/// writes during the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_scene_texcoords_centered(m: *const matrix4) -> bool {
    // SAFETY: the caller guarantees `m` is readable.
    let m = unsafe { ptr::read(m) };
    core::texcoords_centered(&Matrix4::from(m))
}

/// `void obs_scene_render_effect(int scale_filter, float scale_x, float
/// scale_y, struct obs_scene_render_effect_plan *plan)`.
///
/// # Safety
///
/// `plan` is valid for writes of the struct and aligned.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_scene_render_effect(
    scale_filter: c_int,
    scale_x: f32,
    scale_y: f32,
    plan: *mut obs_scene_render_effect_plan,
) {
    let p = core::effect(scale_filter, scale_x, scale_y);
    let out = obs_scene_render_effect_plan {
        effect: effect_value(p.effect),
        point_sampler: p.point_sampler,
        base_dimension: p.base_dimension,
        upscale: p.upscale,
    };
    // SAFETY: the caller guarantees `plan` is writable.
    unsafe { ptr::write(plan, out) };
}

/// `float obs_scene_render_multiplier(int current_space, int source_space,
/// float sdr_white_level)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_scene_render_multiplier(
    current_space: c_int,
    source_space: c_int,
    sdr_white_level: f32,
) -> f32 {
    core::multiplier(current_space, source_space, sdr_white_level)
}

/// `enum obs_scene_render_technique_id obs_scene_render_technique(bool
/// upscale, int current_space, int source_space)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_scene_render_technique(
    upscale: bool,
    current_space: c_int,
    source_space: c_int,
) -> obs_scene_render_enum {
    technique_value(core::technique(upscale, current_space, source_space))
}
