//! obs-source: the push-to-mute and push-to-talk gate
//! (`libobs/obs-source-ptm-gate.c.h`), which `source_muted()` runs for
//! every audio packet a source outputs, and `get_source_volume()` for its
//! stop times in every mix. Each measurement folds its results into a
//! checksum that both sides must match:
//! - `source_muted`: 1024 packets 10 ms apart through `source_muted()` of
//!   `obs-source.c` on a stand-in source (four calls a packet at most, as
//!   the C makes them), with push-to-talk and push-to-mute pressed, held
//!   inside their 300 ms and 100 ms delays and released, and every
//!   combination of the six flags;
//! - `stop_time`: 1024 stop times, from valid delays, a delay of 0 and
//!   delays and clocks at and past the saturation edges.
//!
//! Measured when it was added (x86_64 Linux, GCC, not pinned to a core, a
//! loaded host; median per 1024 calls): source_muted 4.77 us Rust, 4.73 us
//! C (1.01x); stop_time 2.11 us Rust, 2.66 us C (0.79x). Under 1 ns a
//! packet either way.

use std::hint::black_box;

use criterion::{Criterion, Throughput};
use obs_c_oracle::source_ptm_gate as c;
use obs_source::ffi::source_ptm_gate as rs;

const COUNT: usize = 1024;
const MS: u64 = 1_000_000;
const MAX_DELAY: u64 = u64::MAX / MS;

struct Side {
    stop_time: fn(u64, u64) -> u64,
    held: fn(u64, u64) -> bool,
    mutes: fn(bool, bool) -> bool,
    talk_mutes: fn(bool, bool) -> bool,
}

const RUST: Side = Side {
    stop_time: |os_time, delay| rs::obs_source_ptm_gate_stop_time(os_time, delay),
    held: |os_time, stop| rs::obs_source_ptm_gate_held(os_time, stop),
    mutes: |enabled, active| rs::obs_source_ptm_gate_mutes(enabled, active),
    talk_mutes: |enabled, active| rs::obs_source_ptm_gate_talk_mutes(enabled, active),
};

const C: Side = Side {
    // SAFETY: plain values; the C touches no memory.
    stop_time: |os_time, delay| unsafe { c::oracle_obs_source_ptm_gate_stop_time(os_time, delay) },
    // SAFETY: as above.
    held: |os_time, stop| unsafe { c::oracle_obs_source_ptm_gate_held(os_time, stop) != 0 },
    // SAFETY: as above.
    mutes: |enabled, active| unsafe { c::oracle_obs_source_ptm_gate_mutes(enabled, active) != 0 },
    // SAFETY: as above.
    talk_mutes: |enabled, active| unsafe {
        c::oracle_obs_source_ptm_gate_talk_mutes(enabled, active) != 0
    },
};

#[derive(Clone, Copy, Default)]
struct Source {
    enabled: bool,
    user_muted: bool,
    push_to_mute_enabled: bool,
    user_push_to_mute_pressed: bool,
    push_to_talk_enabled: bool,
    user_push_to_talk_pressed: bool,
    push_to_mute_delay: u64,
    push_to_mute_stop_time: u64,
    push_to_talk_delay: u64,
    push_to_talk_stop_time: u64,
}

/// `source_muted()` of `obs-source.c`.
fn source_muted(side: &Side, source: &mut Source, os_time: u64) -> bool {
    if source.push_to_mute_enabled && source.user_push_to_mute_pressed {
        source.push_to_mute_stop_time = (side.stop_time)(os_time, source.push_to_mute_delay);
    }
    if source.push_to_talk_enabled && source.user_push_to_talk_pressed {
        source.push_to_talk_stop_time = (side.stop_time)(os_time, source.push_to_talk_delay);
    }
    let push_to_mute_active =
        source.user_push_to_mute_pressed || (side.held)(os_time, source.push_to_mute_stop_time);
    let push_to_talk_active =
        source.user_push_to_talk_pressed || (side.held)(os_time, source.push_to_talk_stop_time);
    !source.enabled
        || source.user_muted
        || (side.mutes)(source.push_to_mute_enabled, push_to_mute_active)
        || (side.talk_mutes)(source.push_to_talk_enabled, push_to_talk_active)
}

