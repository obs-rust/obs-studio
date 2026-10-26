//! obs-output: the raw output audio admission and block loop decisions
//! (`libobs/obs-output-raw-audio.c.h`). They run for each audio packet of
//! every raw output (about 47 a second per mix at 48 kHz), so each must cost
//! what the C does. Each side runs 1024 packets over fixed inputs (packets
//! before, across and after a video start, some cut, some dropped), asking
//! every decision `prepare_audio()` and `default_raw_audio_callback()` ask,
//! nothing is allocated, and the results fold into a checksum both must
//! match. The conversions that divide by the sample rate stay in C, so the
//! packet's duration and the cut in frames are fixed inputs here.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::output_raw_audio::{
    oracle_obs_output_raw_audio_before_video, oracle_obs_output_raw_audio_block_bytes,
    oracle_obs_output_raw_audio_block_ts, oracle_obs_output_raw_audio_can_pop,
    oracle_obs_output_raw_audio_cut_bytes, oracle_obs_output_raw_audio_cut_frames,
    oracle_obs_output_raw_audio_cut_ns, oracle_obs_output_raw_audio_cut_ts,
    oracle_obs_output_raw_audio_ends_before, oracle_obs_output_raw_audio_next_frames,
    oracle_obs_output_raw_audio_passthrough, oracle_obs_output_raw_audio_paused_ts,
    oracle_obs_output_raw_audio_push_bytes, oracle_obs_output_raw_audio_start_unset,
    oracle_obs_output_raw_audio_video_unset,
};
use obs_output::ffi::output_raw_audio::{
    obs_output_raw_audio_before_video, obs_output_raw_audio_block_bytes,
    obs_output_raw_audio_block_ts, obs_output_raw_audio_can_pop, obs_output_raw_audio_cut_bytes,
    obs_output_raw_audio_cut_frames, obs_output_raw_audio_cut_ns, obs_output_raw_audio_cut_ts,
    obs_output_raw_audio_ends_before, obs_output_raw_audio_next_frames,
    obs_output_raw_audio_passthrough, obs_output_raw_audio_paused_ts,
    obs_output_raw_audio_push_bytes, obs_output_raw_audio_start_unset,
    obs_output_raw_audio_video_unset,
};
use std::hint::black_box;

const N: u64 = 1024;
/// 1024 frames at 48 kHz.
const PKT: u64 = 21_333_333;
const V: u64 = 10_000_000_000;

/// Packet `i`: its timestamp, frames, duration and the cut in frames the C
/// would compute for it.
fn packet(i: u64) -> (u64, u32, u64, u64) {
    let ts = V - 4 * PKT + i * (PKT / 8);
    let cut_frames = (i % 64) * 16;
    (ts, 1024, PKT, cut_frames)
}

macro_rules! side {
    ($name:ident, $call:ident) => {
        fn $name() -> u64 {
            let mut audio_start_ts = 0u64;
            let mut total = 0usize;
            let mut buffered = 0usize;
            let mut acc = 0u64;
            for i in 0..N {
                let (ts, frames, duration, cut) = black_box(packet(i));
                let flags = black_box(3u32);
                let video_start_ts = black_box(V);
                let size = black_box(4usize);
                let mut out_ts = ts;
                let mut out_frames = frames;
                if !$call!(passthrough(flags)) {
                    if $call!(video_unset(video_start_ts)) {
                        continue;
                    }
                    if $call!(before_video(ts, video_start_ts)) {
                        if $call!(ends_before(ts, duration, video_start_ts)) {
                            acc = acc.wrapping_add(1);
                            continue;
                        }
                        let c = $call!(cut_ns(video_start_ts, ts));
                        out_ts = $call!(cut_ts(out_ts, c));
                        acc = acc.wrapping_add($call!(cut_bytes(size, cut)) as u64);
                        out_frames = $call!(cut_frames(out_frames, cut));
                    }
                }
                if $call!(start_unset(audio_start_ts)) {
                    audio_start_ts = out_ts;
                }
                let block = $call!(block_bytes(size));
                buffered = buffered.wrapping_add($call!(push_bytes(out_frames, size)));
                while $call!(can_pop(buffered, block)) {
                    buffered -= block;
                    let t = $call!(block_ts(audio_start_ts, total as u64 * 20_833));
                    acc = acc.wrapping_mul(31).wrapping_add($call!(paused_ts(t, 7)));
                    total = $call!(next_frames(total));
                }
            }
            acc.wrapping_add(total as u64)
        }
    };
}

