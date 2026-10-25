//! obs-scene: the scene item transform steps
//! (`libobs/obs-scene-item-transform.c.h`). `compute_item_transform`
//! runs them for every item it updates (a setter, or a source that changed
//! size): `obs_scene_item_transform_prepare`, `_bounded` twice, `_matrix`
//! twice (the draw and box transforms) and, without bounds,
//! `_box_scale`. Each side runs one update of 16 items, half of them
//! bounded, at rotations from 0 to 337.5 degrees, and folds every output's
//! bits into a checksum both sides must match. The glue between the steps
//! (the casts and `add_alignment`) is left out: it is the same C on both
//! sides of the swap.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::graphics_math::{OracleMatrix4, OracleVec4};
use obs_c_oracle::scene_bounds_place::OracleCrop;
use obs_c_oracle::scene_item_transform::{
    oracle_obs_scene_item_transform_bounded, oracle_obs_scene_item_transform_box_scale,
    oracle_obs_scene_item_transform_matrix, oracle_obs_scene_item_transform_prepare,
};
use obs_c_oracle::vec2::OracleVec2;
use obs_graphics::ffi::matrix4::matrix4;
use obs_graphics::ffi::vec2::vec2;
use obs_graphics::ffi::vec4::vec4;
use obs_scene::ffi::scene_bounds_place::SceneItemCrop;
use obs_scene::ffi::scene_item_transform::{
    obs_scene_item_transform_bounded, obs_scene_item_transform_box_scale,
    obs_scene_item_transform_matrix, obs_scene_item_transform_prepare,
};
use std::hint::black_box;

/// Item `i`: its crop, last size, scale, origin, rotation, position and
/// bounds type.
#[derive(Clone, Copy)]
struct Item {
    crop: [i32; 4],
    last: (u32, u32),
    scale: (f32, f32),
    origin: (f32, f32),
    rot: f32,
    pos: (f32, f32),
    bounds_type: i32,
}

#[allow(clippy::cast_precision_loss)]
fn items() -> Vec<Item> {
    (0..16)
        .map(|i: i32| Item {
            crop: [i, 2 * i, 3 * i, 4 * i],
            last: if i % 4 == 0 { (0, 0) } else { (960, 540) },
            scale: (0.5 + 0.25 * i as f32, -1.0 + 0.125 * i as f32),
            origin: (10.0 * i as f32, 5.0 * i as f32),
            rot: 22.5 * i as f32,
            pos: (100.0 + i as f32, 50.0 - i as f32),
            bounds_type: i % 2 * (i % 7),
        })
        .collect()
}

fn fold(acc: u64, v: u32) -> u64 {
    acc.rotate_left(5) ^ u64::from(v)
}

fn fold_matrix(acc: u64, lanes: [f32; 16]) -> u64 {
    lanes.iter().fold(acc, |a, l| fold(a, l.to_bits()))
}

fn rust_lanes(m: &matrix4) -> [f32; 16] {
    let r = |v: &vec4| [v.x, v.y, v.z, v.w];
    let (x, y, z, t) = (r(&m.x), r(&m.y), r(&m.z), r(&m.t));
    [
        x[0], x[1], x[2], x[3], y[0], y[1], y[2], y[3], z[0], z[1], z[2], z[3], t[0], t[1], t[2],
        t[3],
    ]
}

fn c_lanes(m: &OracleMatrix4) -> [f32; 16] {
    let r = |v: &OracleVec4| [v.x, v.y, v.z, v.w];
    let (x, y, z, t) = (r(&m.x), r(&m.y), r(&m.z), r(&m.t));
    [
        x[0], x[1], x[2], x[3], y[0], y[1], y[2], y[3], z[0], z[1], z[2], z[3], t[0], t[1], t[2],
        t[3],
    ]
}

