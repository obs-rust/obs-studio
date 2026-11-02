//! obs-output: the two sample-rate conversions of the raw output audio trim
//! (`libobs/obs-output-raw-audio-trim.c.h`). They run for each audio packet
//! of a raw output that starts before its first video frame, so each must
//! cost what the C does. Each side runs 1024 packets over fixed inputs
//! (packets before and across a video start at 48 kHz, some dropped, some
//! cut), asks the length, the drop test after it and, for a kept packet,
//! the cut in frames, nothing is allocated, and the results fold into a
//! checksum both must match.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::output_raw_audio::oracle_obs_output_raw_audio_ends_before;
use obs_c_oracle::output_raw_audio_trim::{
    oracle_obs_output_raw_audio_trim_duration, oracle_obs_output_raw_audio_trim_frames,
};
use obs_output::ffi::output_raw_audio::obs_output_raw_audio_ends_before;
use obs_output::ffi::output_raw_audio_trim::{
    obs_output_raw_audio_trim_duration, obs_output_raw_audio_trim_frames,
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
        let rate = black_box(48000usize);
        let duration = obs_output_raw_audio_trim_duration(1024, rate);
        if obs_output_raw_audio_ends_before(ts, duration, black_box(V)) {
            acc = acc.wrapping_add(1);
            continue;
        }
        acc = acc
            .wrapping_mul(31)
            .wrapping_add(obs_output_raw_audio_trim_frames(V - ts, rate));
    }
    acc
}

fn c() -> u64 {
    let mut acc = 0u64;
    for i in 0..N {
        let ts = black_box(timestamp(i));
        let rate = black_box(48000usize);
        // SAFETY: plain values; the rate is not 0 and the cut is shorter
        // than the packet, so neither conversion traps.
        let duration = unsafe { oracle_obs_output_raw_audio_trim_duration(1024, rate) };
        // SAFETY: plain values.
        if unsafe { oracle_obs_output_raw_audio_ends_before(ts, duration, black_box(V)) } {
            acc = acc.wrapping_add(1);
            continue;
        }
        // SAFETY: as above.
        let cut = unsafe { oracle_obs_output_raw_audio_trim_frames(V - ts, rate) };
        acc = acc.wrapping_mul(31).wrapping_add(cut);
    }
    acc
}

pub fn benchmarks(criterion: &mut Criterion) {
    pair(criterion, "output_raw_audio_trim/prepare_audio/1024", rust, c);
}
