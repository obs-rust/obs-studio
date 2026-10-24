//! obs-source: the volume decisions (`libobs/obs-source-volume.c.h`), which
//! `apply_audio_volume()` runs for every audio source on every tick of the
//! mix, with `get_source_volume()`'s snap. Each measurement folds its
//! results into a checksum that both sides must match:
//! - `apply_volume`: 1024 ticks through `apply_audio_volume()` and
//!   `get_source_volume()` of `obs-source.c` on a stand-in source (the due
//!   compare when an action is queued, the gate through the ptm_gate
//!   functions of the same side, the snap, the step and the mix choice for
//!   each of the six mixes when it multiplies), with every combination of
//!   the six gate flags, volumes at and around 0 and 1, mixer masks from
//!   none to all and actions due and not due; the memset and the multiply
//!   are not run;
//! - `snap`: 1024 volumes, the snap edges and their neighbours, -0, NaN,
//!   +-inf and ordinary fader values, muted and not.
//!
//! Measured when it was added (x86_64 Linux, GCC, not pinned to a core;
//! median per 1024 calls): apply_volume 6.71 us Rust, 9.84 us C (0.68x);
//! snap 1.76 us Rust, 1.70 us C (1.03x). Under 10 ns a source a tick either
//! way, of a tick of 21.3 ms.

use std::hint::black_box;

use criterion::{Criterion, Throughput};
use obs_c_oracle::source_ptm_gate as c_ptm;
use obs_c_oracle::source_volume as c;
use obs_source::ffi::source_ptm_gate as rs_ptm;
use obs_source::ffi::source_volume as rs;

const COUNT: usize = 1024;
const TICK: u64 = 21_333_333;
const MIXES: usize = 6;

struct Side {
    stop_time: fn(u64, u64) -> u64,
    held: fn(u64, u64) -> bool,
    mutes: fn(bool, bool) -> bool,
    talk_mutes: fn(bool, bool) -> bool,
    snap: fn(bool, f32) -> f32,
    due: fn(u64, u64, u64) -> bool,
    step: fn(f32, u32) -> u32,
    mixes: fn(u32, u32, usize) -> bool,
}

const RUST: Side = Side {
    stop_time: |t, d| rs_ptm::obs_source_ptm_gate_stop_time(t, d),
    held: |t, s| rs_ptm::obs_source_ptm_gate_held(t, s),
    mutes: |e, a| rs_ptm::obs_source_ptm_gate_mutes(e, a),
    talk_mutes: |e, a| rs_ptm::obs_source_ptm_gate_talk_mutes(e, a),
    snap: |m, v| rs::obs_source_volume_snap(m, v),
    due: |a, t, d| rs::obs_source_volume_action_due(a, t, d),
    step: |v, m| rs::obs_source_volume_step(v, m),
    mixes: |a, m, x| rs::obs_source_volume_mixes(a, m, x),
};

const C: Side = Side {
    // SAFETY: plain values; the C touches no memory.
    stop_time: |t, d| unsafe { c_ptm::oracle_obs_source_ptm_gate_stop_time(t, d) },
    // SAFETY: as above.
    held: |t, s| unsafe { c_ptm::oracle_obs_source_ptm_gate_held(t, s) != 0 },
    // SAFETY: as above.
    mutes: |e, a| unsafe { c_ptm::oracle_obs_source_ptm_gate_mutes(e, a) != 0 },
    // SAFETY: as above.
    talk_mutes: |e, a| unsafe { c_ptm::oracle_obs_source_ptm_gate_talk_mutes(e, a) != 0 },
    // SAFETY: as above.
    snap: |m, v| unsafe { c::oracle_obs_source_volume_snap(m, v) },
    // SAFETY: as above.
    due: |a, t, d| unsafe { c::oracle_obs_source_volume_action_due(a, t, d) != 0 },
    // SAFETY: as above.
    step: |v, m| unsafe { c::oracle_obs_source_volume_step(v, m) },
    // SAFETY: as above; mix < 6.
    mixes: |a, m, x| unsafe { c::oracle_obs_source_volume_mixes(a, m, x) != 0 },
};

#[derive(Clone, Copy, Default)]
struct Source {
    enabled: bool,
    muted: bool,
    push_to_mute_enabled: bool,
    push_to_mute_pressed: bool,
    push_to_talk_enabled: bool,
    push_to_talk_pressed: bool,
    push_to_mute_delay: u64,
    push_to_mute_stop_time: u64,
    push_to_talk_delay: u64,
    push_to_talk_stop_time: u64,
    volume: f32,
    audio_ts: u64,
    audio_mixers: u32,
    actions: usize,
    action_ts: u64,
}

/// `get_source_volume()` of `obs-source.c`.
fn get_source_volume(side: &Side, source: &mut Source, os_time: u64) -> f32 {
    if source.push_to_mute_enabled && source.push_to_mute_pressed {
        source.push_to_mute_stop_time = (side.stop_time)(os_time, source.push_to_mute_delay);
    }
    if source.push_to_talk_enabled && source.push_to_talk_pressed {
        source.push_to_talk_stop_time = (side.stop_time)(os_time, source.push_to_talk_delay);
    }
    let push_to_mute_active =
        source.push_to_mute_pressed || (side.held)(os_time, source.push_to_mute_stop_time);
    let push_to_talk_active =
        source.push_to_talk_pressed || (side.held)(os_time, source.push_to_talk_stop_time);
    let muted = !source.enabled
        || source.muted
        || (side.mutes)(source.push_to_mute_enabled, push_to_mute_active)
        || (side.talk_mutes)(source.push_to_talk_enabled, push_to_talk_active);
    (side.snap)(muted, source.volume)
}

