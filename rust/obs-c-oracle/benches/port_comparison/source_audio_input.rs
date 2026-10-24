//! obs-source: the audio input steps (`libobs/obs-source-audio-input.c.h`),
//! which `process_audio()` runs for every audio packet of every source.
//! Each measurement folds its results into a checksum that both sides must
//! match:
//! - `decisions`: 1024 packets through the discontinuity, the reset
//!   condition and the two gates, as `process_audio()` calls them, with the
//!   timestamps continuing, jittered and jumping and the formats and flags
//!   changing now and then;
//! - `sine_law`: the balance of a stereo packet of 480 frames (10 ms at
//!   48 kHz) at 0.25, the default law;
//! - `downmix_2` and `downmix_8`: Force Mono of 480 frames of stereo and of
//!   7.1.
//!
//! Measured when it was added (x86_64 Linux, GCC, not pinned to a core, on
//! a loaded machine; median): decisions 9.37 us Rust, 13.49 us C per 1024
//! packets (0.70x); sine_law 1.06 us Rust, 1.70 us C per 480 frames
//! (0.62x); downmix_2 1.13 us Rust, 2.20 us C (0.51x); downmix_8 3.30 us
//! Rust, 6.45 us C (0.51x). The Rust loops are vectorized; the outputs are
//! the same bits.

use std::ffi::c_int;
use std::hint::black_box;

use criterion::{Criterion, Throughput};
use obs_c_oracle::source_audio_input::{self as c, OracleAudioInputFormat};
use obs_source::ffi::source_audio_input::{self as rs, obs_source_audio_input_format};

/// A format: `enum audio_format`, the rate, `enum speaker_layout`.
type Fmt = (i32, u32, i32);

const COUNT: usize = 1024;
const FRAMES: usize = 480;
const MSEC: u64 = 1_000_000;

#[derive(Clone, Copy)]
struct Packet {
    has_resampler: bool,
    next: u64,
    timestamp: u64,
    current: Fmt,
    packet: Fmt,
    compensation: bool,
    mono_output: bool,
    balance: f32,
    flags: u32,
}

fn packets() -> Vec<Packet> {
    let mut next = 3_600_000 * MSEC;
    (0..COUNT)
        .map(|i| {
            // continuing, 1 ms late, 100 ms late (a restart) or early
            let timestamp = match i % 8 {
                0..=4 => next,
                5 => next + MSEC,
                6 => next + 100 * MSEC,
                _ => next - 80 * MSEC,
            };
            next = timestamp + 10 * MSEC;
            let rate = if i % 64 == 63 { 44100 } else { 48000 };
            Packet {
                has_resampler: i % 3 != 0,
                next,
                timestamp,
                current: (8, 48000, 2),
                packet: (8, rate, if i % 128 == 127 { 6 } else { 2 }),
                compensation: i % 256 == 255,
                mono_output: i % 16 == 15,
                balance: [0.5f32, 0.25, 0.5, 0.75, 0.5, 0.0, 0.5, 1.0][i % 8],
                flags: if i % 4 == 3 { 2 } else { 0 },
            }
        })
        .collect()
}

struct Side {
    discontinuous: fn(bool, u64, u64) -> bool,
    reset_needed: fn(Fmt, Fmt, bool, bool, bool) -> bool,
    balance_needed: fn(bool, c_int, f32) -> bool,
    mono_needed: fn(bool, u32) -> bool,
    balance: fn(*mut *mut f32, u32, f32, c_int),
    downmix: fn(*mut *mut f32, usize, u32),
}

fn r_fmt(f: Fmt) -> obs_source_audio_input_format {
    obs_source_audio_input_format {
        format: f.0,
        samples_per_sec: f.1,
        speakers: f.2,
    }
}

fn c_fmt(f: Fmt) -> OracleAudioInputFormat {
    OracleAudioInputFormat {
        format: f.0,
        samples_per_sec: f.1,
        speakers: f.2,
    }
}

const RUST: Side = Side {
    discontinuous: |has, next, ts| rs::obs_source_audio_input_discontinuous(has, next, ts),
    reset_needed: |current, packet, last, now, disc| {
        rs::obs_source_audio_input_reset_needed(r_fmt(current), r_fmt(packet), last, now, disc)
    },
    balance_needed: |mono, speakers, balance| {
        rs::obs_source_audio_input_balance_needed(mono, speakers, balance)
    },
    mono_needed: |mono, flags| rs::obs_source_audio_input_mono_needed(mono, flags),
    // SAFETY: the bench's planes hold FRAMES floats each.
    balance: |data, frames, balance, law| unsafe {
        rs::obs_source_audio_input_balance(data, frames, balance, law)
    },
    // SAFETY: as above.
    downmix: |data, channels, frames| unsafe {
        rs::obs_source_audio_input_downmix(data, channels, frames)
    },
};

const C: Side = Side {
    // SAFETY: plain values.
    discontinuous: |has, next, ts| unsafe {
        c::oracle_obs_source_audio_input_discontinuous(has, next, ts)
    },
    // SAFETY: plain values.
    reset_needed: |current, packet, last, now, disc| unsafe {
        c::oracle_obs_source_audio_input_reset_needed(
            c_fmt(current),
            c_fmt(packet),
            last,
            now,
            disc,
        )
    },
    // SAFETY: plain values.
    balance_needed: |mono, speakers, balance| unsafe {
        c::oracle_obs_source_audio_input_balance_needed(mono, speakers, balance)
    },
    // SAFETY: plain values.
    mono_needed: |mono, flags| unsafe { c::oracle_obs_source_audio_input_mono_needed(mono, flags) },
    // SAFETY: the bench's planes hold FRAMES floats each.
    balance: |data, frames, balance, law| unsafe {
        c::oracle_obs_source_audio_input_balance(data, frames, balance, law)
    },
    // SAFETY: as above.
    downmix: |data, channels, frames| unsafe {
        c::oracle_obs_source_audio_input_downmix(data, channels, frames)
    },
};

