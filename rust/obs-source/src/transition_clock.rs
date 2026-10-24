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
    if clock.mode == MODE_MANUAL {
        return clock.manual_val;
    }
    let Some(elapsed) = ts
        .checked_sub(clock.start_time)
        .filter(|&elapsed| elapsed != 0)
    else {
        // ts <= transition_start_time
        return 0.0;
    };
    if elapsed >= clock.duration || clock.duration == 0 {
        return 1.0;
    }
    ratio(elapsed, clock.duration)
}

/// The ratio `elapsed / duration` as an `f32`, for `0 < elapsed <
/// duration` (what [`time`] divides), rounded as the C's
/// `(float)((long double)ts / (long double)end)` is on x86_64 (x87): to a
/// 64-bit significand, then to 24 bits, both to nearest even.
///
/// Below 2^53 both operands are exact in an `f64`, and so is every `f32`
/// rounding tie, so the `f64` quotient is on the same side of every tie as
/// the exact quotient, and as the x87's 64-bit quotient, unless it lands on
/// a tie itself. Only then can the two roundings differ: a plain
/// `(float)((double)ts / (double)end)`, the C where `long double` is a
/// `double` (MSVC, Apple silicon), rounds that tie to even, where the
/// x87's quotient may lie off it. That takes a duration over 2^29 ns
/// (0.5 s) and a ratio within 2^-54 of a tie. So the `f64` quotient is
/// used when it is not on a tie (all but about 1 in 2^29 ratios), and
/// [`x87_ratio`] otherwise and from 2^53 on.
#[allow(clippy::as_conversions)]
pub fn ratio(elapsed: u64, duration: u64) -> f32 {
    const EXACT_IN_F64: u64 = 1 << 53;
    if elapsed < EXACT_IN_F64 && duration < EXACT_IN_F64 {
        // u64 below 2^53 to f64 is exact, as in the C. There is no From.
        let quotient = elapsed as f64 / duration as f64;
        if !on_f32_tie(quotient) {
            // f64 to f32 rounds to nearest even, as the C's (float) does.
            return quotient as f32;
        }
    }
    x87_ratio(elapsed, duration)
}

/// Whether `quotient` (an `f64` in `(0, 1]`, so within the normal range of
/// `f32`) lies exactly half way between two `f32`s: its 29 low mantissa
/// bits, the ones `f32` drops, are `1` and 28 zeros.
fn on_f32_tie(quotient: f64) -> bool {
    const DROPPED: u64 = (1 << 29) - 1;
    const HALF: u64 = 1 << 28;
    quotient.to_bits() & DROPPED == HALF
}

/// `(float)((long double)elapsed / (long double)duration)` with x87
/// `long double`, from integers: both `u64` are exact in the 64-bit
/// significand; the quotient is rounded to 64 significant bits, to nearest
/// even, and that value to 24 bits, to nearest even. For
/// `0 < elapsed < duration`; anything else gives 0 or 1 (never reached).
pub fn x87_ratio(elapsed: u64, duration: u64) -> f32 {
    if elapsed == 0 {
        return 0.0;
    }
    if elapsed >= duration {
        return 1.0;
    }
    let (p, q) = (u128::from(elapsed), u128::from(duration));

    // k with 2^63 <= (p << k) / q < 2^64. p and q are in [2^(a-1), 2^a)
    // and [2^(b-1), 2^b) with a <= b; k = 63 + b - a puts the quotient in
    // (2^62, 2^64), so one more bit at most. p << k stays below 2^(64 + b),
    // within u128 for b <= 64.
    let mut k = 63u32
        .saturating_add(elapsed.leading_zeros())
        .saturating_sub(duration.leading_zeros());
    let mut scaled = p.checked_shl(k).unwrap_or(0);
    if scaled.checked_div(q).unwrap_or(0) < 1 << 63 {
        k = k.saturating_add(1);
        scaled = p.checked_shl(k).unwrap_or(0);
    }
    let mut sig = scaled.checked_div(q).unwrap_or(0);
    let rem = scaled.checked_rem(q).unwrap_or(0);

    // To 64 bits: rem < q < 2^64, so 2 * rem fits.
    let twice = rem.saturating_mul(2);
    if twice > q || (twice == q && sig & 1 == 1) {
        sig = sig.saturating_add(1);
    }
    if sig == 1 << 64 {
        sig = 1 << 63;
        k = k.saturating_sub(1);
    }

    // To 24 bits: drop the low 40.
    const LOW: u128 = (1 << 40) - 1;
    const HALF: u128 = 1 << 39;
    let (mut top, low) = (sig >> 40, sig & LOW);
    if low > HALF || (low == HALF && top & 1 == 1) {
        top = top.saturating_add(1);
    }
    if top == 1 << 24 {
        top = 1 << 23;
        k = k.saturating_sub(1);
    }

    // The value is top * 2^(40 - k), top in [2^23, 2^24): the biased
    // exponent is 40 - k + 23 + 127 = 190 - k, from 63 (k = 127, for
    // 1 / (2^64 - 1)) to 127 (k = 63, a ratio rounded up to 1).
    let biased = 190u32.saturating_sub(k);
    let mantissa = u32::try_from(top).unwrap_or(0) & ((1 << 23) - 1);
    f32::from_bits(biased << 23 | mantissa)
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
    if mode != MODE_MANUAL {
        Step::Hold
    } else if torque == 0.0 {
        // the C's float compare: -0 is 0, NaN is not
        Step::Snap
    } else {
        Step::Step
    }
}

