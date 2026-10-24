//! obs-output: the encoder size and info choices
//! (`libobs/obs-encoder-size.c.h`). `obs_encoder_get_width()` /
//! `_height()` run a size route per call (the output's size getters, the
//! frontend, obs-websocket `GetOutputList`, encoder plugins at create and
//! update); `obs_encoder_get_sample_rate()` a rate route per call (audio
//! encoders and muxers); `get_video_info()` and `get_audio_info()` run the
//! cap override and the audio defaults once per encoder start;
//! `obs_encoder_set_scaled_size()` runs the gate and the match once per
//! setting change; `obs_encoder_scaling_enabled()` runs per call. Each side
//! runs 1024 calls over fixed inputs and folds the results into a checksum
//! both must match.

use super::pair;

use core::ffi::c_void;

use criterion::Criterion;
use obs_c_oracle::encoder_size::{
    oracle_obs_encoder_size_audio_defaults, oracle_obs_encoder_size_cap_override,
    oracle_obs_encoder_size_rate_route, oracle_obs_encoder_size_scaled_gate,
    oracle_obs_encoder_size_scaled_match, oracle_obs_encoder_size_scaling,
    oracle_obs_encoder_size_video_route,
};
use obs_output::ffi::encoder_size::{
    obs_encoder_audio_fill, obs_encoder_size_audio_defaults, obs_encoder_size_cap_override,
    obs_encoder_size_rate_route, obs_encoder_size_scaled_gate, obs_encoder_size_scaled_match,
    obs_encoder_size_scaling, obs_encoder_size_video_route,
};
use std::hint::black_box;

const N: u32 = 1024;

/// Encoder `i`'s infos before the default fill: zeroed (what libobs
/// passes) or partly set, against a 48 kHz stereo float mix or a 44.1 kHz
/// one.
fn fills() -> Vec<(obs_encoder_audio_fill, obs_encoder_audio_fill)> {
    (0..N)
        .map(|i| {
            let info = obs_encoder_audio_fill {
                format: if i.is_multiple_of(3) { 2 } else { 0 },
                samples_per_sec: if i.is_multiple_of(5) { 44100 } else { 0 },
                speakers: if i.is_multiple_of(7) { 1 } else { 0 },
            };
            let mix = obs_encoder_audio_fill {
                format: 8,
                samples_per_sec: if i.is_multiple_of(2) { 48000 } else { 44100 },
                speakers: 2,
            };
            (info, mix)
        })
        .collect()
}

fn rust_defaults(f: &[(obs_encoder_audio_fill, obs_encoder_audio_fill)]) -> u64 {
    f.iter().fold(0, |acc, (info, mix)| {
        let (info, mix) = black_box((*info, *mix));
        // SAFETY: live locals.
        let o = unsafe { obs_encoder_size_audio_defaults(&raw const info, &raw const mix) };
        acc + u64::from(o.format.unsigned_abs())
            + u64::from(o.samples_per_sec)
            + u64::from(o.speakers.unsigned_abs())
    })
}

fn c_defaults(f: &[(obs_encoder_audio_fill, obs_encoder_audio_fill)]) -> u64 {
    f.iter().fold(0, |acc, (info, mix)| {
        let (info, mix) = black_box((*info, *mix));
        // SAFETY: as above; the oracle reads them as the C struct.
        let o = unsafe {
            oracle_obs_encoder_size_audio_defaults(
                (&raw const info).cast::<c_void>(),
                (&raw const mix).cast::<c_void>(),
            )
        };
        acc + u64::from(o.format.unsigned_abs())
            + u64::from(o.samples_per_sec)
            + u64::from(o.speakers.unsigned_abs())
    })
}

/// Size `i`: a 1080p, 720p or 1440p video, scaled to 720p, to the video's
/// own size or not at all, an encoder with or without the scaling cap.
fn sizes(i: u32) -> (u32, u32, u32, u32, u32) {
    let (vw, vh) = match i % 3 {
        0 => (1920, 1080),
        1 => (1280, 720),
        _ => (2560, 1440),
    };
    let (w, h) = match i % 4 {
        0 => (0, 0),
        1 => (vw, vh),
        _ => (1280, 720),
    };
    let caps = if i.is_multiple_of(5) { 1 << 5 } else { 1 << 3 };
    (caps, w, h, vw, vh)
}

fn rust_video() -> u64 {
    (0..N).fold(0, |acc, i| {
        let (caps, w, h, vw, vh) = black_box(sizes(i));
        let t = black_box(1);
        let o = obs_encoder_size_cap_override(caps, w, h, vw, vh);
        let gate = obs_encoder_size_scaled_gate(t, i.is_multiple_of(6), i.is_multiple_of(7));
        let m = obs_encoder_size_scaled_match(i % 8 != 0, vw, vh, w, h);
        let s = obs_encoder_size_scaling(w, h);
        let route = obs_encoder_size_video_route(t, i % 9 != 0, w);
        acc + u64::from(o.width)
            + u64::from(o.height)
            + u64::from(gate)
            + u64::from(m)
            + u64::from(s)
            + u64::from(route)
    })
}

fn c_video() -> u64 {
    (0..N).fold(0, |acc, i| {
        let (caps, w, h, vw, vh) = black_box(sizes(i));
        let t = black_box(1);
        // SAFETY: plain value functions.
        let (o, gate, m, s, route) = unsafe {
            (
                oracle_obs_encoder_size_cap_override(caps, w, h, vw, vh),
                oracle_obs_encoder_size_scaled_gate(t, i.is_multiple_of(6), i.is_multiple_of(7)),
                oracle_obs_encoder_size_scaled_match(i % 8 != 0, vw, vh, w, h),
                oracle_obs_encoder_size_scaling(w, h),
                oracle_obs_encoder_size_video_route(t, i % 9 != 0, w),
            )
        };
        acc + u64::from(o.width)
            + u64::from(o.height)
            + u64::from(gate)
            + u64::from(m)
            + u64::from(s)
            + u64::from(route)
    })
}

fn rust_rate() -> u64 {
    (0..N).fold(0, |acc, i| {
        let i = black_box(i);
        let rate = if i.is_multiple_of(2) { 0 } else { 48000 };
        acc + u64::from(obs_encoder_size_rate_route(
            (i % 3) as i32,
            i % 5 != 0,
            rate,
        ))
    })
}

fn c_rate() -> u64 {
    (0..N).fold(0, |acc, i| {
        let i = black_box(i);
        let rate = if i.is_multiple_of(2) { 0 } else { 48000 };
        // SAFETY: a plain value function.
        acc + u64::from(unsafe {
            oracle_obs_encoder_size_rate_route((i % 3) as i32, i % 5 != 0, rate)
        })
    })
}

pub fn benchmarks(criterion: &mut Criterion) {
    let f = fills();
    pair(
        criterion,
        "encoder_size/audio_defaults/1024",
        || rust_defaults(&f),
        || c_defaults(&f),
    );
    pair(
        criterion,
        "encoder_size/video_override_gate_match_route/1024",
        rust_video,
        c_video,
    );
    pair(criterion, "encoder_size/rate_route/1024", rust_rate, c_rate);
}
