//! obs-audio-monitoring: the WASAPI monitor's delay decisions
//! (`libobs/audio-monitoring/monitor-delay.c.h`). For every packet of a
//! monitored source with video, `process_audio_delay()` asks the stale cut
//! once and the front packet's steps at least once: the difference, the
//! gate's first part, its second (the stall count) only when the first is
//! false, and the verdict. Each side runs 64 packets of a 20 ms cadence
//! around the video, rushing, dragging, in step and stalled, folded into a
//! checksum both sides must match. Nothing is allocated with `bmalloc`.

use super::pair;

use criterion::Criterion;
use obs_audio_monitoring::ffi::monitor_delay::{
    audio_monitor_delay_diff, audio_monitor_delay_judge_diff, audio_monitor_delay_stale,
    audio_monitor_delay_stalled, audio_monitor_delay_unsynced,
};
use obs_c_oracle::monitor_delay::{
    oracle_audio_monitor_delay_diff, oracle_audio_monitor_delay_judge_diff,
    oracle_audio_monitor_delay_stale, oracle_audio_monitor_delay_stalled,
    oracle_audio_monitor_delay_unsynced,
};
use std::hint::black_box;

const PACKETS: u64 = 64;
const MS: u64 = 1_000_000;
const VIDEO: u64 = 1 << 40;

/// Packet `i` of the 64: its arrival, timestamp, padding, video timestamp
/// and stall count.
fn packet(i: u64) -> (u64, u64, u64, u64, u64) {
    let clock = VIDEO + i * 20 * MS + (i % 7) * 400 * MS;
    let ts = VIDEO + (i * 37 % 400) * MS;
    let pad = (i % 5) * 10 * MS;
    let video = if i.is_multiple_of(13) { 0 } else { VIDEO + 200 * MS };
    let stall = (i % 3) * 60 * MS;
    (clock, ts, pad, video, stall)
}

fn mix(acc: u64, value: u64) -> u64 {
    acc.wrapping_mul(31).wrapping_add(value)
}

fn rust_packets() -> u64 {
    let (mut acc, mut last) = (0_u64, 0_u64);
    for i in 0..PACKETS {
        let (clock, ts, pad, video, stall) = black_box(packet(i));
        let mut front_ts = 0_u64;
        // SAFETY: live locals to write.
        let (cut, verdict, diff) = unsafe {
            let cut = audio_monitor_delay_stale(&raw mut last, clock);
            let diff = audio_monitor_delay_diff(ts, pad, video, &raw mut front_ts);
            let bad = audio_monitor_delay_unsynced(video, diff) || audio_monitor_delay_stalled(stall);
            (cut, audio_monitor_delay_judge_diff(bad, diff), diff)
        };
        acc = mix(acc, u64::from(cut) ^ u64::from(verdict));
        acc = mix(acc, front_ts ^ diff.cast_unsigned());
    }
    acc
}

fn c_packets() -> u64 {
    let (mut acc, mut last) = (0_u64, 0_u64);
    for i in 0..PACKETS {
        let (clock, ts, pad, video, stall) = black_box(packet(i));
        let mut front_ts = 0_u64;
        // SAFETY: live locals to write; the inputs keep the C's difference
        // defined.
        let (cut, verdict, diff) = unsafe {
            let cut = oracle_audio_monitor_delay_stale(&raw mut last, clock);
            let diff = oracle_audio_monitor_delay_diff(ts, pad, video, &raw mut front_ts);
            let bad =
                oracle_audio_monitor_delay_unsynced(video, diff) || oracle_audio_monitor_delay_stalled(stall);
            (cut, oracle_audio_monitor_delay_judge_diff(bad, diff), diff)
        };
        acc = mix(acc, u64::from(cut) ^ u64::from(verdict));
        acc = mix(acc, front_ts ^ diff.cast_unsigned());
    }
    acc
}

pub fn benchmarks(criterion: &mut Criterion) {
    pair(criterion, "monitor_delay/packets/64", rust_packets, c_packets);
}
