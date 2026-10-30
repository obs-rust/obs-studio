//! Safe core of `libobs/obs-source-volume-mul.c.h`: the volume multiply
//! kernels of `obs-source.c`.
//!
//! Every audio tick, each source with audio is scaled once per mix it is in:
//! - [`mul_block`]: `multiply_output_audio()`, one volume over a whole mix.
//!   `apply_audio_volume()` passes the mix's first channel buffer and
//!   `AUDIO_OUTPUT_FRAMES * channels` samples, which works because a mix's
//!   channel buffers are consecutive in `allocate_audio_output_buffer()`'s
//!   one allocation;
//! - [`mul_data`]: `multiply_vol_data()`, one channel by a volume per frame,
//!   when `apply_audio_actions()` applies a volume, mute or push-to-talk
//!   change inside the tick.
//!
//! `obs-source.c.h` keeps the action queue and its mutex,
//! `get_source_volume()`, the `vol_data` fill, the memset and the mix and
//! channel loops. These keep no state and call nothing; nothing here can
//! panic on any input.
//!
//! Each sample is one IEEE single multiply, as the C's `*out *= vol`, so the
//! results match the C bit for bit. The one exception the C itself does not
//! pin: a NaN sample times a NaN volume carries one of the two payloads, and
//! which one depends on how the code is compiled (the multiply commutes; the
//! C at `-O0` keeps the sample's, vectorized at `-O2` the volume's).
//!
//! The loops run over slices, which cannot overlap, so LLVM vectorizes them
//! on every target without `std::arch` (#3007). GCC at `-O2`, as libobs
//! ships, leaves the C loops scalar, because the C's pointers might
//! overlap. The loop form of [`mul_data`] depends on the target, because
//! LLVM unrolls the two forms differently there: 4-sample groups on x86_64
//! (the zip loop was 17 % slower than GCC's vectorized C at `-O3`, the
//! grouped one 3 %), a plain zip loop on aarch64 (the grouped one was 49 %
//! slower than clang's C, the zip loop level with it); the choice is
//! `platform::GROUPED_AUDIO_LOOPS`.
//!
//! No intentional differences.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::as_conversions,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

/// `obs_source_volume_mul_block()`: `out[i] *= vol` for every sample.
pub fn mul_block(out: &mut [f32], vol: f32) {
    for sample in out {
        *sample *= vol;
    }
}

/// `obs_source_volume_mul_data()`: `out[i] *= vol[i]` for every sample that
/// has a volume. The C multiplies `count` samples and reads `count`
/// volumes; here that count is the shorter of the two slices, and the
/// samples after it stay as they are. [`mul_data_grouped`] on x86_64,
/// [`mul_data_zipped`] elsewhere (see the module docs).
pub fn mul_data(out: &mut [f32], vol: &[f32]) {
    if crate::platform::GROUPED_AUDIO_LOOPS {
        mul_data_grouped(out, vol);
    } else {
        mul_data_zipped(out, vol);
    }
}

/// [`mul_data`] as 4-sample groups, then the rest. Same result as
/// [`mul_data_zipped`] on every input; public only for the tests, which run
/// both forms on every host.
#[doc(hidden)]
pub fn mul_data_grouped(out: &mut [f32], vol: &[f32]) {
    let n = out.len().min(vol.len());
    let (Some(out), Some(vol)) = (out.get_mut(..n), vol.get(..n)) else {
        return;
    };
    let (out_groups, out_rest) = out.as_chunks_mut::<4>();
    let (vol_groups, vol_rest) = vol.as_chunks::<4>();
    for (samples, volumes) in out_groups.iter_mut().zip(vol_groups) {
        for (sample, v) in samples.iter_mut().zip(volumes) {
            *sample *= *v;
        }
    }
    for (sample, v) in out_rest.iter_mut().zip(vol_rest) {
        *sample *= *v;
    }
}

/// [`mul_data`] as a plain zip loop. Same result as [`mul_data_grouped`] on
/// every input; public only for the tests.
#[doc(hidden)]
pub fn mul_data_zipped(out: &mut [f32], vol: &[f32]) {
    for (sample, v) in out.iter_mut().zip(vol) {
        *sample *= *v;
    }
}
