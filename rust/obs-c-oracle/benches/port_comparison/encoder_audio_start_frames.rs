//! obs-output: the two sample-rate conversions of the encoder's audio start
//! (`libobs/obs-encoder-audio-start-frames.c.h`). They run for each audio
//! packet of an encoder paired with a video encoder until its audio starts,
//! so each must cost what the C does. Each side runs 1024 packets over
//! fixed inputs (packets before and across a video start at 48 kHz, some
//! waiting, some truncated), asks the length, the packet plan after it and,
//! for a truncated packet, the offset in frames and its size, nothing is
//! allocated, and the results fold into a checksum both must match.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::encoder_audio_start::{
    oracle_obs_encoder_audio_start_offset_size, oracle_obs_encoder_audio_start_packet,
};
use obs_c_oracle::encoder_audio_start_frames::{
    oracle_obs_encoder_audio_start_frames_duration, oracle_obs_encoder_audio_start_frames_offset,
};
use obs_output::ffi::encoder_audio_start::{
    obs_encoder_audio_start_offset_size, obs_encoder_audio_start_packet,
};
use obs_output::ffi::encoder_audio_start_frames::{
    obs_encoder_audio_start_frames_duration, obs_encoder_audio_start_frames_offset,
};
use std::hint::black_box;

const N: u64 = 1024;
/// 1024 frames at 48 kHz.
const PKT: u64 = 21_333_333;
const V: u64 = 10_000_000_000;

/// Packet `i`: its timestamp, all before the video start.
fn timestamp(i: u64) -> u64 {
    V - 2 * PKT + i * (2 * PKT / N)
}

fn rust() -> u64 {
    let mut acc = 0u64;
    for i in 0..N {
        let ts = black_box(timestamp(i));
        let rate = black_box(48000u32);
        let duration = obs_encoder_audio_start_frames_duration(1024, rate);
        let plan = obs_encoder_audio_start_packet(ts, duration, black_box(V));
        if plan.wait {
            acc = acc.wrapping_add(1);
            continue;
        }
        let frames = obs_encoder_audio_start_frames_offset(V - ts, rate);
        let size = obs_encoder_audio_start_offset_size(frames, black_box(8));
        acc = acc.wrapping_mul(31).wrapping_add(size as u64);
    }
    acc
}

fn c() -> u64 {
    let mut acc = 0u64;
    for i in 0..N {
        let ts = black_box(timestamp(i));
        let rate = black_box(48000u32);
        // SAFETY: plain values; the rate is not 0.
        let duration = unsafe { oracle_obs_encoder_audio_start_frames_duration(1024, rate) };
        // SAFETY: plain values.
        let plan = unsafe { oracle_obs_encoder_audio_start_packet(ts, duration, black_box(V)) };
        if plan.wait {
            acc = acc.wrapping_add(1);
            continue;
        }
        // SAFETY: plain values; the offset is shorter than the packet, so
        // the quotient fits.
        let frames = unsafe { oracle_obs_encoder_audio_start_frames_offset(V - ts, rate) };
        // SAFETY: plain values.
        let size = unsafe { oracle_obs_encoder_audio_start_offset_size(frames, black_box(8)) };
        acc = acc.wrapping_mul(31).wrapping_add(size as u64);
    }
    acc
}

pub fn benchmarks(criterion: &mut Criterion) {
    pair(criterion, "encoder_audio_start_frames/buffer_audio/1024", rust, c);
}
