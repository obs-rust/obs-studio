//! `oracle_*` build of `libobs/obs-scene-render-plan.c.h`
//! (oracle/scene_render_plan.c), with the layouts and values its header
//! gives the C.

use std::ffi::c_int;

use super::graphics_math::OracleMatrix4;

/// `struct obs_scene_render_triggers`, as the oracle sees it.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub struct oracle_scene_render_triggers {
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

/// `struct obs_scene_render_effect_plan`, as the oracle sees it.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub struct oracle_scene_render_effect_plan {
    pub effect: c_int,
    pub point_sampler: bool,
    pub base_dimension: bool,
    pub upscale: bool,
}

/// How many values `oracle_scene_render_plan_layout` gives, in the order of
/// oracle/scene_render_plan.c.
pub const LAYOUT_COUNT: usize = 59;

unsafe extern "C" {
    pub fn oracle_obs_scene_render_texture_enabled(
        triggers: *const oracle_scene_render_triggers,
    ) -> bool;

    pub fn oracle_obs_scene_render_texrender_action(
        has_texrender: bool,
        use_texrender: bool,
        format_matches: bool,
    ) -> u32;

    pub fn oracle_obs_scene_render_size_empty(width: u32, height: u32) -> bool;

    pub fn oracle_obs_scene_render_clear_white(blend_type: c_int) -> bool;

    pub fn oracle_obs_scene_render_child(
        user_visible: bool,
        show_active: bool,
        hide_active: bool,
    ) -> u32;

    pub fn oracle_obs_scene_render_linear_srgb(has_texrender: bool, blend_method: c_int) -> bool;

    pub fn oracle_obs_scene_texcoords_centered(m: *const OracleMatrix4) -> bool;

    pub fn oracle_obs_scene_render_effect(
        scale_filter: c_int,
        scale_x: f32,
        scale_y: f32,
        plan: *mut oracle_scene_render_effect_plan,
    );

    pub fn oracle_obs_scene_render_multiplier(
        current_space: c_int,
        source_space: c_int,
        sdr_white_level: f32,
    ) -> f32;

    pub fn oracle_obs_scene_render_technique(
        upscale: bool,
        current_space: c_int,
        source_space: c_int,
    ) -> u32;

    pub fn oracle_scene_render_plan_layout(which: usize) -> usize;
}
