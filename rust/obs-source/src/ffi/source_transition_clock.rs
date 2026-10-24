//! C ABI for `libobs/obs-source-transition-clock.c.h`, the clock and fit
//! decisions of the transitions, declared in
//! `libobs/obs-source-transition-clock.h` without `EXPORT`.
//!
//! `obs-source-transition.c.h` (still C) reads the transition's fields
//! under its locks and passes them as plain values: the mode, the manual
//! value, the start time and the duration to
//! [`obs_source_transition_clock_time`]; the mode and the torque to
//! [`obs_source_transition_clock_manual`] (it then calls `calc_torquef()`
//! itself for a step); the sizes to [`obs_source_transition_clock_size`],
//! [`obs_source_transition_clock_fold`] and
//! [`obs_source_transition_clock_fit`]; the two children's colour spaces to
//! [`obs_source_transition_clock_mix_spaces`]; and the two timestamps to
//! [`obs_source_transition_clock_min_ts`]. So `struct obs_source` is never
//! mirrored, no source pointer crosses, and these shims take no lock and
//! call nothing in libobs. All seven stay hidden (`rust-hidden.txt`).
//!
//! The fit writes the scale through two `float *` (the caller's `scale.x`
//! and `scale.y`), only where the C writes it: both for a scale, neither for
//! a child with no size or a scale type outside the enum.
//!
//! Intentional difference from C: a null `scale_x` or `scale_y` is not
//! written (the C writes through it). libobs passes the addresses of its
//! own `struct vec2`.

use std::os::raw::c_int;

use crate::transition_clock::{self as core, Clock, Fit, Step};

/// `enum obs_source_transition_clock_step`, as the C's enum is returned (an
/// `int` or `unsigned int`; every value fits both).
#[allow(non_camel_case_types)]
pub type obs_source_transition_clock_step = u32;
/// `OBS_SOURCE_TRANSITION_CLOCK_HOLD`.
pub const OBS_SOURCE_TRANSITION_CLOCK_HOLD: obs_source_transition_clock_step = 0;
/// `OBS_SOURCE_TRANSITION_CLOCK_SNAP`.
pub const OBS_SOURCE_TRANSITION_CLOCK_SNAP: obs_source_transition_clock_step = 1;
/// `OBS_SOURCE_TRANSITION_CLOCK_STEP`.
pub const OBS_SOURCE_TRANSITION_CLOCK_STEP: obs_source_transition_clock_step = 2;

const fn step_value(step: Step) -> obs_source_transition_clock_step {
    match step {
        Step::Hold => OBS_SOURCE_TRANSITION_CLOCK_HOLD,
        Step::Snap => OBS_SOURCE_TRANSITION_CLOCK_SNAP,
        Step::Step => OBS_SOURCE_TRANSITION_CLOCK_STEP,
    }
}

/// `float obs_source_transition_clock_time(int mode, float manual_val,
/// uint64_t start_time, uint64_t duration, uint64_t ts)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_transition_clock_time(
    mode: c_int,
    manual_val: f32,
    start_time: u64,
    duration: u64,
    ts: u64,
) -> f32 {
    core::time(
        Clock {
            mode,
            manual_val,
            start_time,
            duration,
        },
        ts,
    )
}

/// `enum obs_source_transition_clock_step
/// obs_source_transition_clock_manual(int mode, float torque)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_transition_clock_manual(
    mode: c_int,
    torque: f32,
) -> obs_source_transition_clock_step {
    step_value(core::manual(mode, torque))
}

/// `uint32_t obs_source_transition_clock_size(uint32_t set, uint32_t
/// actual)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_transition_clock_size(set: u32, actual: u32) -> u32 {
    core::size(set, actual)
}

/// `uint32_t obs_source_transition_clock_fold(uint32_t size, uint32_t
/// child)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_transition_clock_fold(size: u32, child: u32) -> u32 {
    core::fold(size, child)
}

/// `bool obs_source_transition_clock_fit(float tr_cx, float tr_cy, float
/// source_cx, float source_cy, int scale_type, float *scale_x, float
/// *scale_y)`.
///
/// # Safety
///
/// `scale_x` and `scale_y` are each null or valid for a write of one
/// aligned `float`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_transition_clock_fit(
    tr_cx: f32,
    tr_cy: f32,
    source_cx: f32,
    source_cy: f32,
    scale_type: c_int,
    scale_x: *mut f32,
    scale_y: *mut f32,
) -> bool {
    match core::fit(tr_cx, tr_cy, source_cx, source_cy, scale_type) {
        Fit::NoMatrix => false,
        Fit::Scale { x, y } => {
            if !scale_x.is_null() {
                // SAFETY: non-null and writable (caller contract).
                unsafe { scale_x.write(x) };
            }
            if !scale_y.is_null() {
                // SAFETY: as above.
                unsafe { scale_y.write(y) };
            }
            true
        }
        Fit::Unset => true,
    }
}

/// `int obs_source_transition_clock_mix_spaces(int a, int b)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_transition_clock_mix_spaces(a: c_int, b: c_int) -> c_int {
    core::mix_spaces(a, b)
}

/// `uint64_t obs_source_transition_clock_min_ts(uint64_t a, uint64_t b)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_transition_clock_min_ts(a: u64, b: u64) -> u64 {
    core::min_ts([a, b])
}
