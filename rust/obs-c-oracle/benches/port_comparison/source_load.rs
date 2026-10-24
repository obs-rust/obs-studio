//! obs-core: the source load migration and value rules
//! (`libobs/obs-source-load.c.h`). `obs_load_source_type()` calls each of
//! the six once per source or filter it loads from a scene collection: a
//! few hundred calls when a collection loads, none per frame. Each side runs
//! 1024 loads' worth over fixed saved values (old and new versions, with and
//! without the monitoring cap, every monitoring type, volumes and balances in
//! and out of range with a NaN now and then, delays and modes either side of
//! their ranges) and folds the results into a checksum both must match.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::source_load::{
    oracle_obs_source_load_balance, oracle_obs_source_load_deinterlace_mode,
    oracle_obs_source_load_delay, oracle_obs_source_load_monitoring_migration,
    oracle_obs_source_load_prev_ver, oracle_obs_source_load_volume,
};
use obs_core::ffi::source_load::{
    obs_source_load_balance, obs_source_load_deinterlace_mode, obs_source_load_delay,
    obs_source_load_monitoring_migration, obs_source_load_prev_ver, obs_source_load_volume,
};
use std::hint::black_box;

const N: u32 = 1024;

/// One source's saved values: prev_ver, caps, monitoring type, volume,
/// balance, push-to-mute and push-to-talk delays, deinterlace mode.
#[derive(Clone, Copy)]
struct Saved {
    prev_ver: i64,
    caps: u32,
    monitoring_type: i64,
    volume: f64,
    balance: f64,
    ptm_delay: i64,
    ptt_delay: i64,
    deinterlace_mode: i64,
}

const VERSIONS: [i64; 6] = [
    0,
    0x1702_0001,
    0x1702_0002,
    0x1f00_0000,
    0x2100_0000,
    0x2002_0000,
];

fn saved() -> Vec<Saved> {
    (0..N)
        .map(|i| {
            let n = f64::from(i);
            Saved {
                prev_ver: VERSIONS[(i % 6) as usize],
                caps: if i.is_multiple_of(3) { 0x802 } else { 0x2 },
                monitoring_type: i64::from(i % 4),
                volume: if i.is_multiple_of(97) {
                    f64::NAN
                } else {
                    n / 512.0 - 0.25
                },
                balance: n / 900.0 - 0.05,
                ptm_delay: i64::from(i) * 7 - 300,
                ptt_delay: i64::from(i) * 13 - 100,
                deinterlace_mode: if i.is_multiple_of(11) {
                    1i64 << 33
                } else {
                    i64::from(i % 10)
                },
            }
        })
        .collect()
}

/// A float into the checksum; any NaN as one value (payloads may differ).
fn fold_float(acc: u64, f: f32) -> u64 {
    acc.wrapping_mul(31).wrapping_add(if f.is_nan() {
        1
    } else {
        u64::from(f.to_bits())
    })
}

fn rust_load(all: &[Saved]) -> u64 {
    all.iter().fold(0, |acc, s| {
        let s = black_box(*s);
        let prev = obs_source_load_prev_ver(s.prev_ver);
        let plan = obs_source_load_monitoring_migration(prev, s.caps, s.monitoring_type);
        let acc = fold_float(acc, obs_source_load_volume(s.volume));
        let acc = fold_float(acc, obs_source_load_balance(s.balance));
        acc.wrapping_add(obs_source_load_delay(s.ptm_delay))
            .wrapping_add(obs_source_load_delay(s.ptt_delay))
            .wrapping_add(
                obs_source_load_deinterlace_mode(s.deinterlace_mode)
                    .unsigned_abs()
                    .into(),
            )
            .wrapping_add(u64::from(plan.mixers))
            .wrapping_add(u64::from(plan.set_mixers) << 8)
            .wrapping_add(u64::from(plan.store_enabled) << 9)
            .wrapping_add(u64::from(plan.enabled) << 10)
    })
}

fn c_load(all: &[Saved]) -> u64 {
    all.iter().fold(0, |acc, s| {
        let s = black_box(*s);
        // SAFETY: plain value functions.
        unsafe {
            let prev = oracle_obs_source_load_prev_ver(s.prev_ver);
            let plan = oracle_obs_source_load_monitoring_migration(prev, s.caps, s.monitoring_type);
            let acc = fold_float(acc, oracle_obs_source_load_volume(s.volume));
            let acc = fold_float(acc, oracle_obs_source_load_balance(s.balance));
            acc.wrapping_add(oracle_obs_source_load_delay(s.ptm_delay))
                .wrapping_add(oracle_obs_source_load_delay(s.ptt_delay))
                .wrapping_add(
                    oracle_obs_source_load_deinterlace_mode(s.deinterlace_mode)
                        .unsigned_abs()
                        .into(),
                )
                .wrapping_add(u64::from(plan.mixers))
                .wrapping_add(u64::from(plan.set_mixers) << 8)
                .wrapping_add(u64::from(plan.store_enabled) << 9)
                .wrapping_add(u64::from(plan.enabled) << 10)
        }
    })
}

pub fn benchmarks(criterion: &mut Criterion) {
    let s = saved();
    pair(
        criterion,
        "source_load/load_rules/1024",
        || rust_load(&s),
        || c_load(&s),
    );
}
