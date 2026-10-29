//! obs-output: the encoder group reconfigure rules
//! (`libobs/obs-encoder-group-reconfig.c.h`). Every grouped encoder asks the
//! frame step once per video frame it encodes
//! (`handle_encoder_group_reconfigure_request()`), and `obs_encoder_update()`
//! asks the update rule. Each side runs a group of three encoders at 29.97
//! fps with frame rate divisors 1, 2 and 3 over 1024 frames, with an update
//! every 64 frames, and folds the answers and the states into a checksum
//! both must match. Nothing is allocated in the loop.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::encoder_group_reconfig::{
    OracleEncoder, OracleGroup, oracle_obs_encoder_group_reconfig_frame,
    oracle_obs_encoder_group_reconfig_update,
};
use obs_output::ffi::encoder_group_reconfig::{
    obs_encoder_group_reconfig, obs_encoder_group_reconfig_frame,
    obs_encoder_group_reconfig_update, obs_encoder_reconfig,
};
use std::hint::black_box;

const FRAMES: u64 = 1024;
const NUM: u32 = 1001; // 29.97 fps
const DIVISORS: [u32; 3] = [1, 2, 3];
const LCM: u32 = 6;
const UPDATE_EVERY: u64 = 64;

fn fold(acc: u64, values: &[u64]) -> u64 {
    values
        .iter()
        .fold(acc, |acc, v| acc.wrapping_mul(31).wrapping_add(*v))
}

fn rust_group() -> u64 {
    let mut group = obs_encoder_group_reconfig::default();
    let mut encoders = [obs_encoder_reconfig::default(); 3];
    let mut pts = [0i64; 3];
    (0..FRAMES).fold(0, |mut acc, f| {
        let f = black_box(f);
        if f % UPDATE_EVERY == 3 {
            // SAFETY: an exclusive, initialized local.
            unsafe {
                obs_encoder_group_reconfig_update(
                    &raw mut group,
                    3,
                    encoders[0].last_handled_reconfigure_request,
                );
            }
        }
        for i in 0..3 {
            if !f.is_multiple_of(u64::from(DIVISORS[i])) {
                continue;
            }
            // SAFETY: two exclusive, initialized locals.
            let apply = unsafe {
                obs_encoder_group_reconfig_frame(
                    &raw mut group,
                    3,
                    3,
                    LCM,
                    &raw mut encoders[i],
                    pts[i],
                    NUM,
                    DIVISORS[i],
                )
            };
            acc = fold(
                acc,
                &[
                    u64::from(apply),
                    group.next_pts.cast_unsigned(),
                    group.reconfigure_request,
                    encoders[i].last_handled_reconfigure_request,
                ],
            );
            pts[i] += i64::from(NUM) * i64::from(DIVISORS[i]);
        }
        acc
    })
}

fn c_group() -> u64 {
    let mut group = OracleGroup::default();
    let mut encoders = [OracleEncoder::default(); 3];
    let mut pts = [0i64; 3];
    (0..FRAMES).fold(0, |mut acc, f| {
        let f = black_box(f);
        if f % UPDATE_EVERY == 3 {
            // SAFETY: an exclusive, initialized local.
            unsafe {
                oracle_obs_encoder_group_reconfig_update(
                    &raw mut group,
                    3,
                    encoders[0].last_handled_reconfigure_request,
                );
            }
        }
        for i in 0..3 {
            if !f.is_multiple_of(u64::from(DIVISORS[i])) {
                continue;
            }
            // SAFETY: two exclusive, initialized locals; C is defined here
            // (timestep 6006, pts far from the i64 edge).
            let apply = unsafe {
                oracle_obs_encoder_group_reconfig_frame(
                    &raw mut group,
                    3,
                    3,
                    LCM,
                    &raw mut encoders[i],
                    pts[i],
                    NUM,
                    DIVISORS[i],
                )
            };
            acc = fold(
                acc,
                &[
                    u64::from(apply),
                    group.next_pts.cast_unsigned(),
                    group.reconfigure_request,
                    encoders[i].last_handled_reconfigure_request,
                ],
            );
            pts[i] += i64::from(NUM) * i64::from(DIVISORS[i]);
        }
        acc
    })
}

pub fn benchmarks(criterion: &mut Criterion) {
    pair(
        criterion,
        "encoder_group_reconfig/frame/1024",
        rust_group,
        c_group,
    );
}
