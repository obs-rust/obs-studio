//! obs-output: the output begin-capture plan
//! (`libobs/obs-output-begin.c.h`). Every start of an encoded output asks
//! it once: each encoder's packet interval, the interleaver's maximum batch
//! size and the pairing gates. Each side plans 256 starts of a multitrack
//! output (three video encoders at 60 fps with divisors 1, 2 and 2, six AAC
//! tracks at 48 kHz) and folds the answers into a checksum both must match.
//! Nothing is allocated in the loop.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::output_begin::{
    oracle_obs_output_begin_audio_interval, oracle_obs_output_begin_batch_size,
    oracle_obs_output_begin_pair_audio, oracle_obs_output_begin_pair_video,
    oracle_obs_output_begin_video_interval,
};
use obs_output::ffi::output_begin::{
    obs_output_begin_audio_interval, obs_output_begin_batch_size, obs_output_begin_pair_audio,
    obs_output_begin_pair_video, obs_output_begin_video_interval,
};
use std::hint::black_box;

const STARTS: u64 = 256;
const DIVISORS: [u32; 3] = [1, 2, 2];
const TRACKS: usize = 6;

fn fold(acc: u64, values: &[u64]) -> u64 {
    values
        .iter()
        .fold(acc, |acc, v| acc.wrapping_mul(31).wrapping_add(*v))
}

fn rust_starts() -> u64 {
    let mut intervals = [0u64; DIVISORS.len() + TRACKS];
    (0..STARTS).fold(0, |acc, s| {
        let s = black_box(s);
        for (slot, &d) in intervals.iter_mut().zip(DIVISORS.iter()) {
            *slot = obs_output_begin_video_interval(60, 1, d);
        }
        for slot in intervals.iter_mut().skip(DIVISORS.len()) {
            *slot = obs_output_begin_audio_interval(48000, 1024);
        }
        // SAFETY: an initialized local array, read only.
        let batch = unsafe { obs_output_begin_batch_size(intervals.as_ptr(), intervals.len()) };
        let video = obs_output_begin_pair_video(s % 2 == 0);
        let paired = (0..TRACKS).filter(|&t| obs_output_begin_pair_audio(false, t % 2)).count();
        fold(
            acc,
            &[
                u64::try_from(batch).unwrap_or(u64::MAX),
                u64::from(video),
                u64::try_from(paired).unwrap_or(u64::MAX),
            ],
        )
    })
}

fn c_starts() -> u64 {
    let mut intervals = [0u64; DIVISORS.len() + TRACKS];
    (0..STARTS).fold(0, |acc, s| {
        let s = black_box(s);
        // SAFETY: plain values (C is defined for them) and an initialized
        // local array, read only.
        let (batch, video, paired) = unsafe {
            for (slot, &d) in intervals.iter_mut().zip(DIVISORS.iter()) {
                *slot = oracle_obs_output_begin_video_interval(60, 1, d);
            }
            for slot in intervals.iter_mut().skip(DIVISORS.len()) {
                *slot = oracle_obs_output_begin_audio_interval(48000, 1024);
            }
            let batch = oracle_obs_output_begin_batch_size(intervals.as_ptr(), intervals.len());
            let video = oracle_obs_output_begin_pair_video(s % 2 == 0);
            let paired = (0..TRACKS)
                .filter(|&t| oracle_obs_output_begin_pair_audio(false, t % 2))
                .count();
            (batch, video, paired)
        };
        fold(
            acc,
            &[
                u64::try_from(batch).unwrap_or(u64::MAX),
                u64::from(video),
                u64::try_from(paired).unwrap_or(u64::MAX),
            ],
        )
    })
}

pub fn benchmarks(criterion: &mut Criterion) {
    pair(criterion, "output_begin/start/256", rust_starts, c_starts);
}
