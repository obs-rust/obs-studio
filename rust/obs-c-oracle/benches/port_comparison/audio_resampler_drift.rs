//! obs-media-io: the resampler's clock drift loop
//! (`libobs/media-io/audio-resampler-drift.c.h`). For every packet of an
//! audio source whose compensation is on, `audio_resampler_resample()` asks
//! one step of the loop filter and its delta, the padding of its output
//! frames and the sample counters; the source sets a new error every few packets. Each side
//! runs 64 packets of 480 frames at 48 kHz with an error that ramps out of
//! the dead band and back, folded into a checksum both sides must match.
//! Nothing is allocated with `bmalloc`.

use super::pair;

use core::ffi::c_int;
use criterion::Criterion;
use obs_c_oracle::audio_resampler_drift::{
    Comp, oracle_audio_resampler_comp_count, oracle_audio_resampler_comp_delta,
    oracle_audio_resampler_comp_set_error,
    oracle_audio_resampler_comp_step, oracle_audio_resampler_output_frames,
};
use obs_media_io::audio_resampler_drift::Compensation;
use obs_media_io::ffi::audio_resampler_drift::{
    audio_resampler_comp_count, audio_resampler_comp_delta, audio_resampler_comp_set_error,
    audio_resampler_comp_step,
    audio_resampler_output_frames,
};
use std::hint::black_box;

const PACKETS: i64 = 64;
const MS: i64 = 1_000_000;

/// The error the source reports before packet `i`, every fourth packet.
fn error(i: i64) -> Option<i64> {
    (i % 4 == 0).then(|| (i - 32) * 2 * MS)
}

fn mix(acc: u64, value: u64) -> u64 {
    acc.wrapping_mul(31).wrapping_add(value)
}

fn rust_packets() -> u64 {
    let mut acc = 0_u64;
    let mut comp = Compensation::default();
    for i in 0..PACKETS {
        let mut delta: c_int = 0;
        // SAFETY: live locals to write.
        let (action, frames) = unsafe {
            if let Some(e) = black_box(error(i)) {
                audio_resampler_comp_set_error(&raw mut comp, e);
            }
            let action = audio_resampler_comp_step(&raw mut comp, 48_000, black_box(480));
            if action != 0 {
                delta = audio_resampler_comp_delta(&raw const comp, 48_000);
            }
            let frames = audio_resampler_output_frames(black_box(480), comp.engaged);
            audio_resampler_comp_count(&raw mut comp, 480, frames);
            (action, frames)
        };
        acc = mix(acc, u64::from(action) ^ u64::from(delta.cast_unsigned()));
        acc = mix(acc, u64::from(frames.cast_unsigned()) ^ comp.vc1.to_bits());
    }
    acc
}

fn c_packets() -> u64 {
    let mut acc = 0_u64;
    let mut comp = Comp::default();
    for i in 0..PACKETS {
        let mut delta: c_int = 0;
        // SAFETY: live locals to write.
        let (action, frames) = unsafe {
            if let Some(e) = black_box(error(i)) {
                oracle_audio_resampler_comp_set_error(&raw mut comp, e);
            }
            let action = oracle_audio_resampler_comp_step(&raw mut comp, 48_000, black_box(480));
            if action != 0 {
                delta = oracle_audio_resampler_comp_delta(&raw const comp, 48_000);
            }
            let frames = oracle_audio_resampler_output_frames(black_box(480), comp.engaged);
            oracle_audio_resampler_comp_count(&raw mut comp, 480, frames);
            (action, frames)
        };
        acc = mix(acc, u64::from(action) ^ u64::from(delta.cast_unsigned()));
        acc = mix(acc, u64::from(frames.cast_unsigned()) ^ comp.vc1.to_bits());
    }
    acc
}

pub fn benchmarks(criterion: &mut Criterion) {
    pair(criterion, "audio_resampler_drift/packets/64", rust_packets, c_packets);
}
