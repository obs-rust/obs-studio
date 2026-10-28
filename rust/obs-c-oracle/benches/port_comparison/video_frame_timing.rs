//! obs-core: the frame timing decisions of the graphics thread loop
//! (`libobs/obs-video-frame-timing.c.h`). The core video thread asks them
//! once per frame: the tick time in `tick_sources()` and the frame
//! statistics at the end of `obs_graphics_thread_loop()`. Each side runs 1024
//! frames at 60 fps (a late frame now and then) and folds the answers into a
//! checksum both must match. Nothing is allocated.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::video_frame_timing::{
    OracleSums, oracle_obs_video_frame_timing_stats, oracle_obs_video_frame_timing_tick,
};
use obs_core::ffi::video_frame_timing::{
    obs_video_frame_sums, obs_video_frame_timing_stats, obs_video_frame_timing_tick,
};
use std::hint::black_box;

const N: u64 = 1024;
const INTERVAL: u64 = 16_666_667; // 60 fps

/// Frame `i`: its tick time, the time after the sleep and its render time.
#[derive(Clone, Copy)]
struct Frame {
    last: u64,
    now: u64,
    render: u64,
}

/// 60 fps; every 29th frame is two intervals late; the first frame starts
/// with no last tick time.
fn frames() -> Vec<Frame> {
    let mut time = 1_000_000_000_000_u64;
    (0..N)
        .map(|i| {
            let step = if i % 29 == 28 { 3 * INTERVAL } else { INTERVAL };
            let frame = Frame {
                last: if i == 0 { 0 } else { time },
                now: time + step,
                render: 2_000_000 + (i % 97) * 10_000,
            };
            time += step;
            frame
        })
        .collect()
}

fn fold(acc: u64, values: &[u64]) -> u64 {
    values
        .iter()
        .fold(acc, |acc, v| acc.wrapping_mul(31).wrapping_add(*v))
}

fn rust_frames(frames: &[Frame]) -> u64 {
    let mut sums = obs_video_frame_sums::default();
    frames.iter().fold(0, |acc, f| {
        let f = black_box(*f);
        let tick = obs_video_frame_timing_tick(f.now, f.last, INTERVAL);
        let stats = obs_video_frame_timing_stats(sums, f.render, f.now, f.now.wrapping_sub(tick.delta_ns));
        sums = stats.sums;
        fold(
            acc,
            &[
                tick.delta_ns,
                u64::from(tick.seconds.to_bits()),
                stats.fps.to_bits(),
                stats.avg_frame_time_ns,
                u64::from(stats.report),
            ],
        )
    })
}

fn c_frames(frames: &[Frame]) -> u64 {
    let mut sums = OracleSums::default();
    frames.iter().fold(0, |acc, f| {
        let f = black_box(*f);
        // SAFETY: plain values in, structs out; the oracle keeps no state.
        let (tick, stats) = unsafe {
            let tick = oracle_obs_video_frame_timing_tick(f.now, f.last, INTERVAL);
            let stats =
                oracle_obs_video_frame_timing_stats(sums, f.render, f.now, f.now.wrapping_sub(tick.delta_ns));
            (tick, stats)
        };
        sums = stats.sums;
        fold(
            acc,
            &[
                tick.delta_ns,
                u64::from(tick.seconds.to_bits()),
                stats.fps.to_bits(),
                stats.avg_frame_time_ns,
                u64::from(stats.report),
            ],
        )
    })
}

pub fn benchmarks(criterion: &mut Criterion) {
    let f = frames();
    pair(
        criterion,
        "video_frame_timing/frame/1024",
        || rust_frames(&f),
        || c_frames(&f),
    );
}
