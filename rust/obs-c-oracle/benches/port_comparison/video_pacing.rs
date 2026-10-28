//! obs-core: the frame pacing decisions of `video_sleep()`
//! (`libobs/obs-video-pacing.c.h`). The core video thread asks them once per
//! frame: the frame time step after the sleep, and the group start test for
//! each ready encoder (usually none, a few at the start of an output). Each
//! side runs 1024 frames over fixed inputs and folds the answers into a
//! checksum both must match. Nothing is allocated.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::video_pacing::{
    oracle_obs_video_pacing_group_starts, oracle_obs_video_pacing_step,
};
use obs_core::ffi::video_pacing::{obs_video_pacing_group_starts, obs_video_pacing_step};
use std::hint::black_box;

const N: u64 = 1024;
const INTERVAL: u64 = 16_666_667; // 60 fps

/// One frame's inputs: the frame time, whether the sleep reached the next
/// frame time, the clock after a late wake-up, and one ready encoder's group.
#[derive(Clone, Copy)]
struct Frame {
    frame_time: u64,
    slept: bool,
    now: u64,
    started: u32,
    encoders: usize,
    start_timestamp: u64,
}

/// Frame `i`: on time most frames; every 17th wakes up 1 to 3 intervals
/// late (a slow render); every 64th has an encoder group waiting to start.
fn frames() -> Vec<Frame> {
    let mut time = 1_000_000_000_000_u64;
    (0..N)
        .map(|i| {
            let late = i % 17 == 16;
            let frame = Frame {
                frame_time: time,
                slept: !late,
                now: time + INTERVAL * (1 + i % 3) + i,
                started: u32::try_from(i % 3).unwrap_or(0),
                encoders: 2,
                start_timestamp: if i % 64 == 0 { 0 } else { time },
            };
            time += INTERVAL;
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
    frames.iter().fold(0, |acc, f| {
        let f = black_box(*f);
        let step = obs_video_pacing_step(f.frame_time, INTERVAL, f.slept, f.now);
        let starts = obs_video_pacing_group_starts(f.started, f.encoders, f.start_timestamp);
        fold(
            acc,
            &[
                step.frame_time,
                u64::from(step.count.unsigned_abs()),
                u64::from(starts),
            ],
        )
    })
}

fn c_frames(frames: &[Frame]) -> u64 {
    frames.iter().fold(0, |acc, f| {
        let f = black_box(*f);
        // SAFETY: plain values in, a struct and a bool out; the oracle keeps
        // no state.
        let (step, starts) = unsafe {
            (
                oracle_obs_video_pacing_step(f.frame_time, INTERVAL, f.slept, f.now),
                oracle_obs_video_pacing_group_starts(f.started, f.encoders, f.start_timestamp),
            )
        };
        fold(
            acc,
            &[
                step.frame_time,
                u64::from(step.count.unsigned_abs()),
                u64::from(starts),
            ],
        )
    })
}

pub fn benchmarks(criterion: &mut Criterion) {
    let f = frames();
    pair(
        criterion,
        "video_pacing/frame/1024",
        || rust_frames(&f),
        || c_frames(&f),
    );
}
