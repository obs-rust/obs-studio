//! obs-output: the encoder callback and connection lifecycle decisions
//! (`libobs/obs-encoder-lifecycle.c.h`). The start and stop decisions run
//! once per output start and stop for each of its encoders, the connection
//! route and the group counts once per encoder connect and disconnect, and
//! the first packet route on every encoded packet (`send_packet()`, for
//! each output the encoder feeds). Each side runs 1024 rounds over encoders
//! of both types with callback counts, indexes, caps, group counts and SEI
//! answers taken from a fixed table, asking every decision a start, a send,
//! a first keyframe and a stop ask, nothing is allocated, and the results
//! fold into a checksum both must match.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::encoder_audio_start::oracle_obs_encoder_audio_start_not_video;
use obs_c_oracle::encoder_bind::oracle_obs_encoder_bind_not_audio;
use obs_c_oracle::encoder_lifecycle::{
    oracle_obs_encoder_lifecycle_first, oracle_obs_encoder_lifecycle_found,
    oracle_obs_encoder_lifecycle_group_idle, oracle_obs_encoder_lifecycle_group_ready,
    oracle_obs_encoder_lifecycle_last, oracle_obs_encoder_lifecycle_lcm_unset,
    oracle_obs_encoder_lifecycle_pass_texture, oracle_obs_encoder_lifecycle_prepend_sei,
    oracle_obs_encoder_lifecycle_started_dec, oracle_obs_encoder_lifecycle_started_inc,
};
use obs_output::ffi::encoder_audio_start::obs_encoder_audio_start_not_video;
use obs_output::ffi::encoder_bind::obs_encoder_bind_not_audio;
use obs_output::ffi::encoder_lifecycle::{
    obs_encoder_lifecycle_first, obs_encoder_lifecycle_found, obs_encoder_lifecycle_group_idle,
    obs_encoder_lifecycle_group_ready, obs_encoder_lifecycle_last, obs_encoder_lifecycle_lcm_unset,
    obs_encoder_lifecycle_pass_texture, obs_encoder_lifecycle_prepend_sei,
    obs_encoder_lifecycle_started_dec, obs_encoder_lifecycle_started_inc,
};
use std::hint::black_box;

const N: u32 = 1024;
const CAPS: [u32; 4] = [0, 2, 3, 0xff];
const IDX: [usize; 4] = [0, 1, usize::MAX, 2];

macro_rules! side {
    ($name:ident, $call:ident) => {
        fn $name() -> u64 {
            let mut acc = 0u64;
            for i in 0..N {
                let t = black_box(i32::from(i % 2 == 1));
                let n = black_box((i % 4) as usize);
                let idx = black_box(IDX[(i % 4) as usize]);
                // the start
                let first = $call!(first(n));
                let added = !$call!(found(idx));
                acc = acc
                    .wrapping_mul(31)
                    .wrapping_add(u64::from(first) * 2 + u64::from(added));
                if added && !$call!(not_video(t)) {
                    acc = acc.wrapping_add(1);
                }
                // the connection
                if $call!(not_audio(t)) {
                    acc = acc.wrapping_add(u64::from($call!(pass_texture(black_box(
                        CAPS[(i % 4) as usize]
                    )))));
                }
                let started = black_box(i % 3);
                if $call!(lcm_unset(black_box(i % 2))) {
                    acc = acc.wrapping_add(3);
                }
                let up = $call!(started_inc(started));
                acc = acc
                    .wrapping_mul(31)
                    .wrapping_add(u64::from($call!(group_ready(up, black_box(2)))));
                // a send and the first keyframe
                if !$call!(not_video(t)) && i % 5 == 0 {
                    let sei = $call!(prepend_sei(
                        black_box(i % 3 != 0),
                        black_box(i % 7 != 0),
                        black_box((i % 4) as usize)
                    ));
                    acc = acc.wrapping_add(u64::from(sei));
                }
                // the stop
                if $call!(found(idx)) && $call!(last(black_box(n.saturating_sub(1)))) {
                    let down = $call!(started_dec(up));
                    acc = acc
                        .wrapping_mul(31)
                        .wrapping_add(u64::from($call!(group_idle(down))));
                }
            }
            acc
        }
    };
}

macro_rules! shim {
    (not_video($($a:expr),*)) => { obs_encoder_audio_start_not_video($($a),*) };
    (not_audio($($a:expr),*)) => { obs_encoder_bind_not_audio($($a),*) };
    (first($($a:expr),*)) => { obs_encoder_lifecycle_first($($a),*) };
    (found($($a:expr),*)) => { obs_encoder_lifecycle_found($($a),*) };
    (last($($a:expr),*)) => { obs_encoder_lifecycle_last($($a),*) };
    (pass_texture($($a:expr),*)) => { obs_encoder_lifecycle_pass_texture($($a),*) };
    (lcm_unset($($a:expr),*)) => { obs_encoder_lifecycle_lcm_unset($($a),*) };
    (started_inc($($a:expr),*)) => { obs_encoder_lifecycle_started_inc($($a),*) };
    (group_ready($($a:expr),*)) => { obs_encoder_lifecycle_group_ready($($a),*) };
    (started_dec($($a:expr),*)) => { obs_encoder_lifecycle_started_dec($($a),*) };
    (group_idle($($a:expr),*)) => { obs_encoder_lifecycle_group_idle($($a),*) };
    (prepend_sei($($a:expr),*)) => { obs_encoder_lifecycle_prepend_sei($($a),*) };
}

// SAFETY (each): plain values; nothing in the oracle divides.
macro_rules! oracle {
    (not_video($($a:expr),*)) => { unsafe { oracle_obs_encoder_audio_start_not_video($($a),*) } };
    (not_audio($($a:expr),*)) => { unsafe { oracle_obs_encoder_bind_not_audio($($a),*) } };
    (first($($a:expr),*)) => { unsafe { oracle_obs_encoder_lifecycle_first($($a),*) } };
    (found($($a:expr),*)) => { unsafe { oracle_obs_encoder_lifecycle_found($($a),*) } };
    (last($($a:expr),*)) => { unsafe { oracle_obs_encoder_lifecycle_last($($a),*) } };
    (pass_texture($($a:expr),*)) => { unsafe { oracle_obs_encoder_lifecycle_pass_texture($($a),*) } };
    (lcm_unset($($a:expr),*)) => { unsafe { oracle_obs_encoder_lifecycle_lcm_unset($($a),*) } };
    (started_inc($($a:expr),*)) => { unsafe { oracle_obs_encoder_lifecycle_started_inc($($a),*) } };
    (group_ready($($a:expr),*)) => { unsafe { oracle_obs_encoder_lifecycle_group_ready($($a),*) } };
    (started_dec($($a:expr),*)) => { unsafe { oracle_obs_encoder_lifecycle_started_dec($($a),*) } };
    (group_idle($($a:expr),*)) => { unsafe { oracle_obs_encoder_lifecycle_group_idle($($a),*) } };
    (prepend_sei($($a:expr),*)) => { unsafe { oracle_obs_encoder_lifecycle_prepend_sei($($a),*) } };
}

side!(rust, shim);
side!(c, oracle);

pub fn benchmarks(criterion: &mut Criterion) {
    pair(criterion, "encoder_lifecycle/start_send_stop/1024", rust, c);
}
