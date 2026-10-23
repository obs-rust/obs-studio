//! obs-scene: the scene item crop arithmetic (`libobs/obs-scene-crop.c.h`).
//! `update_item_transform` sizes every item it updates with two
//! `obs_scene_cropped_size` calls (`calc_cx`, `calc_cy`), a nested scene
//! that changed size rescales its four crops with `obs_scene_scale_crop`,
//! and `render_item` moves a cropped item by two `obs_scene_crop_offset`
//! calls each frame. Each side runs the calls for 16 items.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::scene_crop::{
    oracle_obs_scene_crop_offset, oracle_obs_scene_cropped_size, oracle_obs_scene_scale_crop,
};
use obs_scene::ffi::scene_crop::{
    obs_scene_crop_offset, obs_scene_cropped_size, obs_scene_scale_crop,
};
use std::hint::black_box;

/// Item `i`: a source size, its four crops and its bounds crops, and the
/// factor its nested scene changed size by.
#[derive(Clone, Copy)]
struct Item {
    size: u32,
    crop: [i32; 4],
    bounds: [i32; 2],
    factor: f32,
}

fn items() -> Vec<Item> {
    (0..16)
        .map(|i: i32| Item {
            size: 1920 - 37 * i.unsigned_abs(),
            crop: [3 * i, 5 * i, 7 * i, 11 * i],
            bounds: [i, 2 * i],
            factor: 0.75 + 0.125 * i as f32,
        })
        .collect()
}

fn rust_sizes(items: &[Item]) -> u64 {
    items.iter().fold(0, |acc, it| {
        let it = black_box(*it);
        let cx =
            obs_scene_cropped_size(it.size, it.crop[0], it.crop[2], it.bounds[0], it.bounds[1]);
        let cy =
            obs_scene_cropped_size(it.size, it.crop[1], it.crop[3], it.bounds[1], it.bounds[0]);
        acc + u64::from(cx) + u64::from(cy)
    })
}

fn c_sizes(items: &[Item]) -> u64 {
    items.iter().fold(0, |acc, it| {
        let it = black_box(*it);
        // SAFETY: the oracle takes and returns plain values.
        let (cx, cy) = unsafe {
            (
                oracle_obs_scene_cropped_size(
                    it.size,
                    it.crop[0],
                    it.crop[2],
                    it.bounds[0],
                    it.bounds[1],
                ),
                oracle_obs_scene_cropped_size(
                    it.size,
                    it.crop[1],
                    it.crop[3],
                    it.bounds[1],
                    it.bounds[0],
                ),
            )
        };
        acc + u64::from(cx) + u64::from(cy)
    })
}

fn rust_rescale(items: &[Item]) -> i64 {
    items.iter().fold(0, |acc, it| {
        let it = black_box(*it);
        acc + it
            .crop
            .iter()
            .map(|&c| i64::from(obs_scene_scale_crop(c, it.factor)))
            .sum::<i64>()
    })
}

fn c_rescale(items: &[Item]) -> i64 {
    items.iter().fold(0, |acc, it| {
        let it = black_box(*it);
        acc + it
            .crop
            .iter()
            // SAFETY: as above.
            .map(|&c| i64::from(unsafe { oracle_obs_scene_scale_crop(c, it.factor) }))
            .sum::<i64>()
    })
}

fn rust_offsets(items: &[Item]) -> u64 {
    items.iter().fold(0, |acc, it| {
        let it = black_box(*it);
        let x = obs_scene_crop_offset(it.crop[0], it.bounds[0]);
        let y = obs_scene_crop_offset(it.crop[1], it.bounds[1]);
        acc ^ u64::from(x.to_bits()) ^ (u64::from(y.to_bits()) << 32)
    })
}

fn c_offsets(items: &[Item]) -> u64 {
    items.iter().fold(0, |acc, it| {
        let it = black_box(*it);
        // SAFETY: as above.
        let (x, y) = unsafe {
            (
                oracle_obs_scene_crop_offset(it.crop[0], it.bounds[0]),
                oracle_obs_scene_crop_offset(it.crop[1], it.bounds[1]),
            )
        };
        acc ^ u64::from(x.to_bits()) ^ (u64::from(y.to_bits()) << 32)
    })
}

pub fn benchmarks(criterion: &mut Criterion) {
    let items = items();
    pair(
        criterion,
        "scene_crop/cropped_size/16",
        || rust_sizes(&items),
        || c_sizes(&items),
    );
    pair(
        criterion,
        "scene_crop/scale_crop/16",
        || rust_rescale(&items),
        || c_rescale(&items),
    );
    pair(
        criterion,
        "scene_crop/crop_offset/16",
        || rust_offsets(&items),
        || c_offsets(&items),
    );
}
