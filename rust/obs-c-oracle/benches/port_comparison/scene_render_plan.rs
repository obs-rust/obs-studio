//! obs-scene: the scene item render decisions
//! (`libobs/obs-scene-render-plan.c.h`). Each frame, `render_item` asks
//! every item whether it is drawn through its texrender, what happens to
//! the texrender, its clear colour, its child and its linear sRGB state;
//! an item drawn directly asks whether its texture coordinates are
//! centred, and one drawn through its texrender asks
//! `render_item_texture` for its effect, multiplier and technique. Each
//! side runs the calls for 16 items, folded into a checksum both sides
//! must match.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::graphics_math::{OracleMatrix4, OracleVec4};
use obs_c_oracle::scene_render_plan::{
    oracle_obs_scene_render_child, oracle_obs_scene_render_clear_white,
    oracle_obs_scene_render_effect, oracle_obs_scene_render_linear_srgb,
    oracle_obs_scene_render_multiplier, oracle_obs_scene_render_technique,
    oracle_obs_scene_render_texrender_action, oracle_obs_scene_render_texture_enabled,
    oracle_obs_scene_texcoords_centered, oracle_scene_render_effect_plan,
    oracle_scene_render_triggers,
};
use obs_graphics::ffi::matrix4::matrix4;
use obs_graphics::ffi::vec4::vec4;
use obs_scene::ffi::scene_render_plan::{
    obs_scene_render_child, obs_scene_render_clear_white, obs_scene_render_effect,
    obs_scene_render_effect_plan, obs_scene_render_linear_srgb, obs_scene_render_multiplier,
    obs_scene_render_technique, obs_scene_render_texrender_action,
    obs_scene_render_texture_enabled, obs_scene_render_triggers, obs_scene_texcoords_centered,
};
use std::hint::black_box;

/// Item `i`: its triggers, whether it has a texrender in the format, its
/// visibility and transitions, its output scale, its colour spaces and its
/// draw transform (whole or fractional translations, some scaled).
#[derive(Clone, Copy)]
struct Item {
    triggers: obs_scene_render_triggers,
    has_texrender: bool,
    format_matches: bool,
    visible: bool,
    show_active: bool,
    hide_active: bool,
    scale: (f32, f32),
    current_space: i32,
    source_space: i32,
    transform: [f32; 16],
}

fn items() -> Vec<Item> {
    (0..16i32)
        .map(|i| {
            let f = |n: i32| f32::from(i16::try_from(n).unwrap_or(0));
            Item {
                triggers: obs_scene_render_triggers {
                    crop_left: i % 3,
                    crop_top: 0,
                    crop_right: i % 5,
                    crop_bottom: 0,
                    bounds_crop_left: 0,
                    bounds_crop_top: i % 7,
                    bounds_crop_right: 0,
                    bounds_crop_bottom: 0,
                    scale_filter: i % 6,
                    blend_method: i % 2,
                    blend_type: i % 7,
                    is_scene: i % 4 == 0,
                    is_group: i % 8 == 0,
                },
                has_texrender: i % 2 == 0,
                format_matches: i % 3 != 0,
                visible: i % 5 != 0,
                show_active: i % 6 == 0,
                hide_active: i % 5 == 0,
                scale: (0.25 + 0.125 * f(i), 1.75 - 0.0625 * f(i)),
                current_space: i % 4,
                source_space: (i / 4) % 4,
                transform: [
                    1.0 + 0.5 * f(i % 2),
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    1.0,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    1.0,
                    0.0,
                    f(37 * i) + 0.5 * f(i % 3),
                    f(19 * i),
                    0.0,
                    1.0,
                ],
            }
        })
        .collect()
}

