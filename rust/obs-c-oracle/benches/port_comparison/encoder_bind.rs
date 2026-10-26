//! obs-output: the encoder media binding and shutdown checks
//! (`libobs/obs-encoder-bind.c.h`). The setters run once per output start
//! or settings change and the unstarted shutdown once per output stop; the
//! type refusal also runs on every `obs_encoder_audio()` call (outputs and
//! muxers ask it per start, obs-websocket per request). Each side runs
//! 1024 rounds over encoders of both types with frame rates, sample rates,
//! divisors, trylock results and output states taken from a fixed table,
//! asking every decision the setters, the getters and the unstarted
//! shutdown ask, nothing is allocated, and the results fold into a
//! checksum both must match.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::encoder_audio_start::oracle_obs_encoder_audio_start_not_video;
use obs_c_oracle::encoder_bind::{
    oracle_obs_encoder_bind_audio_timebase, oracle_obs_encoder_bind_divisor_override,
    oracle_obs_encoder_bind_locked, oracle_obs_encoder_bind_not_audio,
    oracle_obs_encoder_bind_shutdown_unstarted, oracle_obs_encoder_bind_video_timebase,
};
use obs_output::ffi::encoder_audio_start::obs_encoder_audio_start_not_video;
use obs_output::ffi::encoder_bind::{
    obs_encoder_bind_audio_timebase, obs_encoder_bind_divisor_override, obs_encoder_bind_locked,
    obs_encoder_bind_not_audio, obs_encoder_bind_shutdown_unstarted,
    obs_encoder_bind_video_timebase,
};
use std::hint::black_box;

const N: u32 = 1024;
const FPS: [(u32, u32); 4] = [(30000, 1001), (60, 1), (24000, 1001), (50, 1)];
const RATES: [u32; 2] = [48000, 44100];

macro_rules! side {
    ($name:ident, $call:ident) => {
        fn $name() -> u64 {
            let mut acc = 0u64;
            for i in 0..N {
                let t = black_box(i32::from(i % 2 == 1));
                let (num, den) = FPS[usize::from(i % 4 == 1) + 2 * usize::from(i % 4 >= 2)];
                let rate = RATES[usize::from(i % 2 == 1)];
                if !$call!(not_video(t)) {
                    let tb = $call!(video_timebase(black_box(i % 3 != 0), num, den));
                    acc = acc
                        .wrapping_mul(31)
                        .wrapping_add((u64::from(tb.num) << 32) | u64::from(tb.den));
                    acc = acc.wrapping_add(u64::from($call!(divisor_override(black_box(i % 3)))));
                }
                if !$call!(not_audio(t)) {
                    let tb = $call!(audio_timebase(black_box(i % 5 != 0), rate));
                    acc = acc
                        .wrapping_mul(31)
                        .wrapping_add((u64::from(tb.num) << 32) | u64::from(tb.den));
                }
                let busy = black_box(if i % 7 == 0 { 16 } else { 0 });
                if $call!(locked(busy)) && $call!(locked(black_box(0))) {
                    let needed = black_box(i % 4 == 1);
                    let init = black_box(i % 2 == 0);
                    acc = acc.wrapping_add(u64::from($call!(shutdown_unstarted(init, needed))));
                }
            }
            acc
        }
    };
}

macro_rules! shim {
    (not_video($($a:expr),*)) => { obs_encoder_audio_start_not_video($($a),*) };
    (not_audio($($a:expr),*)) => { obs_encoder_bind_not_audio($($a),*) };
    (video_timebase($($a:expr),*)) => { obs_encoder_bind_video_timebase($($a),*) };
    (audio_timebase($($a:expr),*)) => { obs_encoder_bind_audio_timebase($($a),*) };
    (divisor_override($($a:expr),*)) => { obs_encoder_bind_divisor_override($($a),*) };
    (locked($($a:expr),*)) => { obs_encoder_bind_locked($($a),*) };
    (shutdown_unstarted($($a:expr),*)) => { obs_encoder_bind_shutdown_unstarted($($a),*) };
}

// SAFETY (each): plain values; nothing in the oracle divides.
macro_rules! oracle {
    (not_video($($a:expr),*)) => { unsafe { oracle_obs_encoder_audio_start_not_video($($a),*) } };
    (not_audio($($a:expr),*)) => { unsafe { oracle_obs_encoder_bind_not_audio($($a),*) } };
    (video_timebase($($a:expr),*)) => { unsafe { oracle_obs_encoder_bind_video_timebase($($a),*) } };
    (audio_timebase($($a:expr),*)) => { unsafe { oracle_obs_encoder_bind_audio_timebase($($a),*) } };
    (divisor_override($($a:expr),*)) => { unsafe { oracle_obs_encoder_bind_divisor_override($($a),*) } };
    (locked($($a:expr),*)) => { unsafe { oracle_obs_encoder_bind_locked($($a),*) } };
    (shutdown_unstarted($($a:expr),*)) => { unsafe { oracle_obs_encoder_bind_shutdown_unstarted($($a),*) } };
}

side!(rust, shim);
side!(c, oracle);

pub fn benchmarks(criterion: &mut Criterion) {
    pair(criterion, "encoder_bind/setters_and_shutdown/1024", rust, c);
}
