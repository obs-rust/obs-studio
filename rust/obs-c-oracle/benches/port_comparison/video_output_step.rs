//! obs-core: the per-frame video output step decisions
//! (`libobs/obs-video-output-step.c.h`). The core video thread asks them on
//! every frame of each mix: one activation in `update_active_state()`, the
//! ring twice, the download and output checks, the render targets and steps
//! and the stage in `output_frame()`, and the queue step, UV copy and count
//! once per frame handed to the GPU encoders: about a dozen calls per frame
//! at 60 fps. Each side runs 1024 frames over fixed inputs and folds the
//! answers into a checksum both must match. Nothing is allocated.

use super::pair;

use core::ffi::{c_int, c_long, c_void};

use criterion::Criterion;
use obs_c_oracle::video_output_step::{
    oracle_obs_video_step_activation, oracle_obs_video_step_download, oracle_obs_video_step_output,
    oracle_obs_video_step_queue, oracle_obs_video_step_queue_copy_uv,
    oracle_obs_video_step_queue_count, oracle_obs_video_step_render,
    oracle_obs_video_step_render_targets, oracle_obs_video_step_ring, oracle_obs_video_step_stage,
};
use obs_core::ffi::video_output_step::{
    obs_video_step_activation, obs_video_step_activation_input, obs_video_step_download,
    obs_video_step_output, obs_video_step_queue, obs_video_step_queue_copy_uv,
    obs_video_step_queue_count, obs_video_step_render, obs_video_step_render_targets,
    obs_video_step_ring, obs_video_step_stage,
};
use std::hint::black_box;

const N: u32 = 1024;

/// One frame's inputs: the was flags and counters, GPU conversion, the
/// channel count, the queue sizes, the repeat count, the cur texture and
/// whether the download is ready.
#[derive(Clone, Copy)]
struct Frame {
    input: obs_video_step_activation_input,
    conversion: bool,
    channels: usize,
    avail: usize,
    queue: usize,
    count: c_int,
    cur: c_int,
    ready: bool,
    share: bool,
}

/// Frame `i`: raw output, GPU encoding, both or neither, switching every
/// few frames; NV12 conversion (3 channels) or not; Windows's one encode
/// surface now and then; a repeat every seventh frame.
fn frames() -> Vec<Frame> {
    (0..N)
        .map(|i| {
            let phase = (i / 64) % 4;
            let raw = phase & 1 != 0;
            let gpu = phase & 2 != 0;
            let prev = ((i.wrapping_sub(1)) / 64) % 4;
            Frame {
                input: obs_video_step_activation_input {
                    raw_was_active: prev & 1 != 0,
                    gpu_was_active: prev & 2 != 0,
                    was_active: prev != 0,
                    raw_active: c_long::from(raw),
                    gpu_encoder_active: c_long::from(gpu),
                },
                conversion: !i.is_multiple_of(3),
                channels: if i.is_multiple_of(5) { 1 } else { 3 },
                avail: if i.is_multiple_of(11) { 0 } else { 48 * 2 },
                queue: if i.is_multiple_of(2) { 48 } else { 0 },
                count: if i.is_multiple_of(7) { 2 } else { 1 },
                cur: (i % 2) as c_int,
                ready: !i.is_multiple_of(13),
                share: false,
            }
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
        // SAFETY: a live input.
        let a = unsafe { obs_video_step_activation(&raw const f.input) };
        let ring = obs_video_step_ring(f.cur);
        let targets = obs_video_step_render_targets(a.raw_active, a.gpu_active);
        let steps = obs_video_step_render(a.raw_active, a.gpu_active, f.conversion);
        let stage = obs_video_step_stage(f.conversion, true, f.channels);
        let queue = obs_video_step_queue(f.avail, f.queue, f.count, a.raw_active);
        let uv = obs_video_step_queue_copy_uv(f.share);
        let left = obs_video_step_queue_count(f.count);
        let download = obs_video_step_download(a.raw_active);
        let output = obs_video_step_output(a.raw_active, f.ready);
        let next = obs_video_step_ring(f.cur);
        fold(
            acc,
            &[
                u64::from(a.clear_base) | u64::from(a.clear_raw) << 1 | u64::from(a.clear_gpu) << 2,
                ring.prev as u64,
                u64::from(targets.outputs) | u64::from(targets.encode) << 1,
                u64::from(steps.convert)
                    | u64::from(steps.encode) << 1
                    | u64::from(steps.stage) << 2,
                stage.channels as u64 | u64::from(stage.from_output) << 8,
                u64::from(queue) | u64::from(uv) << 4,
                left as u64,
                u64::from(download) | u64::from(output) << 1,
                next.next as u64,
            ],
        )
    })
}

fn c_frames(frames: &[Frame]) -> u64 {
    frames.iter().fold(0, |acc, f| {
        let f = black_box(*f);
        // SAFETY: plain value functions and a live input, laid out as the
        // C's.
        let (a, ring, targets, steps, stage, queue, uv, left, download, output, next) = unsafe {
            let a = oracle_obs_video_step_activation((&raw const f.input).cast::<c_void>());
            (
                a,
                oracle_obs_video_step_ring(f.cur),
                oracle_obs_video_step_render_targets(a.raw_active, a.gpu_active),
                oracle_obs_video_step_render(a.raw_active, a.gpu_active, f.conversion),
                oracle_obs_video_step_stage(f.conversion, true, f.channels),
                oracle_obs_video_step_queue(f.avail, f.queue, f.count, a.raw_active),
                oracle_obs_video_step_queue_copy_uv(f.share),
                oracle_obs_video_step_queue_count(f.count),
                oracle_obs_video_step_download(a.raw_active),
                oracle_obs_video_step_output(a.raw_active, f.ready),
                oracle_obs_video_step_ring(f.cur),
            )
        };
        fold(
            acc,
            &[
                u64::from(a.clear_base) | u64::from(a.clear_raw) << 1 | u64::from(a.clear_gpu) << 2,
                ring.prev as u64,
                u64::from(targets.outputs) | u64::from(targets.encode) << 1,
                u64::from(steps.convert)
                    | u64::from(steps.encode) << 1
                    | u64::from(steps.stage) << 2,
                stage.channels as u64 | u64::from(stage.from_output) << 8,
                u64::from(queue) | u64::from(uv) << 4,
                left as u64,
                u64::from(download) | u64::from(output) << 1,
                next.next as u64,
            ],
        )
    })
}

pub fn benchmarks(criterion: &mut Criterion) {
    let f = frames();
    pair(
        criterion,
        "video_output_step/frame/1024",
        || rust_frames(&f),
        || c_frames(&f),
    );
}
