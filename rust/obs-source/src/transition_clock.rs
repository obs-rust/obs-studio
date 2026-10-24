//! Safe core of `libobs/obs-source-transition-clock.c.h`: the clock and fit
//! decisions of the transitions in `libobs/obs-source-transition.c.h`.
//!
//! A transition draws its two children at a position from 0 to 1 and fits
//! each child into its own size. These functions make those decisions:
//! - [`time`]: `calc_time()`, the position at a timestamp. The manual value
//!   in manual mode; otherwise 0 up to the start, 1 from the end on (and for
//!   a duration of 0), and [`ratio`] of the elapsed time and the duration in
//!   between;
//! - [`manual`]: the manual step of `obs_transition_tick()`: [`Step::Hold`]
//!   outside manual mode, [`Step::Snap`] to the target for a torque of 0,
//!   [`Step::Step`] otherwise (the C then calls `calc_torquef()`);
//! - [`size`] and [`fold`]: `get_cx()` and `get_cy()`, and the size fold of
//!   `recalculate_transition_size()`;
//! - [`fit`]: the scale of `recalculate_transition_matrix()` for each
//!   [`ScaleType`];
//! - [`mix_spaces`]: `mix_spaces()`, the colour space two children are
//!   rendered in;
//! - [`min_ts`]: `transition_calc_min_ts()`, the earliest audio timestamp
//!   that is not 0.
//!
//! `obs-source-transition.c.h` keeps the locks, the field reads and writes,
//! the child size and timestamp reads, `calc_torquef()`, the offsets passed
//! to `add_alignment()` and their `(int)` casts, `add_alignment()`, the
//! matrix build, `get_sample_time()`, the texrenders and the swap handover
//! (#556). Nothing here keeps state, takes a lock or calls libobs.
//!
//! The C divides the clock in `long double`, whose width differs by
//! platform; [`ratio`] gives the x87 result of x86_64 for every `u64`
//! clock, from integers where `f64` would round differently: see
//! [`ratio`].

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

/// `OBS_TRANSITION_MODE_MANUAL` (`enum obs_transition_mode`).
pub const MODE_MANUAL: i32 = 1;

/// `OBS_TRANSITION_SCALE_MAX_ONLY` (`enum obs_transition_scale_type`).
pub const SCALE_MAX_ONLY: i32 = 0;
/// `OBS_TRANSITION_SCALE_ASPECT`.
pub const SCALE_ASPECT: i32 = 1;
/// `OBS_TRANSITION_SCALE_STRETCH`.
pub const SCALE_STRETCH: i32 = 2;

/// `GS_CS_SRGB` (`enum gs_color_space`).
pub const CS_SRGB: i32 = 0;
/// `GS_CS_SRGB_16F`.
pub const CS_SRGB_16F: i32 = 1;
/// `GS_CS_709_EXTENDED`.
pub const CS_709_EXTENDED: i32 = 2;
/// `GS_CS_709_SCRGB`.
pub const CS_709_SCRGB: i32 = 3;

/// The fields of the transition [`time`] reads.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Clock {
    /// `transition_mode`, an `enum obs_transition_mode`.
    pub mode: i32,
    /// `transition_manual_val`.
    pub manual_val: f32,
    /// `transition_start_time`, in ns.
    pub start_time: u64,
    /// `transition_duration`, in ns.
    pub duration: u64,
}

/// `obs_source_transition_clock_time()` (`calc_time()`): the position of
/// the transition at `ts`.
pub fn time(clock: Clock, ts: u64) -> f32 {
    let _ = (clock, ts);
    todo!()
}

/// The ratio `elapsed / duration` as an `f32`, for `0 < elapsed <
/// duration` (what [`time`] divides), rounded as the C's
/// `(float)((long double)ts / (long double)end)` is on x86_64 (x87): to a
/// 64-bit significand, then to 24 bits, both to nearest even.
pub fn ratio(elapsed: u64, duration: u64) -> f32 {
    let _ = (elapsed, duration);
    todo!()
}

/// `enum obs_source_transition_clock_step`: what `obs_transition_tick()`
/// does with `transition_manual_val`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Step {
    /// `OBS_SOURCE_TRANSITION_CLOCK_HOLD`: not manual, the value is left as
    /// it is.
    Hold,
    /// `OBS_SOURCE_TRANSITION_CLOCK_SNAP`: the value becomes the target.
    Snap,
    /// `OBS_SOURCE_TRANSITION_CLOCK_STEP`: the value becomes
    /// `calc_torquef(value, target, torque, clamp, t)`, which the C calls.
    Step,
}

/// `obs_source_transition_clock_manual()`: the manual step for `mode` and
/// the manual `torque`.
pub fn manual(mode: i32, torque: f32) -> Step {
    let _ = (mode, torque);
    todo!()
}

/// `obs_source_transition_clock_size()` (`get_cx()`, `get_cy()`): the size
/// `set` (`transition_cx`), or the `actual` one when none is set.
pub fn size(set: u32, actual: u32) -> u32 {
    let _ = (set, actual);
    todo!()
}

/// `obs_source_transition_clock_fold()`: one step of the size fold, the
/// larger of `size` and a `child`'s.
pub fn fold(size: u32, child: u32) -> u32 {
    let _ = (size, child);
    todo!()
}

/// `enum obs_transition_scale_type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScaleType {
    /// [`SCALE_MAX_ONLY`]: a child that fits keeps its size; a larger one is
    /// fitted as [`ScaleType::Aspect`].
    MaxOnly,
    /// [`SCALE_ASPECT`]: the smaller ratio of the two sides, on both.
    Aspect,
    /// [`SCALE_STRETCH`]: each side's ratio.
    Stretch,
}

impl ScaleType {
    /// The scale type of an `enum obs_transition_scale_type` value; `None`
    /// for any other value.
    pub fn from_c(scale_type: i32) -> Option<Self> {
        let _ = scale_type;
        todo!()
    }
}

/// What [`fit`] decides for a child.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Fit {
    /// The child has a width or height of 0: no matrix, nothing written.
    NoMatrix,
    /// A matrix with this scale.
    Scale {
        /// The horizontal scale.
        x: f32,
        /// The vertical scale.
        y: f32,
    },
    /// A scale type outside the enum: a matrix, but the scale is not
    /// written.
    Unset,
}

/// `obs_source_transition_clock_fit()`: the scale of a child of
/// `source_cx` by `source_cy` in a transition of `tr_cx` by `tr_cy` with
/// `scale_type`, as the C's float compares and divisions give it.
pub fn fit(tr_cx: f32, tr_cy: f32, source_cx: f32, source_cy: f32, scale_type: i32) -> Fit {
    let _ = (tr_cx, tr_cy, source_cx, source_cy, scale_type);
    todo!()
}

/// `obs_source_transition_clock_mix_spaces()` (`mix_spaces()`):
/// [`CS_709_EXTENDED`] if either space is it or [`CS_709_SCRGB`], else
/// [`CS_SRGB_16F`] if either is it, else [`CS_SRGB`].
pub fn mix_spaces(a: i32, b: i32) -> i32 {
    let _ = (a, b);
    todo!()
}

/// `obs_source_transition_clock_min_ts()` (`transition_calc_min_ts()`):
/// the earliest of the timestamps that is not 0, or 0 when all are. A child
/// with no timestamp (none, or its audio pending) is 0.
pub fn min_ts(timestamps: [u64; 2]) -> u64 {
    let _ = timestamps;
    todo!()
}
