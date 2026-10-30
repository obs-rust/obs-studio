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
//! The loops are plain iterator loops on slices, which cannot overlap, so
//! LLVM vectorizes them on every target without `std::arch` (#3007). GCC
//! leaves the C loops scalar, because the C's pointers might overlap.
//!
//! No intentional differences.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::as_conversions,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable
)]

/// `obs_source_volume_mul_block()`: `out[i] *= vol` for every sample.
pub fn mul_block(out: &mut [f32], vol: f32) {
    let _ = (out, vol);
    todo!()
}

/// `obs_source_volume_mul_data()`: `out[i] *= vol[i]` for every sample that
/// has a volume. The C multiplies `count` samples and reads `count`
/// volumes; here that count is the shorter of the two slices, and the
/// samples after it stay as they are.
pub fn mul_data(out: &mut [f32], vol: &[f32]) {
    let _ = (out, vol);
    todo!()
}