/// `obs_source_transition_clock_size()` (`get_cx()`, `get_cy()`): the size
/// `set` (`transition_cx`), or the `actual` one when none is set.
pub fn size(set: u32, actual: u32) -> u32 {
    if set != 0 { set } else { actual }
}

/// `obs_source_transition_clock_fold()`: one step of the size fold, the
/// larger of `size` and a `child`'s.
pub fn fold(size: u32, child: u32) -> u32 {
    size.max(child)
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
        match scale_type {
            SCALE_MAX_ONLY => Some(Self::MaxOnly),
            SCALE_ASPECT => Some(Self::Aspect),
            SCALE_STRETCH => Some(Self::Stretch),
            _ => None,
        }
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
    // f32 division and compares, as the C's float arithmetic: NaN and inf
    // aspects for a transition side of 0, NaN compares false.
    let tr_aspect = tr_cx / tr_cy;

    if source_cx == 0.0 || source_cy == 0.0 {
        return Fit::NoMatrix;
    }

    let source_aspect = source_cx / source_cy;

    let scale_type = match ScaleType::from_c(scale_type) {
        Some(ScaleType::MaxOnly) if source_cx > tr_cx || source_cy > tr_cy => ScaleType::Aspect,
        Some(ScaleType::MaxOnly) => return Fit::Scale { x: 1.0, y: 1.0 },
        Some(scale_type) => scale_type,
        None => return Fit::Unset,
    };

    match scale_type {
        ScaleType::Aspect => {
            let use_width = tr_aspect < source_aspect;
            let scale = if use_width {
                tr_cx / source_cx
            } else {
                tr_cy / source_cy
            };
            Fit::Scale { x: scale, y: scale }
        }
        ScaleType::Stretch => Fit::Scale {
            x: tr_cx / source_cx,
            y: tr_cy / source_cy,
        },
        // turned into Aspect or returned above
        ScaleType::MaxOnly => Fit::Scale { x: 1.0, y: 1.0 },
    }
}

/// `obs_source_transition_clock_mix_spaces()` (`mix_spaces()`):
/// [`CS_709_EXTENDED`] if either space is it or [`CS_709_SCRGB`], else
/// [`CS_SRGB_16F`] if either is it, else [`CS_SRGB`].
pub fn mix_spaces(a: i32, b: i32) -> i32 {
    let extended = |s: i32| s == CS_709_EXTENDED || s == CS_709_SCRGB;
    if extended(a) || extended(b) {
        CS_709_EXTENDED
    } else if a == CS_SRGB_16F || b == CS_SRGB_16F {
        CS_SRGB_16F
    } else {
        CS_SRGB
    }
}

/// `obs_source_transition_clock_min_ts()` (`transition_calc_min_ts()`):
/// the earliest of the timestamps that is not 0, or 0 when all are. A child
/// with no timestamp (none, or its audio pending) is 0.
pub fn min_ts(timestamps: [u64; 2]) -> u64 {
    let mut min_ts = 0;
    for timestamp in timestamps {
        if timestamp != 0 && (min_ts == 0 || timestamp < min_ts) {
            min_ts = timestamp;
        }
    }
    min_ts
}
