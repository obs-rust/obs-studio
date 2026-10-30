//! #3003: the volume meter's peak kernels against the C, the `volmeter`
//! category of #3009. `get_sample_peak()` and `get_true_peak()` of
//! `libobs/obs-audio-controls.c.h` are SSE in the C (simde on non-x86) and
//! scalar `[f32; 4]` lanes in `obs_audio_controls::peak`. Each call is one
//! channel of one audio tick: 1024 frames of a tone with noise, after the
//! last four samples of the tick before. The meter runs both per channel of
//! every metered source, every tick.
//!
//! The RMS (`volmeter_process_magnitude`) is not here: #3009 keeps it scalar
//! (a float sum in the C's order), so it is no SIMD candidate.

use std::hint::black_box;

use criterion::Criterion;
use obs_audio_controls::peak;
use obs_c_oracle::audio_controls as c;

const FRAMES: usize = 1024;

/// One channel of one tick, 16-byte aligned for the C's `_mm_load_ps`.
#[repr(C, align(16))]
struct Tick([f32; FRAMES]);

/// A 997 Hz tone at 48 kHz near full scale, with deterministic noise, so
/// both the samples and the oversampled points between them set peaks.
fn tick() -> Box<Tick> {
    let mut state = 0x2545_f491_u32;
    let mut samples = Box::new(Tick([0.0; FRAMES]));
    for (i, sample) in samples.0.iter_mut().enumerate() {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let noise = (state >> 8) as f32 / (1 << 24) as f32 - 0.5;
        let phase = 2.0 * std::f32::consts::PI * 997.0 * i as f32 / 48_000.0;
        *sample = 0.9 * phase.sin() + 0.05 * noise;
    }
    samples
}

pub fn benchmarks(criterion: &mut Criterion) {
    let samples = tick();
    let previous = [0.12_f32, -0.48, 0.81, -0.33];
    // SAFETY (all C calls): `previous` holds 4 floats and `samples` FRAMES
    // floats, 16-byte aligned.
    super::pair(
        criterion,
        "volmeter/sample_peak/1024",
        || peak::sample_peak(black_box(previous), black_box(&samples.0), FRAMES).to_bits(),
        || unsafe {
            c::oracle_get_sample_peak(previous.as_ptr(), black_box(samples.0.as_ptr()), FRAMES)
                .to_bits()
        },
    );
    super::pair(
        criterion,
        "volmeter/true_peak/1024",
        || peak::true_peak(black_box(previous), black_box(&samples.0), FRAMES).to_bits(),
        || unsafe {
            c::oracle_get_true_peak(previous.as_ptr(), black_box(samples.0.as_ptr()), FRAMES)
                .to_bits()
        },
    );
}
