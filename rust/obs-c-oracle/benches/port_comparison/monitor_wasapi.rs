//! obs-audio-monitoring: the WASAPI monitor's speaker layout and video stall
//! tracker (`libobs/audio-monitoring/monitor-speaker-layout.c.h` and
//! `monitor-video-sync.c.h`). The monitor asks the layout once per device
//! (re)open and the tracker for every packet of a monitored source with
//! video. Each side runs the layout for 64 masks and channel counts (the
//! listed masks, others and counts with no layout) and the tracker over 64
//! packets whose video starts, stands still and moves on, folded into a
//! checksum both sides must match. Nothing is allocated with `bmalloc`.

use super::pair;

use criterion::Criterion;
use obs_audio_monitoring::ffi::monitor_delay::audio_monitor_track_video_ts;
use obs_audio_monitoring::ffi::speaker_layout::audio_monitor_speaker_layout;
use obs_c_oracle::monitor_delay::oracle_audio_monitor_track_video_ts;
use obs_c_oracle::speaker_layout::oracle_audio_monitor_speaker_layout;
use std::hint::black_box;

const CASES: u32 = 64;
const MASKS: [u32; 8] = [0x0B, 0x107, 0x10F, 0x3F, 0xFF, 0x0, 0x33, 0x60F];
/// A 1024-frame packet at 48 kHz.
const PACKET_NS: u64 = 21_333_333;

/// Case `i`: a mask and channel count, and the packet's video timestamp
/// (a new one every 8 packets, 0 every 29th).
fn case(i: u32) -> (u32, u32, u64) {
    let mask = MASKS.get(usize::try_from(i % 8).unwrap_or(0)).copied().unwrap_or(0);
    let channels = i % 11;
    let video = if i.is_multiple_of(29) {
        0
    } else {
        (1 << 40) + u64::from(i / 8) * 33_000_000
    };
    (mask, channels, video)
}

fn mix(acc: u64, value: u64) -> u64 {
    acc.wrapping_mul(31).wrapping_add(value)
}

fn rust_cases() -> u64 {
    let (mut acc, mut prev, mut count) = (0_u64, 0_u64, 0_u64);
    for i in 0..CASES {
        let (mask, channels, video) = black_box(case(i));
        let layout = audio_monitor_speaker_layout(mask, channels);
        // SAFETY: live locals to write.
        unsafe { audio_monitor_track_video_ts(&raw mut prev, &raw mut count, video, PACKET_NS) };
        acc = mix(acc, u64::from(layout.cast_unsigned()));
        acc = mix(acc, prev ^ count);
    }
    acc
}

fn c_cases() -> u64 {
    let (mut acc, mut prev, mut count) = (0_u64, 0_u64, 0_u64);
    for i in 0..CASES {
        let (mask, channels, video) = black_box(case(i));
        // SAFETY: plain integers.
        let layout = unsafe { oracle_audio_monitor_speaker_layout(mask, channels) };
        // SAFETY: live locals to write.
        unsafe { oracle_audio_monitor_track_video_ts(&raw mut prev, &raw mut count, video, PACKET_NS) };
        acc = mix(acc, u64::from(layout.cast_unsigned()));
        acc = mix(acc, prev ^ count);
    }
    acc
}

pub fn benchmarks(criterion: &mut Criterion) {
    pair(criterion, "monitor_wasapi/cases/64", rust_cases, c_cases);
}
