//! obs-output: the GPU encode thread gates (`libobs/obs-encoder-gpu-gates.c.h`).
//! They run for each frame and each texture encoder of every NVENC, QSV,
//! AMF or VA-API texture recording and stream (60 or more frames a second),
//! so each must cost what the C does. Each side runs 1024 frames through
//! four encoders with divisors 1 to 4, asking every decision
//! `gpu_encode_thread()` asks (the divisor counter, the key, the timing
//! entry, the pts step, the count and the requeue), nothing is allocated,
//! and the results fold into a checksum both must match.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::encoder_audio_start::oracle_obs_encoder_audio_start_video_pts;
use obs_c_oracle::encoder_gpu_gates::{
    oracle_obs_encoder_gpu_gates_count_next, oracle_obs_encoder_gpu_gates_divisor_inc,
    oracle_obs_encoder_gpu_gates_divisor_skip, oracle_obs_encoder_gpu_gates_divisor_wrap,
    oracle_obs_encoder_gpu_gates_next_key, oracle_obs_encoder_gpu_gates_record_timing,
    oracle_obs_encoder_gpu_gates_requeue, oracle_obs_encoder_gpu_gates_requeue_ts,
};
use obs_output::ffi::encoder_audio_start::obs_encoder_audio_start_video_pts;
use obs_output::ffi::encoder_gpu_gates::{
    obs_encoder_gpu_gates_count_next, obs_encoder_gpu_gates_divisor_inc,
    obs_encoder_gpu_gates_divisor_skip, obs_encoder_gpu_gates_divisor_wrap,
    obs_encoder_gpu_gates_next_key, obs_encoder_gpu_gates_record_timing,
    obs_encoder_gpu_gates_requeue, obs_encoder_gpu_gates_requeue_ts,
};
use std::hint::black_box;

const N: u64 = 1024;
const ENCODERS: usize = 4;
const FRAME_NS: u64 = 16_666_667;

macro_rules! side {
    ($name:ident, $call:ident) => {
        fn $name() -> u64 {
            let mut counters = [0u32; ENCODERS];
            let mut pts = [0i64; ENCODERS];
            let mut key = 0u64;
            let mut acc = 0u64;
            for f in 0..N {
                let mut timestamp = black_box(f * FRAME_NS + 1);
                let mut count = black_box(1 + (f % 3) as i32);
                loop {
                    let mut lock_count = 0usize;
                    let mut next = key;
                    for e in 0..ENCODERS {
                        let divisor = black_box(e as u32 + 1);
                        let skip = counters[e];
                        counters[e] = $call!(divisor_inc(skip));
                        if $call!(divisor_wrap(counters[e], divisor)) {
                            counters[e] = 0;
                        }
                        if $call!(divisor_skip(skip)) {
                            continue;
                        }
                        lock_count += 1;
                        next = $call!(next_key(lock_count, ENCODERS, next));
                        if $call!(record_timing(timestamp)) {
                            acc = acc.wrapping_add(1);
                        }
                        pts[e] = $call!(video_pts(pts[e], 1001, divisor));
                        acc = acc.wrapping_mul(31).wrapping_add(next);
                    }
                    key = next;
                    count = $call!(count_next(count));
                    if !$call!(requeue(count)) {
                        break;
                    }
                    timestamp = $call!(requeue_ts(timestamp, FRAME_NS));
                }
            }
            pts.iter().fold(acc, |a, &p| a.wrapping_add(p as u64))
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
        // SAFETY: plain values; nothing in the oracle divides, and the
        // counts stay above INT_MIN.
        unsafe { paste_oracle!($f, $($a),*) }
    };
}
macro_rules! paste_shim {
    (divisor_inc, $($a:expr),*) => { obs_encoder_gpu_gates_divisor_inc($($a),*) };
    (divisor_wrap, $($a:expr),*) => { obs_encoder_gpu_gates_divisor_wrap($($a),*) };
    (divisor_skip, $($a:expr),*) => { obs_encoder_gpu_gates_divisor_skip($($a),*) };
    (next_key, $($a:expr),*) => { obs_encoder_gpu_gates_next_key($($a),*) };
    (record_timing, $($a:expr),*) => { obs_encoder_gpu_gates_record_timing($($a),*) };
    (video_pts, $($a:expr),*) => { obs_encoder_audio_start_video_pts($($a),*) };
    (count_next, $($a:expr),*) => { obs_encoder_gpu_gates_count_next($($a),*) };
    (requeue, $($a:expr),*) => { obs_encoder_gpu_gates_requeue($($a),*) };
    (requeue_ts, $($a:expr),*) => { obs_encoder_gpu_gates_requeue_ts($($a),*) };
}
macro_rules! paste_oracle {
    (divisor_inc, $($a:expr),*) => { oracle_obs_encoder_gpu_gates_divisor_inc($($a),*) };
    (divisor_wrap, $($a:expr),*) => { oracle_obs_encoder_gpu_gates_divisor_wrap($($a),*) };
    (divisor_skip, $($a:expr),*) => { oracle_obs_encoder_gpu_gates_divisor_skip($($a),*) };
    (next_key, $($a:expr),*) => { oracle_obs_encoder_gpu_gates_next_key($($a),*) };
    (record_timing, $($a:expr),*) => { oracle_obs_encoder_gpu_gates_record_timing($($a),*) };
    (video_pts, $($a:expr),*) => { oracle_obs_encoder_audio_start_video_pts($($a),*) };
    (count_next, $($a:expr),*) => { oracle_obs_encoder_gpu_gates_count_next($($a),*) };
    (requeue, $($a:expr),*) => { oracle_obs_encoder_gpu_gates_requeue($($a),*) };
    (requeue_ts, $($a:expr),*) => { oracle_obs_encoder_gpu_gates_requeue_ts($($a),*) };
}

side!(rust, shim);
side!(c, oracle);

pub fn benchmarks(criterion: &mut Criterion) {
    pair(
        criterion,
        "encoder_gpu_gates/gpu_encode_thread/1024",
        rust,
        c,
    );
}