fn rust_update(items: &[Item]) -> u64 {
    items.iter().fold(0, |acc, it| {
        let it = black_box(*it);
        let mut crop = SceneItemCrop {
            left: it.crop[0],
            top: it.crop[1],
            right: it.crop[2],
            bottom: it.crop[3],
        };
        let mut bounds_crop = crop;
        let (mut lw, mut lh) = it.last;
        let scale = vec2 {
            x: it.scale.0,
            y: it.scale.1,
        };
        let origin = vec2 {
            x: it.origin.0,
            y: it.origin.1,
        };
        let pos = vec2 {
            x: it.pos.0,
            y: it.pos.1,
        };
        let mut draw = matrix4::default();
        let mut boxed = matrix4::default();
        let mut box_scale = scale;
        // SAFETY: live locals, distinct as obs-scene.c passes the fields.
        let size = unsafe {
            let size = obs_scene_item_transform_prepare(
                &raw mut crop,
                &raw mut bounds_crop,
                &raw mut lw,
                &raw mut lh,
                1920,
                1080,
                true,
            );
            obs_scene_item_transform_matrix(
                &raw mut draw,
                &raw const scale,
                &raw const origin,
                it.rot,
                &raw const pos,
            );
            if !obs_scene_item_transform_bounded(it.bounds_type) {
                obs_scene_item_transform_box_scale(&raw mut box_scale, size.cx, size.cy);
            }
            obs_scene_item_transform_matrix(
                &raw mut boxed,
                &raw const box_scale,
                &raw const origin,
                it.rot,
                &raw const pos,
            );
            size
        };
        let acc = fold(fold(fold(acc, size.cx), size.cy), crop.left as u32);
        fold_matrix(fold_matrix(acc, rust_lanes(&draw)), rust_lanes(&boxed))
    })
}

fn c_update(items: &[Item]) -> u64 {
    items.iter().fold(0, |acc, it| {
        let it = black_box(*it);
        let mut crop = OracleCrop {
            left: it.crop[0],
            top: it.crop[1],
            right: it.crop[2],
            bottom: it.crop[3],
        };
        let mut bounds_crop = crop;
        let (mut lw, mut lh) = it.last;
        let scale = OracleVec2 {
            x: it.scale.0,
            y: it.scale.1,
        };
        let origin = OracleVec2 {
            x: it.origin.0,
            y: it.origin.1,
        };
        let pos = OracleVec2 {
            x: it.pos.0,
            y: it.pos.1,
        };
        let zero = OracleVec4 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 0.0,
        };
        let mut draw = OracleMatrix4 {
            x: zero,
            y: zero,
            z: zero,
            t: zero,
        };
        let mut boxed = draw;
        let mut box_scale = scale;
        // SAFETY: live locals, distinct as obs-scene.c passes the fields.
        let size = unsafe {
            let size = oracle_obs_scene_item_transform_prepare(
                &raw mut crop,
                &raw mut bounds_crop,
                &raw mut lw,
                &raw mut lh,
                1920,
                1080,
                true,
            );
            oracle_obs_scene_item_transform_matrix(
                &raw mut draw,
                &raw const scale,
                &raw const origin,
                it.rot,
                &raw const pos,
            );
            if !oracle_obs_scene_item_transform_bounded(it.bounds_type) {
                oracle_obs_scene_item_transform_box_scale(&raw mut box_scale, size.cx, size.cy);
            }
            oracle_obs_scene_item_transform_matrix(
                &raw mut boxed,
                &raw const box_scale,
                &raw const origin,
                it.rot,
                &raw const pos,
            );
            size
        };
        let acc = fold(fold(fold(acc, size.cx), size.cy), crop.left as u32);
        fold_matrix(fold_matrix(acc, c_lanes(&draw)), c_lanes(&boxed))
    })
}

pub(super) fn benchmarks(criterion: &mut Criterion) {
    let items = items();
    pair(
        criterion,
        "scene_item_transform/update_16_items",
        || rust_update(&items),
        || c_update(&items),
    );
}
