//! obs-output: the output reconnect policy
//! (`libobs/obs-output-reconnect.c.h`). Every stop an output's plugin
//! reports asks whether reconnecting is on and whether the stop reconnects,
//! and each attempt asks for its wait and whether to give up. Each side runs
//! 256 disconnects of a stream with the default settings (a 2 s delay, 20
//! retries, an exponent of 1.5 to 1.55), each through all 20 attempts and
//! the give-up, and folds the answers into a checksum both must match.
//! Nothing is allocated in the loop.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::output_reconnect::{
    oracle_obs_output_reconnect_active, oracle_obs_output_reconnect_can,
    oracle_obs_output_reconnect_first_delay, oracle_obs_output_reconnect_give_up,
    oracle_obs_output_reconnect_grows, oracle_obs_output_reconnect_next_delay,
};
use obs_output::ffi::output_reconnect::{
    obs_output_reconnect_active, obs_output_reconnect_can, obs_output_reconnect_first_delay,
    obs_output_reconnect_give_up, obs_output_reconnect_grows, obs_output_reconnect_next_delay,
};
use std::hint::black_box;

const DISCONNECTS: u32 = 256;
const RETRY_SEC: i32 = 2;
const RETRY_MAX: i32 = 20;
const DISCONNECTED: i32 = -5;

/// The exponent of disconnect `d`, spread over the jitter at create.
fn exp(d: u32) -> f32 {
    1.5 + f32::from(u16::try_from(d % 64).unwrap_or(0)) / 64.0 * 0.05
}

fn fold(acc: u64, value: u64) -> u64 {
    acc.wrapping_mul(31).wrapping_add(value)
}

fn rust_disconnects() -> u64 {
    (0..DISCONNECTS).fold(0, |mut acc, d| {
        let d = black_box(d);
        let on = obs_output_reconnect_active(RETRY_MAX);
        let mut reconnecting = false;
        let mut retries = 0;
        let mut cur = 0;
        while obs_output_reconnect_can(reconnecting, on, DISCONNECTED) {
            if !reconnecting {
                cur = obs_output_reconnect_first_delay(RETRY_SEC);
                retries = 0;
            }
            if obs_output_reconnect_give_up(retries, RETRY_MAX) {
                break;
            }
            reconnecting = true;
            if obs_output_reconnect_grows(retries) {
                cur = obs_output_reconnect_next_delay(cur, exp(d));
            }
            retries += 1;
            acc = fold(acc, u64::from(cur));
        }
        fold(acc, u64::try_from(retries).unwrap_or(0))
    })
}

fn c_disconnects() -> u64 {
    (0..DISCONNECTS).fold(0, |mut acc, d| {
        let d = black_box(d);
        // SAFETY: plain values, in the C's defined range (a 2 s delay, a wait
        // at most 15 minutes times an exponent under 2).
        unsafe {
            let on = oracle_obs_output_reconnect_active(RETRY_MAX);
            let mut reconnecting = false;
            let mut retries = 0;
            let mut cur = 0;
            while oracle_obs_output_reconnect_can(reconnecting, on, DISCONNECTED) {
                if !reconnecting {
                    cur = oracle_obs_output_reconnect_first_delay(RETRY_SEC);
                    retries = 0;
                }
                if oracle_obs_output_reconnect_give_up(retries, RETRY_MAX) {
                    break;
                }
                reconnecting = true;
                if oracle_obs_output_reconnect_grows(retries) {
                    cur = oracle_obs_output_reconnect_next_delay(cur, exp(d));
                }
                retries += 1;
                acc = fold(acc, u64::from(cur));
            }
            fold(acc, u64::try_from(retries).unwrap_or(0))
        }
    })
}

pub fn benchmarks(criterion: &mut Criterion) {
    pair(criterion, "output_reconnect/disconnect/256", rust_disconnects, c_disconnects);
}
