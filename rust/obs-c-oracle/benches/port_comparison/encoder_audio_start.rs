//! obs-output: the encoder audio start alignment and the setter gates
//! (`libobs/obs-encoder-audio-start.c.h`). `buffer_audio()` runs the size,
//! the branch checks and the push size for every audio packet of every
//! audio encoder (one per 1024 frames, about 47 a second at 48 kHz), and the
//! start plan with the offsets until its paired video starts; the send loop
//! and the audio pts step run per encoded frame; `receive_video()` runs the
//! group gate, the paired walk and the video pts step per frame of every
//! video encoder. The setter gates run once per settings change and are
//! left out. Each side runs 1024 calls over fixed inputs, nothing is
//! allocated, and the results fold into a checksum both must match.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::encoder_audio_start::{
    oracle_obs_encoder_audio_start_audio_pts, oracle_obs_encoder_audio_start_buffer_offset,
    oracle_obs_encoder_audio_start_can_send, oracle_obs_encoder_audio_start_group_ready,
    oracle_obs_encoder_audio_start_no_video, oracle_obs_encoder_audio_start_offset_ns,
    oracle_obs_encoder_audio_start_offset_size, oracle_obs_encoder_audio_start_packet,
    oracle_obs_encoder_audio_start_paired, oracle_obs_encoder_audio_start_paired_late,
    oracle_obs_encoder_audio_start_push_size, oracle_obs_encoder_audio_start_size,
    oracle_obs_encoder_audio_start_unpaired, oracle_obs_encoder_audio_start_video_pts,
};
use obs_output::ffi::encoder_audio_start::{
    obs_encoder_audio_start_audio_pts, obs_encoder_audio_start_buffer_offset,
    obs_encoder_audio_start_can_send, obs_encoder_audio_start_group_ready,
    obs_encoder_audio_start_no_video, obs_encoder_audio_start_offset_ns,
    obs_encoder_audio_start_offset_size, obs_encoder_audio_start_packet,
    obs_encoder_audio_start_paired, obs_encoder_audio_start_paired_late,
    obs_encoder_audio_start_push_size, obs_encoder_audio_start_size,
    obs_encoder_audio_start_unpaired, obs_encoder_audio_start_video_pts,
};
use std::hint::black_box;

const N: u64 = 1024;
/// 1024 frames at 48 kHz.
const PKT: u64 = 21_333_333;
const V: u64 = 10_000_000_000;

/// Packet `i`: its timestamp around the video's start, the start
/// timestamp (0 for the first packets), whether it is paired, and the
/// video's start (0 before the video).
fn packet(i: u64) -> (u64, u64, bool, u64) {
    let ts = V - 8 * PKT + i * PKT / 4;
    let start = if i < 512 { 0 } else { V };
    let paired = !i.is_multiple_of(7);
    let v = if i.is_multiple_of(5) { 0 } else { V };
    (ts, start, paired, v)
}

fn rust_audio() -> u64 {
    (0..N).fold(0, |acc, i| {
        let (ts, start, paired, v) = black_box(packet(i));
        let size = obs_encoder_audio_start_size(1024, 4);
        let mut offset = 0;
        if obs_encoder_audio_start_paired(start, paired) && !obs_encoder_audio_start_no_video(v) {
            let p = obs_encoder_audio_start_packet(ts, PKT, v);
            if p.truncate {
                let ns = obs_encoder_audio_start_offset_ns(v, ts);
                offset = obs_encoder_audio_start_offset_size(ns / 20_834, 4);
            }
            if p.from_buffer && obs_encoder_audio_start_buffer_offset(ts - PKT, v) {
                offset += 1;
            }
        } else if obs_encoder_audio_start_unpaired(start, paired) {
            offset += 2;
        }
        let pushed = obs_encoder_audio_start_push_size(size, offset);
        let send = obs_encoder_audio_start_can_send(pushed + 4096, 4096);
        let pts = obs_encoder_audio_start_audio_pts(i as i64 * 1024, 1024);
        acc + pushed as u64 + u64::from(send) + pts as u64
    })
}

fn c_audio() -> u64 {
    (0..N).fold(0, |acc, i| {
        let (ts, start, paired, v) = black_box(packet(i));
        // SAFETY: plain value functions.
        unsafe {
            let size = oracle_obs_encoder_audio_start_size(1024, 4);
            let mut offset = 0;
            if oracle_obs_encoder_audio_start_paired(start, paired)
                && !oracle_obs_encoder_audio_start_no_video(v)
            {
                let p = oracle_obs_encoder_audio_start_packet(ts, PKT, v);
                if p.truncate {
                    let ns = oracle_obs_encoder_audio_start_offset_ns(v, ts);
                    offset = oracle_obs_encoder_audio_start_offset_size(ns / 20_834, 4);
                }
                if p.from_buffer && oracle_obs_encoder_audio_start_buffer_offset(ts - PKT, v) {
                    offset += 1;
                }
            } else if oracle_obs_encoder_audio_start_unpaired(start, paired) {
                offset += 2;
            }
            let pushed = oracle_obs_encoder_audio_start_push_size(size, offset);
            let send = oracle_obs_encoder_audio_start_can_send(pushed + 4096, 4096);
            let pts = oracle_obs_encoder_audio_start_audio_pts(i as i64 * 1024, 1024);
            acc + pushed as u64 + u64::from(send) + pts as u64
        }
    })
}

/// Frame `i` at 60 fps: a group start equal to it or not, and the first
/// timestamps of three paired audio encoders around it.
fn frame(i: u64) -> (u64, u64, [u64; 3]) {
    let ts = V + i * 16_666_667;
    let group = if i.is_multiple_of(3) { ts } else { V };
    (ts, group, [ts - 1, ts, ts + i % 2])
}

fn rust_video() -> u64 {
    (0..N).fold(0, |acc, i| {
        let (ts, group, firsts) = black_box(frame(i));
        let ready = obs_encoder_audio_start_group_ready(group, ts);
        let late = firsts
            .iter()
            .take_while(|&&f| !obs_encoder_audio_start_paired_late(f, ts))
            .count();
        let pts = obs_encoder_audio_start_video_pts(i as i64, 1, 1 + (i % 2) as u32);
        acc + u64::from(ready) + late as u64 + pts as u64
    })
}

fn c_video() -> u64 {
    (0..N).fold(0, |acc, i| {
        let (ts, group, firsts) = black_box(frame(i));
        // SAFETY: plain value functions; the pts sum stays small.
        unsafe {
            let ready = oracle_obs_encoder_audio_start_group_ready(group, ts);
            let late = firsts
                .iter()
                .take_while(|&&f| !oracle_obs_encoder_audio_start_paired_late(f, ts))
                .count();
            let pts = oracle_obs_encoder_audio_start_video_pts(i as i64, 1, 1 + (i % 2) as u32);
            acc + u64::from(ready) + late as u64 + pts as u64
        }
    })
}

pub fn benchmarks(criterion: &mut Criterion) {
    pair(
        criterion,
        "encoder_audio_start/buffer_audio/1024",
        rust_audio,
        c_audio,
    );
    pair(
        criterion,
        "encoder_audio_start/receive_video/1024",
        rust_video,
        c_video,
    );
}