fn decisions(side: &Side, packets: &[Packet]) -> u64 {
    let mut sum = 0u64;
    let mut last_compensation = false;
    for p in packets {
        let p = black_box(*p);
        let disc = (side.discontinuous)(p.has_resampler, p.next, p.timestamp);
        let reset =
            (side.reset_needed)(p.current, p.packet, last_compensation, p.compensation, disc);
        if reset {
            last_compensation = p.compensation;
        }
        let balance = (side.balance_needed)(p.mono_output, p.packet.2, p.balance);
        let mono = (side.mono_needed)(p.mono_output, p.flags);
        let bits = u64::from(disc)
            | u64::from(reset) << 1
            | u64::from(balance) << 2
            | u64::from(mono) << 3;
        sum = sum.wrapping_mul(31).wrapping_add(bits);
    }
    sum
}

fn input(channels: usize) -> Vec<Vec<f32>> {
    (0..channels)
        .map(|c| {
            (0..FRAMES)
                .map(|f| {
                    let x = f32::from(u16::try_from((f * 7 + c * 13) % 1000).unwrap());
                    x / 1000.0 - 0.5
                })
                .collect()
        })
        .collect()
}

fn checksum(planes: &[Vec<f32>]) -> u64 {
    planes.iter().flatten().fold(0u64, |sum, s| {
        sum.wrapping_mul(31).wrapping_add(u64::from(s.to_bits()))
    })
}

fn sine_law(side: &Side, planes: &mut [Vec<f32>], fresh: &[Vec<f32>]) -> u64 {
    for (plane, fresh) in planes.iter_mut().zip(fresh) {
        plane.copy_from_slice(fresh);
    }
    let mut data: Vec<*mut f32> = planes.iter_mut().map(|p| p.as_mut_ptr()).collect();
    (side.balance)(
        data.as_mut_ptr(),
        u32::try_from(FRAMES).unwrap(),
        black_box(0.25),
        0,
    );
    checksum(planes)
}

fn downmix(side: &Side, planes: &mut [Vec<f32>], fresh: &[Vec<f32>]) -> u64 {
    for (plane, fresh) in planes.iter_mut().zip(fresh) {
        plane.copy_from_slice(fresh);
    }
    let mut data: Vec<*mut f32> = planes.iter_mut().map(|p| p.as_mut_ptr()).collect();
    (side.downmix)(
        data.as_mut_ptr(),
        planes.len(),
        u32::try_from(FRAMES).unwrap(),
    );
    checksum(planes)
}

pub(crate) fn benchmarks(criterion: &mut Criterion) {
    let packets = packets();
    assert_eq!(
        decisions(&RUST, &packets),
        decisions(&C, &packets),
        "benchmark fixture differs: decisions"
    );
    let stereo = input(2);
    let surround = input(8);
    let mut a = stereo.clone();
    let mut b = stereo.clone();
    assert_eq!(
        sine_law(&RUST, &mut a, &stereo),
        sine_law(&C, &mut b, &stereo),
        "benchmark fixture differs: sine_law"
    );
    assert_eq!(
        downmix(&RUST, &mut a, &stereo),
        downmix(&C, &mut b, &stereo),
        "benchmark fixture differs: downmix_2"
    );
    let mut a8 = surround.clone();
    let mut b8 = surround.clone();
    assert_eq!(
        downmix(&RUST, &mut a8, &surround),
        downmix(&C, &mut b8, &surround),
        "benchmark fixture differs: downmix_8"
    );

    let mut group = criterion.benchmark_group("source_audio_input/decisions");
    group.throughput(Throughput::Elements(COUNT as u64));
    group.bench_function("rust", |bench| {
        bench.iter(|| black_box(decisions(&RUST, &packets)))
    });
    group.bench_function("c", |bench| {
        bench.iter(|| black_box(decisions(&C, &packets)))
    });
    group.finish();

    let mut group = criterion.benchmark_group("source_audio_input/sine_law");
    group.throughput(Throughput::Elements(FRAMES as u64));
    group.bench_function("rust", |bench| {
        bench.iter(|| black_box(sine_law(&RUST, &mut a, &stereo)))
    });
    group.bench_function("c", |bench| {
        bench.iter(|| black_box(sine_law(&C, &mut b, &stereo)))
    });
    group.finish();

    let mut group = criterion.benchmark_group("source_audio_input/downmix_2");
    group.throughput(Throughput::Elements(FRAMES as u64));
    group.bench_function("rust", |bench| {
        bench.iter(|| black_box(downmix(&RUST, &mut a, &stereo)))
    });
    group.bench_function("c", |bench| {
        bench.iter(|| black_box(downmix(&C, &mut b, &stereo)))
    });
    group.finish();

    let mut group = criterion.benchmark_group("source_audio_input/downmix_8");
    group.throughput(Throughput::Elements(FRAMES as u64));
    group.bench_function("rust", |bench| {
        bench.iter(|| black_box(downmix(&RUST, &mut a8, &surround)))
    });
    group.bench_function("c", |bench| {
        bench.iter(|| black_box(downmix(&C, &mut b8, &surround)))
    });
    group.finish();
}
