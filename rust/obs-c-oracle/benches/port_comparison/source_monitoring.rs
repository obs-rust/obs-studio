//! obs-source: the monitoring, monitor-only mute, balance, mixer, flag,
//! push-to and audio active rules (`libobs/obs-source-monitoring.c.h`).
//! Every flag, mixer, balance and monitoring change of a source asks them,
//! and each audio tick that turns a source's audio on or off asks the
//! active signal. Each side runs 1024 updates over every combination of the
//! flags the C reads, with monitoring types in and out of the enum, mixers
//! a handler may leave past 32 bits and balances at NaN, the ends of the
//! range and beyond; each asks, in the C's order, the same-value rule, the
//! audio gate, the stored mixers, the push-to log rule, the monitoring
//! types and monitor route, the type's route and its three mute steps, the
//! balance twice (the value sent and the one a handler leaves) and the
//! active signal; nothing is allocated, and the results fold into a
//! checksum both must match.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::source_monitoring as oracle;
use obs_source::ffi::source_monitoring as shim;
use std::hint::black_box;
use std::os::raw::c_int;

const N: u32 = 1024;

/// Update `i`: its flags, monitoring type, handler's mixers and balance.
fn update(i: u32) -> (u32, c_int, i64, f32) {
    let ty = [0, 1, 2, 3, -1][usize::try_from(i % 5).unwrap_or(0)];
    let mixers = [0x3f, -1, (1_i64 << 32) + 3, 0, 1][usize::try_from(i % 5).unwrap_or(0)];
    let balance = [f32::NAN, 0.5, -0.0, 2.0, -1.0, 0.25, 1.0][usize::try_from(i % 7).unwrap_or(0)];
    (i, ty, mixers, balance)
}

fn bit(flags: u32, n: u32) -> bool {
    flags & (1 << n) != 0
}

fn fold(acc: u64, values: [u64; 15]) -> u64 {
    values
        .iter()
        .fold(acc, |a, &v| a.wrapping_mul(31).wrapping_add(v))
}

/// An `int` as the checksum's `u64` (every value here is 0, 1 or 2).
fn int(v: c_int) -> u64 {
    u64::try_from(v).unwrap_or(u64::MAX)
}

fn rust() -> u64 {
    (0..N).fold(0, |acc, i| {
        let (f, ty, mx, bal) = black_box(update(i));
        let b = |n| bit(f, n);
        fold(
            acc,
            [
                u64::from(shim::obs_source_monitoring_changes(f & 7, f >> 3 & 7)),
                u64::from(shim::obs_source_monitoring_has_audio(f)),
                u64::from(shim::obs_source_monitoring_mixers_store(mx)),
                u64::from(shim::obs_source_monitoring_push_to_logs(f, b(4))),
                int(shim::obs_source_monitoring_on_type(b(5))),
                int(shim::obs_source_monitoring_stored_type(b(6))),
                u64::from(shim::obs_source_monitoring_monitor(b(6), b(7))),
                u64::from(shim::obs_source_monitoring_route(ty)),
                u64::from(shim::obs_source_monitoring_off_reads_mute(b(8))),
                u64::from(shim::obs_source_monitoring_off_unmutes(b(9))),
                u64::from(shim::obs_source_monitoring_only_mutes(b(9))),
                u64::from(shim::obs_source_monitoring_balance(bal).to_bits()),
                u64::from(shim::obs_source_monitoring_balance(-bal).to_bits()),
                u64::from(shim::obs_source_monitoring_audio_active(b(0), b(1))),
                u64::from(i),
            ],
        )
    })
}

fn c() -> u64 {
    (0..N).fold(0, |acc, i| {
        let (f, ty, mx, bal) = black_box(update(i));
        let b = |n| bit(f, n);
        // SAFETY: plain values in and out; the C touches no memory.
        let values = unsafe {
            [
                u64::from(oracle::oracle_obs_source_monitoring_changes(
                    f & 7,
                    f >> 3 & 7,
                )),
                u64::from(oracle::oracle_obs_source_monitoring_has_audio(f)),
                u64::from(oracle::oracle_obs_source_monitoring_mixers_store(mx)),
                u64::from(oracle::oracle_obs_source_monitoring_push_to_logs(f, b(4))),
                int(oracle::oracle_obs_source_monitoring_on_type(b(5))),
                int(oracle::oracle_obs_source_monitoring_stored_type(b(6))),
                u64::from(oracle::oracle_obs_source_monitoring_monitor(b(6), b(7))),
                u64::from(oracle::oracle_obs_source_monitoring_route(ty)),
                u64::from(oracle::oracle_obs_source_monitoring_off_reads_mute(b(8))),
                u64::from(oracle::oracle_obs_source_monitoring_off_unmutes(b(9))),
                u64::from(oracle::oracle_obs_source_monitoring_only_mutes(b(9))),
                u64::from(oracle::oracle_obs_source_monitoring_balance(bal).to_bits()),
                u64::from(oracle::oracle_obs_source_monitoring_balance(-bal).to_bits()),
                u64::from(oracle::oracle_obs_source_monitoring_audio_active(
                    b(0),
                    b(1),
                )),
                u64::from(i),
            ]
        };
        fold(acc, values)
    })
}

pub fn benchmarks(criterion: &mut Criterion) {
    pair(
        criterion,
        "source_monitoring/obs_source_set_monitoring_type/1024",
        rust,
        c,
    );
}