macro_rules! shim {
    ($f:ident($($a:expr),*)) => {
        paste_shim!($f, $($a),*)
    };
}
macro_rules! oracle {
    ($f:ident($($a:expr),*)) => {
        // SAFETY: plain values; nothing in the oracle divides.
        unsafe { paste_oracle!($f, $($a),*) }
    };
}
macro_rules! paste_shim {
    (passthrough, $($a:expr),*) => { obs_output_raw_audio_passthrough($($a),*) };
    (video_unset, $($a:expr),*) => { obs_output_raw_audio_video_unset($($a),*) };
    (before_video, $($a:expr),*) => { obs_output_raw_audio_before_video($($a),*) };
    (ends_before, $($a:expr),*) => { obs_output_raw_audio_ends_before($($a),*) };
    (cut_ns, $($a:expr),*) => { obs_output_raw_audio_cut_ns($($a),*) };
    (cut_ts, $($a:expr),*) => { obs_output_raw_audio_cut_ts($($a),*) };
    (cut_bytes, $($a:expr),*) => { obs_output_raw_audio_cut_bytes($($a),*) };
    (cut_frames, $($a:expr),*) => { obs_output_raw_audio_cut_frames($($a),*) };
    (start_unset, $($a:expr),*) => { obs_output_raw_audio_start_unset($($a),*) };
    (block_bytes, $($a:expr),*) => { obs_output_raw_audio_block_bytes($($a),*) };
    (push_bytes, $($a:expr),*) => { obs_output_raw_audio_push_bytes($($a),*) };
    (can_pop, $($a:expr),*) => { obs_output_raw_audio_can_pop($($a),*) };
    (block_ts, $($a:expr),*) => { obs_output_raw_audio_block_ts($($a),*) };
    (paused_ts, $($a:expr),*) => { obs_output_raw_audio_paused_ts($($a),*) };
    (next_frames, $($a:expr),*) => { obs_output_raw_audio_next_frames($($a),*) };
}
macro_rules! paste_oracle {
    (passthrough, $($a:expr),*) => { oracle_obs_output_raw_audio_passthrough($($a),*) };
    (video_unset, $($a:expr),*) => { oracle_obs_output_raw_audio_video_unset($($a),*) };
    (before_video, $($a:expr),*) => { oracle_obs_output_raw_audio_before_video($($a),*) };
    (ends_before, $($a:expr),*) => { oracle_obs_output_raw_audio_ends_before($($a),*) };
    (cut_ns, $($a:expr),*) => { oracle_obs_output_raw_audio_cut_ns($($a),*) };
    (cut_ts, $($a:expr),*) => { oracle_obs_output_raw_audio_cut_ts($($a),*) };
    (cut_bytes, $($a:expr),*) => { oracle_obs_output_raw_audio_cut_bytes($($a),*) };
    (cut_frames, $($a:expr),*) => { oracle_obs_output_raw_audio_cut_frames($($a),*) };
    (start_unset, $($a:expr),*) => { oracle_obs_output_raw_audio_start_unset($($a),*) };
    (block_bytes, $($a:expr),*) => { oracle_obs_output_raw_audio_block_bytes($($a),*) };
    (push_bytes, $($a:expr),*) => { oracle_obs_output_raw_audio_push_bytes($($a),*) };
    (can_pop, $($a:expr),*) => { oracle_obs_output_raw_audio_can_pop($($a),*) };
    (block_ts, $($a:expr),*) => { oracle_obs_output_raw_audio_block_ts($($a),*) };
    (paused_ts, $($a:expr),*) => { oracle_obs_output_raw_audio_paused_ts($($a),*) };
    (next_frames, $($a:expr),*) => { oracle_obs_output_raw_audio_next_frames($($a),*) };
}

side!(rust, shim);
side!(c, oracle);

pub fn benchmarks(criterion: &mut Criterion) {
    pair(
        criterion,
        "output_raw_audio/raw_audio_callback/1024",
        rust,
        c,
    );
}