fn oracle_triggers(t: &obs_scene_render_triggers) -> oracle_scene_render_triggers {
    oracle_scene_render_triggers {
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

fn shim_matrix(m: &[f32; 16]) -> matrix4 {
    let row = |i: usize| vec4 {
        x: m[i],
        y: m[i + 1],
        z: m[i + 2],
        w: m[i + 3],
    };
    matrix4 {
        x: row(0),
        y: row(4),
        z: row(8),
        t: row(12),
    }
}

fn oracle_matrix(m: &[f32; 16]) -> OracleMatrix4 {
    let row = |i: usize| OracleVec4 {
        x: m[i],
        y: m[i + 1],
        z: m[i + 2],
        w: m[i + 3],
    };
    OracleMatrix4 {
        x: row(0),
        y: row(4),
        z: row(8),
        t: row(12),
    }
}

fn rust_items(items: &[Item]) -> u64 {
    items.iter().fold(0, |acc, it| {
        let it = black_box(*it);
        // SAFETY: reads a local struct.
        let wanted = unsafe { obs_scene_render_texture_enabled(&it.triggers) };
        let action = obs_scene_render_texrender_action(
            it.has_texrender,
            wanted,
            it.has_texrender && wanted && it.format_matches,
        );
        let white = obs_scene_render_clear_white(it.triggers.blend_type);
        let child = obs_scene_render_child(it.visible, it.show_active, it.hide_active);
        let linear = obs_scene_render_linear_srgb(wanted, it.triggers.blend_method);
        acc.rotate_left(7)
            ^ u64::from(wanted)
            ^ u64::from(action) << 1
            ^ u64::from(white) << 4
            ^ u64::from(child) << 5
            ^ u64::from(linear) << 7
    })
}

fn c_items(items: &[Item]) -> u64 {
    items.iter().fold(0, |acc, it| {
        let it = black_box(*it);
        let t = oracle_triggers(&it.triggers);
        // SAFETY: the oracle reads a local struct and takes plain values.
        let (wanted, action, white, child, linear) = unsafe {
            let wanted = oracle_obs_scene_render_texture_enabled(&t);
            (
                wanted,
                oracle_obs_scene_render_texrender_action(
                    it.has_texrender,
                    wanted,
                    it.has_texrender && wanted && it.format_matches,
                ),
                oracle_obs_scene_render_clear_white(it.triggers.blend_type),
                oracle_obs_scene_render_child(it.visible, it.show_active, it.hide_active),
                oracle_obs_scene_render_linear_srgb(wanted, it.triggers.blend_method),
            )
        };
        acc.rotate_left(7)
            ^ u64::from(wanted)
            ^ u64::from(action) << 1
            ^ u64::from(white) << 4
            ^ u64::from(child) << 5
            ^ u64::from(linear) << 7
    })
}

fn rust_textures(items: &[Item]) -> u64 {
    items.iter().fold(0, |acc, it| {
        let it = black_box(*it);
        let mut plan = obs_scene_render_effect_plan::default();
        // SAFETY: writes a local struct.
        unsafe {
            obs_scene_render_effect(
                it.triggers.scale_filter,
                it.scale.0,
                it.scale.1,
                &raw mut plan,
            );
        }
        let m = obs_scene_render_multiplier(it.current_space, it.source_space, 300.0);
        let tech = obs_scene_render_technique(plan.upscale, it.current_space, it.source_space);
        acc.rotate_left(9)
            ^ u64::from(plan.effect.unsigned_abs())
            ^ u64::from(plan.point_sampler) << 3
            ^ u64::from(plan.base_dimension) << 4
            ^ u64::from(plan.upscale) << 5
            ^ u64::from(tech) << 6
            ^ u64::from(m.to_bits()) << 9
    })
}

fn c_textures(items: &[Item]) -> u64 {
    items.iter().fold(0, |acc, it| {
        let it = black_box(*it);
        let mut plan = oracle_scene_render_effect_plan::default();
        // SAFETY: the oracle writes a local struct and takes plain values.
        let (m, tech) = unsafe {
            oracle_obs_scene_render_effect(
                it.triggers.scale_filter,
                it.scale.0,
                it.scale.1,
                &raw mut plan,
            );
            (
                oracle_obs_scene_render_multiplier(it.current_space, it.source_space, 300.0),
                oracle_obs_scene_render_technique(plan.upscale, it.current_space, it.source_space),
            )
        };
        acc.rotate_left(9)
            ^ u64::from(plan.effect.unsigned_abs())
            ^ u64::from(plan.point_sampler) << 3
            ^ u64::from(plan.base_dimension) << 4
            ^ u64::from(plan.upscale) << 5
            ^ u64::from(tech) << 6
            ^ u64::from(m.to_bits()) << 9
    })
}

pub fn benchmarks(criterion: &mut Criterion) {
    let items = items();
    let shim_matrices: Vec<matrix4> = items.iter().map(|it| shim_matrix(&it.transform)).collect();
    let oracle_matrices: Vec<OracleMatrix4> = items
        .iter()
        .map(|it| oracle_matrix(&it.transform))
        .collect();
    pair(
        criterion,
        "scene_render_plan/item/16",
        || rust_items(&items),
        || c_items(&items),
    );
    pair(
        criterion,
        "scene_render_plan/texture/16",
        || rust_textures(&items),
        || c_textures(&items),
    );
    pair(
        criterion,
        "scene_render_plan/texcoords_centered/16",
        || {
            shim_matrices.iter().fold(0u32, |acc, m| {
                // SAFETY: reads a local matrix.
                (acc << 1) | u32::from(unsafe { obs_scene_texcoords_centered(black_box(m)) })
            })
        },
        || {
            oracle_matrices.iter().fold(0u32, |acc, m| {
                // SAFETY: as above.
                (acc << 1) | u32::from(unsafe { oracle_obs_scene_texcoords_centered(black_box(m)) })
            })
        },
    );
}