/// Per packet: the flags to set before it (`None` keeps the last ones).
fn packets() -> Vec<(u64, Option<u8>)> {
    (0..COUNT)
        .map(|i| {
            let os_time = 1_000_000 * MS + u64::try_from(i).unwrap() * 10 * MS;
            // the first half: push-to-talk and push-to-mute pressed and let
            // go every 40 packets (held 200 ms, released 200 ms); the second
            // half: every combination of the six flags, 8 packets each
            let flags = if i < COUNT / 2 {
                match i % 40 {
                    0 => Some(0b11_0101),
                    10 => Some(0b01_1101),
                    20 => Some(0b01_0101),
                    _ => None,
                }
            } else if i % 8 == 0 {
                Some(u8::try_from((i / 8) % 64).unwrap())
            } else {
                None
            };
            (os_time, flags)
        })
        .collect()
}

fn run_packets(side: &Side, packets: &[(u64, Option<u8>)]) -> u64 {
    let mut source = Source {
        push_to_mute_delay: 100,
        push_to_talk_delay: 300,
        ..Source::default()
    };
    let mut sum = 0u64;
    for &(os_time, flags) in packets {
        if let Some(bits) = flags {
            source.enabled = bits & 1 != 0;
            source.user_muted = bits & 2 != 0;
            source.push_to_mute_enabled = bits & 4 != 0;
            source.user_push_to_mute_pressed = bits & 8 != 0;
            source.push_to_talk_enabled = bits & 16 != 0;
            source.user_push_to_talk_pressed = bits & 32 != 0;
        }
        let muted = source_muted(side, &mut source, black_box(os_time));
        sum = sum
            .wrapping_mul(31)
            .wrapping_add(u64::from(muted))
            .wrapping_add(source.push_to_mute_stop_time ^ source.push_to_talk_stop_time);
    }
    sum
}

fn stops() -> Vec<(u64, u64)> {
    const DELAYS: [u64; 8] = [
        0,
        1,
        300,
        2_147_483_647,
        MAX_DELAY,
        MAX_DELAY + 1,
        u64::MAX / 2,
        u64::MAX,
    ];
    (0..COUNT)
        .map(|i| {
            let n = u64::try_from(i).unwrap();
            let os_time = match i % 4 {
                0 => n * 10 * MS,
                1 => 551_615 - (n % 3),
                2 => u64::MAX - n * MS,
                _ => n.wrapping_mul(0x9e37_79b9_7f4a_7c15),
            };
            (os_time, DELAYS[(i / 4) % DELAYS.len()])
        })
        .collect()
}

fn run_stops(side: &Side, stops: &[(u64, u64)]) -> u64 {
    let mut sum = 0u64;
    for &(os_time, delay) in stops {
        let stop = (side.stop_time)(black_box(os_time), black_box(delay));
        sum = sum.wrapping_mul(31).wrapping_add(stop);
    }
    sum
}

pub(crate) fn benchmarks(criterion: &mut Criterion) {
    let packets = packets();
    let stops = stops();
    assert_eq!(
        run_packets(&RUST, &packets),
        run_packets(&C, &packets),
        "benchmark fixture differs: source_muted"
    );
    assert_eq!(
        run_stops(&RUST, &stops),
        run_stops(&C, &stops),
        "benchmark fixture differs: stop_time"
    );

    let mut group = criterion.benchmark_group("source_ptm_gate/source_muted");
    group.throughput(Throughput::Elements(COUNT as u64));
    group.bench_function("rust", |b| {
        b.iter(|| black_box(run_packets(&RUST, &packets)))
    });
    group.bench_function("c", |b| b.iter(|| black_box(run_packets(&C, &packets))));
    group.finish();

    let mut group = criterion.benchmark_group("source_ptm_gate/stop_time");
    group.throughput(Throughput::Elements(COUNT as u64));
    group.bench_function("rust", |b| b.iter(|| black_box(run_stops(&RUST, &stops))));
    group.bench_function("c", |b| b.iter(|| black_box(run_stops(&C, &stops))));
    group.finish();
}
