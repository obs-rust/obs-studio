//! obs-output: the output start and begin-capture decisions
//! (`libobs/obs-output-start.c.h`). They run once per output start (every
//! stream, recording, replay buffer and virtual camera start, and each
//! reconnect and delayed restart). Each side runs 1024 rounds over flag
//! words of the common outputs (stream, recording, raw video, raw AV) with
//! restart counts, delays and conversions taken from a fixed table, asking
//! every decision obs_output_start(), obs_output_actual_start(),
//! obs_output_initialize_encoders(), reset_raw_output() and
//! obs_output_begin_data_capture_internal() ask, in their order; nothing is
//! allocated, and the results fold into a checksum both must match.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::encoder_size::{AudioFill, oracle_obs_encoder_size_audio_defaults};
use obs_c_oracle::output_raw_audio::oracle_obs_output_raw_audio_delay_on;
use obs_c_oracle::output_start::{
    oracle_obs_output_start_drop_restart_ref, oracle_obs_output_start_init_audio,
    oracle_obs_output_start_init_video, oracle_obs_output_start_pair,
    oracle_obs_output_start_reset_raw, oracle_obs_output_start_service,
};
use obs_output::ffi::encoder_size::{obs_encoder_audio_fill, obs_encoder_size_audio_defaults};
use obs_output::ffi::output_raw_audio::obs_output_raw_audio_delay_on;
use obs_output::ffi::output_start::{
    obs_output_start_drop_restart_ref, obs_output_start_init_audio, obs_output_start_init_video,
    obs_output_start_pair, obs_output_start_reset_raw, obs_output_start_service,
};
use std::ffi::c_long;
use std::hint::black_box;

const N: u32 = 1024;
/// stream, recording, raw video, raw AV
const FLAGS: [u32; 4] = [0b1111, 0b0111, 0b0001, 0b0011];
const DELAYS: [u32; 4] = [0, 0, 0, 20];
/// (format, rate, speakers): unset, a rate, a format, all three
const CONVS: [(i32, u32, i32); 4] = [(0, 0, 0), (0, 44100, 0), (2, 0, 0), (2, 44100, 6)];

macro_rules! side {
    ($name:ident, $call:ident, $fill:ident) => {
        fn $name() -> u64 {
            let mut acc = 0u64;
            for i in 0..N {
                let flags = black_box(FLAGS[usize::from(i % 4 == 1) + 2 * usize::from(i % 4 >= 2)]);
                let delay = black_box(DELAYS[usize::from(i % 8 == 7) * 3]);
                let refs: c_long = black_box(c_long::from(i % 5 == 0));
                let mut bits = 0u64;
                // obs_output_start()
                bits = (bits << 1) | u64::from($call!(service(flags)));
                bits = (bits << 1) | u64::from($call!(delay_on(delay)));
                // obs_output_actual_start()
                bits = (bits << 1) | u64::from($call!(drop_restart_ref(refs)));
                // obs_output_initialize_encoders()
                bits = (bits << 1) | u64::from($call!(init_video(flags)));
                bits = (bits << 1) | u64::from($call!(init_audio(flags)));
                // obs_output_begin_data_capture_internal(), reset_raw_output()
                if $call!(reset_raw(flags)) {
                    let (f, r, s) = CONVS[usize::from(i % 3 == 1) + 2 * usize::from(i % 3 == 2)];
                    let conv = $fill {
                        format: f,
                        samples_per_sec: r,
                        speakers: s,
                    };
                    let mix = $fill {
                        format: 8,
                        samples_per_sec: 48000,
                        speakers: 2,
                    };
                    let out = $call!(audio_defaults(&conv, &mix));
                    acc = acc.wrapping_mul(31).wrapping_add(
                        (u64::from(out.samples_per_sec) << 16)
                            ^ u64::from(out.format.unsigned_abs())
                            ^ (u64::from(out.speakers.unsigned_abs()) << 8),
                    );
                }
                bits = (bits << 1) | u64::from($call!(pair(flags)));
                bits = (bits << 1) | u64::from($call!(service(flags)));
                acc = acc.wrapping_mul(131).wrapping_add(bits);
            }
            acc
        }
    };
}

// SAFETY (audio_defaults): both point to live fills for the call.
macro_rules! shim {
    (service($($a:expr),*)) => { obs_output_start_service($($a),*) };
    (delay_on($($a:expr),*)) => { obs_output_raw_audio_delay_on($($a),*) };
    (drop_restart_ref($($a:expr),*)) => { obs_output_start_drop_restart_ref($($a),*) };
    (init_video($($a:expr),*)) => { obs_output_start_init_video($($a),*) };
    (init_audio($($a:expr),*)) => { obs_output_start_init_audio($($a),*) };
    (reset_raw($($a:expr),*)) => { obs_output_start_reset_raw($($a),*) };
    (pair($($a:expr),*)) => { obs_output_start_pair($($a),*) };
    (audio_defaults($c:expr, $m:expr)) => { unsafe { obs_encoder_size_audio_defaults($c, $m) } };
}

// SAFETY (each): plain values, and for audio_defaults two live fills;
// nothing in the oracle divides.
macro_rules! oracle {
    (service($($a:expr),*)) => { unsafe { oracle_obs_output_start_service($($a),*) } };
    (delay_on($($a:expr),*)) => { unsafe { oracle_obs_output_raw_audio_delay_on($($a),*) } };
    (drop_restart_ref($($a:expr),*)) => { unsafe { oracle_obs_output_start_drop_restart_ref($($a),*) } };
    (init_video($($a:expr),*)) => { unsafe { oracle_obs_output_start_init_video($($a),*) } };
    (init_audio($($a:expr),*)) => { unsafe { oracle_obs_output_start_init_audio($($a),*) } };
    (reset_raw($($a:expr),*)) => { unsafe { oracle_obs_output_start_reset_raw($($a),*) } };
    (pair($($a:expr),*)) => { unsafe { oracle_obs_output_start_pair($($a),*) } };
    (audio_defaults($c:expr, $m:expr)) => {
        unsafe { oracle_obs_encoder_size_audio_defaults(std::ptr::from_ref($c).cast(), std::ptr::from_ref($m).cast()) }
    };
}

side!(rust, shim, obs_encoder_audio_fill);
side!(c, oracle, AudioFill);

pub fn benchmarks(criterion: &mut Criterion) {
    pair(criterion, "output_start/start_and_begin/1024", rust, c);
}
