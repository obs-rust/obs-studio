//! obs-output: the output stop and end-capture decisions
//! (`libobs/obs-output-stop.c.h`). They run once per output stop (every
//! stream, recording, replay buffer and virtual camera stop, and each
//! reconnect and stream that never connected). Each side runs 1024 rounds
//! over flag words of the common outputs (stream, recording, raw video, raw
//! AV) with delays, restart counts, thread results and mixer masks taken
//! from a fixed table, asking every decision obs_output_stop(),
//! obs_output_end_data_capture_internal(), end_data_capture_thread() and
//! stop_raw_audio() ask, in their order; nothing is allocated, and the
//! results fold into a checksum both must match.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::output_raw_audio::{
    oracle_obs_output_raw_audio_interleave, oracle_obs_output_raw_audio_mix_on,
};
use obs_c_oracle::output_stop::{
    oracle_obs_output_stop_delay_on, oracle_obs_output_stop_encoded,
    oracle_obs_output_stop_end_delay, oracle_obs_output_stop_has_audio,
    oracle_obs_output_stop_has_video, oracle_obs_output_stop_service,
    oracle_obs_output_stop_thread_failed,
};
use obs_output::ffi::output_raw_audio::{
    obs_output_raw_audio_interleave, obs_output_raw_audio_mix_on,
};
use obs_output::ffi::output_stop::{
    obs_output_stop_delay_on, obs_output_stop_encoded, obs_output_stop_end_delay,
    obs_output_stop_has_audio, obs_output_stop_has_video, obs_output_stop_service,
    obs_output_stop_thread_failed,
};
use std::ffi::{c_int, c_long};
use std::hint::black_box;

const N: u32 = 1024;
/// stream, recording, raw video, raw AV
const FLAGS: [u32; 4] = [0b1111, 0b0111, 0b0001, 0b0011];
const DELAYS: [u64; 2] = [0, 20_000_000_000];
const MASKS: [usize; 3] = [0b1, 0b101, 0b10_0110];

macro_rules! side {
    ($name:ident, $call:ident) => {
        fn $name() -> u64 {
            let mut acc = 0u64;
            for i in 0..N {
                let flags = black_box(FLAGS[usize::from(i % 4 == 1) + 2 * usize::from(i % 4 >= 2)]);
                let delay = black_box(DELAYS[usize::from(i % 8 == 7)]);
                let refs: c_long = black_box(c_long::from(i % 5 == 0));
                let ret: c_int = black_box(if i % 16 == 3 { 11 } else { 0 });
                let mask = black_box(MASKS[usize::from(i % 3 == 1) + 2 * usize::from(i % 3 == 2)]);
                let mut bits = 0u64;
                // obs_output_stop()
                let encoded = $call!(encoded(flags));
                bits = (bits << 1) | u64::from(encoded);
                if encoded {
                    bits = (bits << 1) | u64::from($call!(delay_on(delay)));
                }
                // obs_output_end_data_capture_internal()
                bits = (bits << 1) | u64::from($call!(end_delay(refs)));
                bits = (bits << 1) | u64::from($call!(has_video(flags)));
                bits = (bits << 1) | u64::from($call!(thread_failed(ret)));
                // end_data_capture_thread()
                let has_video = $call!(has_video(flags));
                let has_audio = $call!(has_audio(flags));
                if $call!(encoded(flags)) {
                    bits = (bits << 1) | u64::from($call!(delay_on(delay)));
                    bits = (bits << 1) | u64::from($call!(interleave(has_video, has_audio)));
                } else if has_audio {
                    // stop_raw_audio(), with raw_audio2
                    for idx in 0..6 {
                        bits = (bits << 1) | u64::from($call!(mix_on(mask, idx)));
                    }
                }
                bits = (bits << 1) | u64::from($call!(service(flags)));
                bits = (bits << 1) | u64::from($call!(delay_on(delay)));
                acc = acc.wrapping_mul(131).wrapping_add(bits);
            }
            acc
        }
    };
}

macro_rules! shim {
    (encoded($($a:expr),*)) => { obs_output_stop_encoded($($a),*) };
    (delay_on($($a:expr),*)) => { obs_output_stop_delay_on($($a),*) };
    (has_video($($a:expr),*)) => { obs_output_stop_has_video($($a),*) };
    (has_audio($($a:expr),*)) => { obs_output_stop_has_audio($($a),*) };
    (service($($a:expr),*)) => { obs_output_stop_service($($a),*) };
    (end_delay($($a:expr),*)) => { obs_output_stop_end_delay($($a),*) };
    (thread_failed($($a:expr),*)) => { obs_output_stop_thread_failed($($a),*) };
    (interleave($($a:expr),*)) => { obs_output_raw_audio_interleave($($a),*) };
    (mix_on($($a:expr),*)) => { obs_output_raw_audio_mix_on($($a),*) };
}

// SAFETY (each): plain values; nothing in the oracle divides, and the mix
// test is only asked below six.
macro_rules! oracle {
    (encoded($($a:expr),*)) => { unsafe { oracle_obs_output_stop_encoded($($a),*) } };
    (delay_on($($a:expr),*)) => { unsafe { oracle_obs_output_stop_delay_on($($a),*) } };
    (has_video($($a:expr),*)) => { unsafe { oracle_obs_output_stop_has_video($($a),*) } };
    (has_audio($($a:expr),*)) => { unsafe { oracle_obs_output_stop_has_audio($($a),*) } };
    (service($($a:expr),*)) => { unsafe { oracle_obs_output_stop_service($($a),*) } };
    (end_delay($($a:expr),*)) => { unsafe { oracle_obs_output_stop_end_delay($($a),*) } };
    (thread_failed($($a:expr),*)) => { unsafe { oracle_obs_output_stop_thread_failed($($a),*) } };
    (interleave($($a:expr),*)) => { unsafe { oracle_obs_output_raw_audio_interleave($($a),*) } };
    (mix_on($($a:expr),*)) => { unsafe { oracle_obs_output_raw_audio_mix_on($($a),*) } };
}

side!(rust, shim);
side!(c, oracle);

pub fn benchmarks(criterion: &mut Criterion) {
    pair(criterion, "output_stop/stop_and_end/1024", rust, c);
}
