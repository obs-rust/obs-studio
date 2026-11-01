//! The output clamp of `clamp_audio_output` (`audio-io.c`): each sample
//! becomes 0 if NaN, then is clamped to [-1, 1].
//!
//! [`clamp_samples`] runs an AVX2 copy of the same scalar loop on hosts that
//! have it (obs-rust/obs-studio#3414); both paths give the same bits.

/// Zeroes NaNs and clamps every sample to [-1, 1], in place.
pub fn clamp_samples(buf: &mut [f32]) {
    crate::platform::clamp_samples(buf);
}

/// The portable loop [`clamp_samples`] dispatches to; public for the
/// bit-identity tests.
#[doc(hidden)]
#[inline(always)]
pub fn clamp_samples_scalar(buf: &mut [f32]) {
    for val in buf {
        let mut v = *val;
        if v.is_nan() {
            v = 0.0;
        }
        *val = v.clamp(-1.0, 1.0);
    }
}
