//! obs-scene: the audio window decisions of `scene_audio_render()`
//! (`libobs/obs-scene-audio-window.c.h`). Each audio tick, a scene asks
//! every item which source to read and whether its timestamp counts towards
//! the tick's earliest, then, unless the scene is silent, every item's
//! window, whether it is mixed and whether its mix is added. Each side runs
//! those calls for a scene of 16 items, in `scene_audio_render()`'s order
//! with the reads it asks for, at 48 kHz, folded into a checksum both sides
//! must match. Nothing is allocated.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::scene_audio_window::{
    oracle_obs_scene_audio_window_adds, oracle_obs_scene_audio_window_counts,
    oracle_obs_scene_audio_window_earliest, oracle_obs_scene_audio_window_frames,
    oracle_obs_scene_audio_window_mixes, oracle_obs_scene_audio_window_pick,
    oracle_obs_scene_audio_window_silent,
};
use obs_scene::ffi::scene_audio_window::{
    obs_scene_audio_window_adds, obs_scene_audio_window_counts, obs_scene_audio_window_earliest,
    obs_scene_audio_window_frames, obs_scene_audio_window_mixes, obs_scene_audio_window_pick,
    obs_scene_audio_window_silent,
};
use std::hint::black_box;

const TICK: u64 = 1_000_000_000_000;
const RATE: usize = 48000;

/// Item `i`: its visibility, transitions, pending state, timestamp (on the
/// tick, a few frames in, past the tick, or none), volume buffer and
/// duplicated flag.
#[derive(Clone, Copy)]
struct Item {
    visible: bool,
    show_active: bool,
    hide_active: bool,
    pending: bool,
    ts: u64,
    apply_buf: bool,
    duplicated: bool,
}

fn items() -> Vec<Item> {
    (0..16u64)
        .map(|i| Item {
            visible: i % 5 != 0,
            show_active: i % 6 == 0,
            hide_active: i % 5 == 0,
            pending: i % 7 == 3,
            ts: match i % 4 {
                0 => TICK,
                1 => TICK + i * 20_834,
                2 => TICK + 30_000_000,
                _ => 0,
            },
            apply_buf: i % 3 == 0,
            duplicated: i % 8 == 7,
        })
        .collect()
}

fn rust_scene(items: &[Item]) -> u64 {
    let mut sum = 0u64;
    let mut timestamp = 0;
    for it in items {
        let it = black_box(*it);
        let show = it.visible && it.show_active;
        let hide = !show && !it.visible && it.hide_active;
        let child = obs_scene_audio_window_pick(it.visible, show, hide);
        let (visible, hiding) = if it.pending {
            (false, false)
        } else {
            (it.visible, !it.visible && it.hide_active)
        };
        if obs_scene_audio_window_counts(it.pending, visible, hiding) {
            timestamp = obs_scene_audio_window_earliest(timestamp, it.ts);
        }
        sum = sum
            .wrapping_mul(31)
            .wrapping_add(u64::from(child.unsigned_abs()));
    }
    if obs_scene_audio_window_silent(timestamp) {
        return sum;
    }
    for it in items {
        let it = black_box(*it);
        let show = it.visible && it.show_active;
        let hide = !show && !it.visible && it.hide_active;
        let child = obs_scene_audio_window_pick(it.visible, show, hide);
        let source_ts = if it.pending { 0 } else { it.ts };
        let w = obs_scene_audio_window_frames(it.pending, source_ts, timestamp, RATE);
        let mut v = u64::from(child.unsigned_abs());
        if w.mix {
            let (visible, hiding) = if it.apply_buf {
                (false, false)
            } else {
                (it.visible, !it.visible && it.hide_active)
            };
            if obs_scene_audio_window_mixes(it.apply_buf, visible, hiding) {
                v = v
                    .wrapping_add(u64::try_from(w.pos).unwrap_or(0) << 8)
                    .wrapping_add(u64::from(obs_scene_audio_window_adds(it.duplicated)) << 20);
            }
        }
        sum = sum.wrapping_mul(31).wrapping_add(v);
    }
    sum ^ timestamp
}

fn c_scene(items: &[Item]) -> u64 {
    let mut sum = 0u64;
    let mut timestamp = 0;
    // SAFETY (every oracle call below): the oracle takes plain values and
    // touches no memory.
    unsafe {
        for it in items {
            let it = black_box(*it);
            let show = it.visible && it.show_active;
            let hide = !show && !it.visible && it.hide_active;
            let child = oracle_obs_scene_audio_window_pick(it.visible, show, hide);
            let (visible, hiding) = if it.pending {
                (false, false)
            } else {
                (it.visible, !it.visible && it.hide_active)
            };
            if oracle_obs_scene_audio_window_counts(it.pending, visible, hiding) {
                timestamp = oracle_obs_scene_audio_window_earliest(timestamp, it.ts);
            }
            sum = sum
                .wrapping_mul(31)
                .wrapping_add(u64::from(child.unsigned_abs()));
        }
        if oracle_obs_scene_audio_window_silent(timestamp) {
            return sum;
        }
        for it in items {
            let it = black_box(*it);
            let show = it.visible && it.show_active;
            let hide = !show && !it.visible && it.hide_active;
            let child = oracle_obs_scene_audio_window_pick(it.visible, show, hide);
            let source_ts = if it.pending { 0 } else { it.ts };
            let w = oracle_obs_scene_audio_window_frames(it.pending, source_ts, timestamp, RATE);
            let mut v = u64::from(child.unsigned_abs());
            if w.mix {
                let (visible, hiding) = if it.apply_buf {
                    (false, false)
                } else {
                    (it.visible, !it.visible && it.hide_active)
                };
                if oracle_obs_scene_audio_window_mixes(it.apply_buf, visible, hiding) {
                    v = v
                        .wrapping_add(u64::try_from(w.pos).unwrap_or(0) << 8)
                        .wrapping_add(
                            u64::from(oracle_obs_scene_audio_window_adds(it.duplicated)) << 20,
                        );
                }
            }
            sum = sum.wrapping_mul(31).wrapping_add(v);
        }
    }
    sum ^ timestamp
}

pub fn benchmarks(criterion: &mut Criterion) {
    let items = items();
    assert_eq!(rust_scene(&items), c_scene(&items), "checksums differ");
    pair(
        criterion,
        "scene_audio_window/scene/16",
        || rust_scene(&items),
        || c_scene(&items),
    );
}