/// `apply_audio_volume()` of `obs-source.c`, without the buffers: what it
/// would do, folded into a number.
fn apply_audio_volume(side: &Side, source: &mut Source, mixers: u32) -> u64 {
    if source.actions > 0 && (side.due)(source.action_ts, source.audio_ts, TICK) {
        return 1;
    }
    let audio_ts = source.audio_ts;
    let vol = get_source_volume(side, source, audio_ts);
    match (side.step)(vol, mixers) {
        0 => 2,
        1 => 3,
        _ => {
            let mut sum = u64::from(vol.to_bits());
            for mix in 0..MIXES {
                if (side.mixes)(source.audio_mixers, mixers, mix) {
                    sum = sum
                        .wrapping_mul(7)
                        .wrapping_add(u64::try_from(mix).unwrap() + 1);
                }
            }
            sum
        }
    }
}

const VOLUMES: [f32; 16] = [
    1.0,
    0.5,
    0.0,
    -0.0,
    0.0001,
    0.00011,
    0.99995,
    0.9999,
    1.0001,
    0.25,
    2.0,
    20.0,
    0.031_622_78,
    0.707_945_8,
    f32::NAN,
    f32::INFINITY,
];
const MASKS: [u32; 8] = [0x3f, 0x01, 0x03, 0x00, 0x40, 0x3e, 0xff, u32::MAX];

/// Per tick: the source's flags, volume and masks, and the tick's mixers.
fn ticks() -> Vec<(Source, u32)> {
    (0..COUNT)
        .map(|i| {
            let n = u64::try_from(i).unwrap();
            let bits = i % 64;
            let audio_ts = 1_000_000_000_000 + n * TICK;
            let source = Source {
                enabled: bits & 1 == 0 || i % 3 == 0,
                muted: bits & 2 != 0 && i % 5 == 0,
                push_to_mute_enabled: bits & 4 != 0,
                push_to_mute_pressed: bits & 8 != 0,
                push_to_talk_enabled: bits & 16 != 0,
                push_to_talk_pressed: bits & 32 != 0,
                push_to_mute_delay: 100,
                push_to_talk_delay: 300,
                push_to_mute_stop_time: audio_ts.wrapping_add((n % 5) * 50_000_000),
                push_to_talk_stop_time: audio_ts.wrapping_sub((n % 7) * 50_000_000),
                volume: VOLUMES[i % VOLUMES.len()],
                audio_ts,
                audio_mixers: MASKS[(i / 3) % MASKS.len()],
                actions: i % 4,
                action_ts: audio_ts + (n % 3) * (TICK / 2) + TICK / 3,
            };
            (source, MASKS[(i / 7) % MASKS.len()])
        })
        .collect()
}

fn run_ticks(side: &Side, ticks: &[(Source, u32)]) -> u64 {
    let mut sum = 0_u64;
    for &(source, mixers) in ticks {
        let mut s = black_box(source);
        let r = apply_audio_volume(side, &mut s, black_box(mixers));
        sum = sum
            .wrapping_mul(31)
            .wrapping_add(r)
            .wrapping_add(s.push_to_mute_stop_time ^ s.push_to_talk_stop_time);
    }
    sum
}

fn volumes() -> Vec<(bool, f32)> {
    (0..COUNT)
        .map(|i| {
            let base = VOLUMES[i % VOLUMES.len()];
            let v = match (i / VOLUMES.len()) % 4 {
                0 => base,
                1 => f32::from_bits(base.to_bits().wrapping_add(1)),
                2 => f32::from_bits(base.to_bits().wrapping_sub(1)),
                _ => -base,
            };
            (i % 9 == 0, v)
        })
        .collect()
}

fn run_snaps(side: &Side, volumes: &[(bool, f32)]) -> u64 {
    let mut sum = 0_u64;
    for &(muted, v) in volumes {
        let r = (side.snap)(black_box(muted), black_box(v));
        sum = sum.wrapping_mul(31).wrapping_add(u64::from(r.to_bits()));
    }
    sum
}

pub(crate) fn benchmarks(criterion: &mut Criterion) {
    let ticks = ticks();
    let volumes = volumes();
    assert_eq!(
        run_ticks(&RUST, &ticks),
        run_ticks(&C, &ticks),
        "benchmark fixture differs: apply_volume"
    );
    assert_eq!(
        run_snaps(&RUST, &volumes),
        run_snaps(&C, &volumes),
        "benchmark fixture differs: snap"
    );

    let mut group = criterion.benchmark_group("source_volume/apply_volume");
    group.throughput(Throughput::Elements(COUNT as u64));
    group.bench_function("rust", |b| b.iter(|| black_box(run_ticks(&RUST, &ticks))));
    group.bench_function("c", |b| b.iter(|| black_box(run_ticks(&C, &ticks))));
    group.finish();

    let mut group = criterion.benchmark_group("source_volume/snap");
    group.throughput(Throughput::Elements(COUNT as u64));
    group.bench_function("rust", |b| b.iter(|| black_box(run_snaps(&RUST, &volumes))));
    group.bench_function("c", |b| b.iter(|| black_box(run_snaps(&C, &volumes))));
    group.finish();
}
