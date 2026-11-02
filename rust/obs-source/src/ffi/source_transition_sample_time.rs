//! C ABI for `libobs/obs-source-transition-sample-time.c.h`, the time of
//! each audio sample a transition mixes, declared in
//! `libobs/obs-source-transition-sample-time.h` without `EXPORT`.
//!
//! `mix_child()` in `obs-source-transition.c.h` (still C) reads the
//! transition's mode, manual value, start time and duration for every
//! sample and passes them, with the block's timestamp, the sample rate and
//! the sample index, as plain values to
//! [`obs_source_transition_sample_time`]. So `struct obs_source` is never
//! mirrored, no source pointer crosses, and the shim takes no lock and
//! calls nothing in libobs. It stays hidden (`rust-hidden.d`).
//!
//! Intentional difference from C: a sample rate of 0 gives every sample
//! the block's own time, where the C divides by zero.

use std::os::raw::c_int;

use crate::transition_clock::Clock;
use crate::transition_sample_time as core;

/// `float obs_source_transition_sample_time(int mode, float manual_val,
/// uint64_t start_time, uint64_t duration, uint64_t ts, size_t
/// sample_rate, size_t sample)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_transition_sample_time(
    mode: c_int,
    manual_val: f32,
    start_time: u64,
    duration: u64,
    ts: u64,
    sample_rate: usize,
    sample: usize,
) -> f32 {
    core::sample_time(
        Clock {
            mode,
            manual_val,
            start_time,
            duration,
        },
        ts,
        sample_rate,
        sample,
    )
}
