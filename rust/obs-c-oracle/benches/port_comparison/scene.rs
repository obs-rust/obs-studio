//! obs-scene: the content extent (`libobs/obs-scene-extent.c.h`), the fold
//! `resize_scene_base` runs over every item of a scene or group each time
//! one is resized. Both sides map the corners through `vec3_transform`'s
//! sum order: the C calls the graphics oracle out of line, the Rust core
//! has it inline.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::graphics_math::{OracleMatrix4, OracleVec4};
use obs_c_oracle::scene_extent::{
    oracle_obs_scene_extent_add, oracle_obs_scene_extent_init, oracle_obs_scene_extent_size,
};
use obs_c_oracle::vec2::OracleVec2;
use obs_graphics::ffi::matrix4::matrix4;
use obs_graphics::ffi::vec2::vec2;
use obs_graphics::ffi::vec4::vec4;
use obs_scene::ffi::scene_extent::{
    obs_scene_extent_add, obs_scene_extent_init, obs_scene_extent_size,
};
use std::hint::black_box;

/// Item `i` of a scene: a 320 x 180 box turned a little and moved along.
fn rows(i: usize) -> [[f32; 4]; 4] {
    let f = i as f32;
    let (c, s) = ((0.1 * f).cos(), (0.1 * f).sin());
    [
        [320.0 * c, 320.0 * s, 0.0, 0.0],
        [-180.0 * s, 180.0 * c, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [40.0 * f, 25.0 * f, 0.0, 1.0],
    ]
}

fn rust_matrix(r: [[f32; 4]; 4]) -> matrix4 {
    let v = |[x, y, z, w]: [f32; 4]| vec4 { x, y, z, w };
    matrix4 {
        x: v(r[0]),
        y: v(r[1]),
        z: v(r[2]),
        t: v(r[3]),
    }
}

fn c_matrix(r: [[f32; 4]; 4]) -> OracleMatrix4 {
    let v = |[x, y, z, w]: [f32; 4]| OracleVec4 { x, y, z, w };
    OracleMatrix4 {
        x: v(r[0]),
        y: v(r[1]),
        z: v(r[2]),
        t: v(r[3]),
    }
}

fn bits(v: [f32; 4]) -> [u32; 4] {
    v.map(f32::to_bits)
}

fn rust_fold(items: &[matrix4]) -> ([u32; 4], u32, u32) {
    let (mut min, mut max) = (vec2::default(), vec2::default());
    let mut scale = vec2::default();
    let (mut cx, mut cy) = (0, 0);
    // SAFETY: live, distinct stack values; the matrices are only read.
    unsafe {
        obs_scene_extent_init(&raw mut min, &raw mut max);
        for m in items {
            obs_scene_extent_add(&raw mut min, &raw mut max, black_box(m));
        }
        obs_scene_extent_size(
            &raw mut scale,
            &raw mut cx,
            &raw mut cy,
            &raw const min,
            &raw const max,
        );
    }
    (bits([min.x, min.y, scale.x, scale.y]), cx, cy)
}

fn c_fold(items: &[OracleMatrix4]) -> ([u32; 4], u32, u32) {
    let (mut min, mut max) = (OracleVec2::default(), OracleVec2::default());
    let mut scale = OracleVec2::default();
    let (mut cx, mut cy) = (0, 0);
    // SAFETY: as above, for the oracle's own structs.
    unsafe {
        oracle_obs_scene_extent_init(&raw mut min, &raw mut max);
        for m in items {
            oracle_obs_scene_extent_add(&raw mut min, &raw mut max, black_box(m));
        }
        oracle_obs_scene_extent_size(
            &raw mut scale,
            &raw mut cx,
            &raw mut cy,
            &raw const min,
            &raw const max,
        );
    }
    (bits([min.x, min.y, scale.x, scale.y]), cx, cy)
}

pub fn benchmarks(criterion: &mut Criterion) {
    for count in [1, 16] {
        let rust: Vec<matrix4> = (0..count).map(|i| rust_matrix(rows(i))).collect();
        let c: Vec<OracleMatrix4> = (0..count).map(|i| c_matrix(rows(i))).collect();
        pair(
            criterion,
            &format!("scene_extent/fold/{count}"),
            || rust_fold(&rust),
            || c_fold(&c),
        );
    }
}
