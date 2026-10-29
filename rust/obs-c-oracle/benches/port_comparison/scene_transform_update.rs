//! obs-scene: the transform update decisions
//! (`libobs/obs-scene-transform-update.c.h`). On each frame,
//! `scene_video_render` asks once whether the scene has a new size, and
//! `update_transforms_and_prune_sources` asks every item whether its
//! transform is computed again. Each side runs 60 frames of a 16-item scene
//! (a new scene size now and then, a few flags and new source sizes),
//! folded into a checksum both sides must match. Nothing is allocated.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::scene_transform_update::{
    oracle_obs_scene_transform_update_item, oracle_obs_scene_transform_update_scene_size,
};
use obs_scene::ffi::scene_transform_update::{
    obs_scene_transform_update_item, obs_scene_transform_update_scene_size,
};
use std::hint::black_box;

const FRAMES: u32 = 60;
const ITEMS: u32 = 16;

/// Frame `f`'s scene size: 1920 x 1080, 1280 x 720 from frame 30.
fn scene_size(f: u32) -> (u32, u32) {
    if f < 30 { (1920, 1080) } else { (1280, 720) }
}

/// Item `i` on frame `f`: its flag, its last size and its source size (one
/// in seven items has a new source size, one in five its flag set).
fn item(f: u32, i: u32) -> (bool, u32, u32, u32, u32) {
    let k = f * ITEMS + i;
    let update = k.is_multiple_of(5);
    let (w, h) = (320 + i * 40, 180 + i * 20);
    let grown = if k.is_multiple_of(7) { 2 } else { 0 };
    let (sw, sh) = if update { (0, 0) } else { (w + grown, h) };
    (update, w, h, sw, sh)
}

fn rust_frames() -> u32 {
    let (mut lw, mut lh) = (0_u32, 0_u32);
    let mut acc = 0_u32;
    for f in 0..FRAMES {
        let (w, h) = black_box(scene_size(f));
        // SAFETY: two locals, written only by the call.
        let changed = unsafe { obs_scene_transform_update_scene_size(&mut lw, &mut lh, w, h) };
        acc = acc.wrapping_mul(31).wrapping_add(u32::from(changed));
        for i in 0..ITEMS {
            let (u, iw, ih, sw, sh) = black_box(item(f, i));
            let update = obs_scene_transform_update_item(u, iw, ih, sw, sh, changed);
            acc = acc.wrapping_mul(31).wrapping_add(u32::from(update));
        }
    }
    acc
}

fn c_frames() -> u32 {
    let (mut lw, mut lh) = (0_u32, 0_u32);
    let mut acc = 0_u32;
    for f in 0..FRAMES {
        let (w, h) = black_box(scene_size(f));
        // SAFETY: two locals, written only by the call; the oracle keeps no
        // state.
        let changed =
            unsafe { oracle_obs_scene_transform_update_scene_size(&mut lw, &mut lh, w, h) };
        acc = acc.wrapping_mul(31).wrapping_add(u32::from(changed));
        for i in 0..ITEMS {
            let (u, iw, ih, sw, sh) = black_box(item(f, i));
            // SAFETY: plain values in, a bool out.
            let update =
                unsafe { oracle_obs_scene_transform_update_item(u, iw, ih, sw, sh, changed) };
            acc = acc.wrapping_mul(31).wrapping_add(u32::from(update));
        }
    }
    acc
}

pub fn benchmarks(criterion: &mut Criterion) {
    pair(
        criterion,
        "scene_transform_update/frame/60x16",
        rust_frames,
        c_frames,
    );
}
