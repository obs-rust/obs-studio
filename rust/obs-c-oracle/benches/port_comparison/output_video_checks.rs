//! obs-output: the output video size, conversion and capture checks
//! (`libobs/obs-output-video-checks.c.h`). `obs_output_can_begin_data_capture()`
//! and `obs_output_actual_start()` run the capture check once per start;
//! `obs_output_get_video_conversion()` runs the conversion plan once per raw
//! output start (and per call from a plugin); `obs_output_get_width2()` /
//! `_height2()` run a size route per call (the frontend, obs-websocket
//! `GetOutputList`, plugins); a raw output's audio start and stop take the
//! first mixer; the frontend's stats dock reads the congestion about once a
//! second per output. Each side runs 1024 calls over fixed inputs and folds
//! the results into a checksum both must match.

use super::pair;

use core::ffi::c_void;

use criterion::Criterion;
use obs_c_oracle::output_video_checks::{
    oracle_obs_output_begin_gate, oracle_obs_output_capture_ready,
    oracle_obs_output_congestion_clamp, oracle_obs_output_conversion_plan,
    oracle_obs_output_first_mixer, oracle_obs_output_size_route,
};
use obs_output::ffi::output_video_checks::{
    obs_output_begin_gate, obs_output_capture_presence, obs_output_capture_ready,
    obs_output_congestion_clamp, obs_output_conversion_plan, obs_output_conversion_sizes,
    obs_output_first_mixer, obs_output_size_route,
};
use std::hint::black_box;

const N: u32 = 1024;

/// Output `i`: a stream, a recording, a raw output or a broken one, with
/// encoders, video, audio and a service present or not.
fn presences() -> Vec<obs_output_capture_presence> {
    (0..N)
        .map(|i| obs_output_capture_presence {
            flags: i % 16,
            video_encoders: if i % 3 == 0 { 0 } else { 1 << (i % 10) },
            audio_encoders: if i % 5 == 0 { 0 } else { 1 << (i % 6) },
            video: i % 7 != 0,
            audio: i % 11 != 0,
            service: i % 13 != 0,
        })
        .collect()
}

/// Sizes `i`: 1080p and 720p canvases, scaled to 720p or not, a set
/// conversion with and without 0 axes, and no video.
fn conversion_sizes() -> Vec<obs_output_conversion_sizes> {
    (0..N)
        .map(|i| {
            let (vw, vh) = match i % 4 {
                0 => (1920, 1080),
                1 => (1280, 720),
                2 => (0, 0),
                _ => (2560, 1440),
            };
            let (cw, ch) = if i % 3 == 0 { (0, 0) } else { (1280, 720) };
            obs_output_conversion_sizes {
                set: i % 5 == 0,
                set_width: if i % 2 == 0 { 0 } else { 640 },
                set_height: if i % 7 == 0 { 0 } else { 360 },
                scaled_width: cw,
                scaled_height: ch,
                video_width: vw,
                video_height: vh,
            }
        })
        .collect()
}

fn rust_capture(p: &[obs_output_capture_presence]) -> u32 {
    p.iter().fold(0, |acc, p| {
        let p = black_box(*p);
        // SAFETY: a live presence.
        acc + u32::from(unsafe { obs_output_capture_ready(&raw const p) })
    })
}

fn c_capture(p: &[obs_output_capture_presence]) -> u32 {
    p.iter().fold(0, |acc, p| {
        let p = black_box(*p);
        // SAFETY: as above; the oracle reads it as the C struct.
        acc + u32::from(unsafe { oracle_obs_output_capture_ready((&raw const p).cast::<c_void>()) })
    })
}

fn rust_conversion(s: &[obs_output_conversion_sizes]) -> u64 {
    s.iter().fold(0, |acc, s| {
        let s = black_box(*s);
        // SAFETY: live sizes.
        let c = unsafe { obs_output_conversion_plan(&raw const s) };
        acc + u64::from(c.kind.unsigned_abs()) + u64::from(c.width) + u64::from(c.height)
    })
}

fn c_conversion(s: &[obs_output_conversion_sizes]) -> u64 {
    s.iter().fold(0, |acc, s| {
        let s = black_box(*s);
        // SAFETY: as above.
        let c = unsafe { oracle_obs_output_conversion_plan((&raw const s).cast::<c_void>()) };
        acc + u64::from(c.kind.unsigned_abs()) + u64::from(c.width) + u64::from(c.height)
    })
}

fn congestion(i: u32) -> f32 {
    // -0.5 to 1.5 in steps, plus a NaN every 97th.
    if i.is_multiple_of(97) {
        f32::NAN
    } else {
        (i % 64) as f32 / 32.0 - 0.5
    }
}

fn rust_small() -> u64 {
    (0..N).fold(0, |acc, i| {
        let i = black_box(i);
        let mixer = obs_output_first_mixer(i as usize * 0x41);
        let route = obs_output_size_route(i % 2 == 0, i % 3 == 0, i % 5);
        let gate = obs_output_begin_gate(i % 4 == 0, i % 3 == 0);
        let c = obs_output_congestion_clamp(congestion(i));
        acc + mixer as u64 + u64::from(route) + u64::from(gate) + u64::from(c.to_bits())
    })
}

fn c_small() -> u64 {
    (0..N).fold(0, |acc, i| {
        let i = black_box(i);
        // SAFETY: plain value functions.
        let (mixer, route, gate, c) = unsafe {
            (
                oracle_obs_output_first_mixer(i as usize * 0x41),
                oracle_obs_output_size_route(i % 2 == 0, i % 3 == 0, i % 5),
                oracle_obs_output_begin_gate(i % 4 == 0, i % 3 == 0),
                oracle_obs_output_congestion_clamp(congestion(i)),
            )
        };
        acc + mixer as u64 + u64::from(route) + u64::from(gate) + u64::from(c.to_bits())
    })
}

pub fn benchmarks(criterion: &mut Criterion) {
    let p = presences();
    let s = conversion_sizes();
    pair(
        criterion,
        "output_video_checks/capture_ready/1024",
        || rust_capture(&p),
        || c_capture(&p),
    );
    pair(
        criterion,
        "output_video_checks/conversion_plan/1024",
        || rust_conversion(&s),
        || c_conversion(&s),
    );
    pair(
        criterion,
        "output_video_checks/mixer_route_gate_congestion/1024",
        rust_small,
        c_small,
    );
}
