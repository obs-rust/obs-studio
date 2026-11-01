//! obs-audio-monitoring: the CoreAudio monitor's queue pacing
//! (`libobs/audio-monitoring/osx/coreaudio-pacing.c.h`). It runs for each
//! audio packet of every monitored source and each AudioQueue buffer that
//! comes back, under the monitor's mutex. Each side runs 64 steps through
//! what `on_audio_playback()`, `fill_buffer()` and `coreaudio_buffer_audio()`
//! ask: the volume gate, the end of the wait, the fill loop and the pause
//! test, with a buffer back every other step. No enqueue fails, so the one
//! intentional difference is not asked. Nothing is allocated, and the
//! results fold into a checksum both must match.

use super::pair;

use criterion::Criterion;
use obs_audio_monitoring::ffi::coreaudio_pacing::{
    coreaudio_monitor_all_back, coreaudio_monitor_buffer_size, coreaudio_monitor_can_fill,
    coreaudio_monitor_needs_volume, coreaudio_monitor_wait_over, coreaudio_monitor_wait_size,
};
use obs_c_oracle::coreaudio_pacing::{
    oracle_coreaudio_monitor_all_back, oracle_coreaudio_monitor_buffer_size,
    oracle_coreaudio_monitor_can_fill, oracle_coreaudio_monitor_needs_volume,
    oracle_coreaudio_monitor_wait_over, oracle_coreaudio_monitor_wait_size,
};
use std::hint::black_box;

const STEPS: usize = 64;
const PTR: usize = size_of::<*const ()>();
const VOLUMES: [f32; 4] = [1.0, 0.5, 1.000_05, 2.0];

/// Step `i`'s pushed bytes (a 10 ms packet, stereo at 48 kHz, give or take)
/// and volume.
fn step(i: usize) -> (usize, f32) {
    (3840 + (i % 5) * 960, VOLUMES[i % VOLUMES.len()])
}

fn mix(acc: u64, value: u64) -> u64 {
    acc.wrapping_mul(31).wrapping_add(value)
}

/// The pacing functions of one side.
struct Side {
    buffer_size: fn(u32, u32) -> usize,
    wait_size: fn(usize) -> usize,
    can_fill: fn(usize, usize) -> bool,
    needs_volume: fn(f32) -> bool,
    wait_over: fn(usize, usize) -> bool,
    all_back: fn(usize, usize) -> bool,
}

fn steps(s: &Side) -> u64 {
    let buf = (s.buffer_size)(black_box(2), black_box(48000));
    let mut wait = (s.wait_size)(buf);
    let (mut queued, mut free, mut acc) = (0_usize, 3_usize, 0_u64);
    let fill = |queued: &mut usize, free: &mut usize, acc: &mut u64| {
        while *free > 0 && (s.can_fill)(*queued, buf) {
            *queued -= buf;
            *free -= 1;
            *acc = mix(*acc, 1);
        }
    };
    for i in 0..STEPS {
        let (bytes, vol) = black_box(step(i));
        acc = mix(acc, u64::from((s.needs_volume)(vol)));
        queued += bytes;
        if (s.wait_over)(queued, wait) {
            wait = 0;
            fill(&mut queued, &mut free, &mut acc);
        }
        if i % 2 == 1 && free < 3 {
            free += 1;
            fill(&mut queued, &mut free, &mut acc);
            if (s.all_back)(free * PTR, PTR) {
                wait = (s.wait_size)(buf);
                acc = mix(acc, 2);
            }
        }
        acc = mix(acc, (queued ^ wait) as u64);
    }
    acc
}

fn rust_steps() -> u64 {
    steps(&Side {
        buffer_size: |c, r| coreaudio_monitor_buffer_size(c, r),
        wait_size: |b| coreaudio_monitor_wait_size(b),
        can_fill: |q, b| coreaudio_monitor_can_fill(q, b),
        needs_volume: |v| coreaudio_monitor_needs_volume(v),
        wait_over: |q, w| coreaudio_monitor_wait_over(q, w),
        all_back: |e, p| coreaudio_monitor_all_back(e, p),
    })
}

fn c_steps() -> u64 {
    // SAFETY (each closure): plain values; the oracle reads nothing else.
    steps(&Side {
        buffer_size: |c, r| unsafe { oracle_coreaudio_monitor_buffer_size(c, r) },
        wait_size: |b| unsafe { oracle_coreaudio_monitor_wait_size(b) },
        can_fill: |q, b| unsafe { oracle_coreaudio_monitor_can_fill(q, b) },
        needs_volume: |v| unsafe { oracle_coreaudio_monitor_needs_volume(v) },
        wait_over: |q, w| unsafe { oracle_coreaudio_monitor_wait_over(q, w) },
        all_back: |e, p| unsafe { oracle_coreaudio_monitor_all_back(e, p) },
    })
}

pub fn benchmarks(criterion: &mut Criterion) {
    pair(criterion, "coreaudio_pacing/steps/64", rust_steps, c_steps);
}
