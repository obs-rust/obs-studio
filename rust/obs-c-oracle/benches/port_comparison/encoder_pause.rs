//! obs-output: the encoder pause checks (`libobs/obs-encoder-pause.c.h`).
//! Every video frame and every audio buffer of a recording's encoders asks
//! whether a pause is on and, while one is, what the frame or buffer does.
//! Each side runs 10 s of a recording at 60 fps with 1024-frame buffers at
//! 48 kHz (600 frames, 469 buffers), paused from 3 s to 6 s, through the
//! checks as `obs-encoder.c` asks them, and folds the answers, the record
//! and the buffers into a checksum both must match. The pause ends on a
//! frame and inside a buffer, where the port keeps the C's answers. Nothing
//! is allocated in the loop.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::encoder_pause::{
    oracle_obs_encoder_pause_audio_cut_ns, oracle_obs_encoder_pause_audio_drop,
    oracle_obs_encoder_pause_audio_end, oracle_obs_encoder_pause_audio_kept,
    oracle_obs_encoder_pause_audio_left, oracle_obs_encoder_pause_audio_paused,
    oracle_obs_encoder_pause_audio_start, oracle_obs_encoder_pause_off,
    oracle_obs_encoder_pause_video_drop, oracle_obs_encoder_pause_video_end,
};
use obs_output::ffi::encoder_pause::{
    OBS_ENCODER_PAUSE_AUDIO_PAUSED_RESUME, OBS_ENCODER_PAUSE_AUDIO_PAUSED_UNPAUSE,
    OBS_ENCODER_PAUSE_AUDIO_START_BEFORE, OBS_ENCODER_PAUSE_AUDIO_START_INSIDE,
    obs_encoder_pause_audio_cut_ns,
    obs_encoder_pause_audio_drop, obs_encoder_pause_audio_end, obs_encoder_pause_audio_kept,
    obs_encoder_pause_audio_left, obs_encoder_pause_audio_paused, obs_encoder_pause_audio_start,
    obs_encoder_pause_off, obs_encoder_pause_video_drop, obs_encoder_pause_video_end,
};
use obs_output::output_begin::mul_div64;
use std::hint::black_box;

const T0: u64 = 1_000_000_000;
const NS: u64 = 1_000_000_000;
const FRAME: u64 = 16_666_666;
const FRAMES: u64 = 600;
const RATE: u64 = 48000;
const BUFFER: u32 = 1024;
const BUFFERS: u64 = 469;
const START: u64 = T0 + 180 * FRAME;
const STOP: u64 = T0 + 360 * FRAME;

fn fold(acc: u64, value: u64) -> u64 {
    acc.wrapping_mul(31).wrapping_add(value)
}

/// The recording through `video_pause_check()` and `audio_pause_check()`,
/// from the ten functions of one side.
macro_rules! recording {
    ($off:expr, $video_end:expr, $video_drop:expr, $audio_end:expr, $audio_start:expr, $audio_paused:expr,
     $cut_ns:expr, $kept:expr, $left:expr, $drop:expr) => {{
        let mut acc = 0;
        let (mut ts_start, mut ts_end, mut ts_offset) = (0_u64, 0_u64, 0_u64);
        for n in 0..FRAMES {
            let ts = black_box(T0 + n * FRAME);
            if ts == START {
                ts_start = START;
            }
            if ts == STOP - 2 * FRAME {
                ts_end = STOP;
                ts_offset += STOP - START;
            }
            let mut drop = false;
            if !$off(ts_start) {
                if $video_end(ts_end, ts) {
                    ts_start = 0;
                    ts_end = 0;
                } else if $video_drop(ts_start, ts) {
                    drop = true;
                }
            }
            acc = fold(acc, u64::from(drop));
        }
        let len = mul_div64(u64::from(BUFFER), NS, RATE);
        let (mut a_start, mut a_end) = (START, STOP);
        for k in 0..BUFFERS {
            let mut timestamp = black_box(T0 + k * len);
            let mut frames = BUFFER;
            let mut advanced = 0_u64;
            let mut drop = false;
            if !$off(a_start) {
                let end_ts = $audio_end(timestamp, mul_div64(u64::from(frames), NS, RATE));
                let start = $audio_start(a_start, timestamp, end_ts);
                if start == OBS_ENCODER_PAUSE_AUDIO_START_INSIDE {
                    let cut = mul_div64($cut_ns(a_start, timestamp), RATE, NS);
                    frames = $kept(cut);
                    drop = $drop(frames);
                } else if start == OBS_ENCODER_PAUSE_AUDIO_START_BEFORE {
                    let paused = $audio_paused(a_end, timestamp, end_ts);
                    if paused == OBS_ENCODER_PAUSE_AUDIO_PAUSED_UNPAUSE {
                        let cut = mul_div64($cut_ns(a_end, timestamp), RATE, NS);
                        advanced = cut * 4;
                        timestamp = a_start;
                        frames = $left(frames, cut);
                        a_start = 0;
                        a_end = 0;
                        drop = $drop(frames);
                    } else if paused == OBS_ENCODER_PAUSE_AUDIO_PAUSED_RESUME {
                        a_start = 0;
                        a_end = 0;
                        drop = $drop(frames);
                    } else {
                        drop = true;
                    }
                }
            }
            // audio_pause_check() takes the offset off every buffer.
            timestamp = timestamp.wrapping_sub(ts_offset);
            acc = fold(fold(fold(fold(acc, u64::from(drop)), u64::from(frames)), timestamp), advanced);
        }
        fold(fold(acc, ts_start | ts_end), a_start | a_end)
    }};
}

fn rust_recording() -> u64 {
    recording!(
        obs_encoder_pause_off,
        obs_encoder_pause_video_end,
        obs_encoder_pause_video_drop,
        obs_encoder_pause_audio_end,
        obs_encoder_pause_audio_start,
        obs_encoder_pause_audio_paused,
        obs_encoder_pause_audio_cut_ns,
        obs_encoder_pause_audio_kept,
        obs_encoder_pause_audio_left,
        obs_encoder_pause_audio_drop
    )
}

fn c_recording() -> u64 {
    // SAFETY: plain values; every input is defined in the C.
    unsafe {
        recording!(
            oracle_obs_encoder_pause_off,
            oracle_obs_encoder_pause_video_end,
            oracle_obs_encoder_pause_video_drop,
            oracle_obs_encoder_pause_audio_end,
            oracle_obs_encoder_pause_audio_start,
            oracle_obs_encoder_pause_audio_paused,
            oracle_obs_encoder_pause_audio_cut_ns,
            oracle_obs_encoder_pause_audio_kept,
            oracle_obs_encoder_pause_audio_left,
            oracle_obs_encoder_pause_audio_drop
        )
    }
}

pub fn benchmarks(criterion: &mut Criterion) {
    pair(criterion, "encoder_pause/recording/10s", rust_recording, c_recording);
}
