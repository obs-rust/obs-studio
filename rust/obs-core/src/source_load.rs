//! Safe core of `libobs/obs-source-load.c.h`: the source load migration and
//! value rules of `obs_load_source_type()` (`obs.c`).
//!
//! `obs.c` keeps every `obs_data_*()` call and setter, in the same order and
//! the same number of calls, the source creation, the canvas lookup and the
//! filter recursion. It reads each saved value, passes it here, and hands
//! the answer to the setter. These keep no state and take no lock: they only
//! compute values.
//!
//! - [`prev_ver`]: the saved `"prev_ver"` (an `int64`) as the `uint32_t` the
//!   source is created with: its low 32 bits ("characterized, not
//!   endorsed": `2^32 + 23.2.1` reads as 23.2.1).
//! - [`volume`] (#2128): 0 when negative, infinity past `FLT_MAX`, then the
//!   nearest `f32`. NaN stays NaN; `obs_source_set_volume()` maps NaN,
//!   negative and infinite volumes.
//! - [`balance`] (#2058): 0 when negative, 1 when past 1, then the nearest
//!   `f32`. NaN stays NaN; `obs_source_set_balance_value()` maps it to 0.5.
//! - [`delay`] (#2058): a negative push-to-mute or push-to-talk delay is
//!   none.
//! - [`deinterlace_mode`] (#2120): a mode past the `int` range is
//!   [`DEINTERLACE_DISABLE`], not its low 32 bits; one inside it is passed
//!   on (the setter disables a mode it does not know).
//! - [`monitoring_migration`]: before 23.2.2 a source with
//!   [`MONITOR_BY_DEFAULT`] becomes monitor-only with all six mixers; before
//!   33.0.0 `"monitoring_enabled"` is stored as the (migrated) type not being
//!   none. The saved type is an `int64` read as an `int` ("characterized,
//!   not endorsed": `2^32` is none).
//!
//! The comparisons are the C's, NaN included: `-0.0` is not below 0, so it
//! stays `-0.0` ("characterized, not endorsed"), and a value past `FLT_MAX`
//! that `f32` rounding would bring back to `FLT_MAX` is still infinity. The
//! `f64` to `f32` conversion is round to nearest, ties to even, as the C's
//! cast under the default floating-point environment.
//!
//! Every input comes from a scene collection (any JSON number), so nothing
//! here can panic on any input.
//!
//! No intentional differences.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

/// `MAKE_SEMANTIC_VERSION(major, minor, patch)` (`obs-config.h`).
#[must_use]
pub const fn semantic_version(major: u8, minor: u8, patch: u16) -> u32 {
    (u32_of(major) << 24) | (u32_of(minor) << 16) | u32_of16(patch)
}

/// `MAKE_SEMANTIC_VERSION(23, 2, 2)`: a source saved before it, with
/// [`MONITOR_BY_DEFAULT`], becomes monitor-only.
pub const MONITOR_ONLY_BEFORE: u32 = semantic_version(23, 2, 2);

/// `MAKE_SEMANTIC_VERSION(33, 0, 0)`: a source saved before it gets its
/// `"monitoring_enabled"` from its monitoring type.
pub const MONITORING_ENABLED_BEFORE: u32 = semantic_version(33, 0, 0);

/// `OBS_SOURCE_MONITOR_BY_DEFAULT` (`obs-source.h`).
pub const MONITOR_BY_DEFAULT: u32 = 1 << 11;

/// `OBS_MONITORING_TYPE_NONE` (`obs.h`).
pub const MONITORING_NONE: i32 = 0;

/// `OBS_MONITORING_TYPE_MONITOR_ONLY` (`obs.h`).
pub const MONITORING_MONITOR_ONLY: i32 = 1;

/// All six audio mixers (`MAX_AUDIO_MIXES`).
pub const ALL_MIXERS: u32 = 0x3F;

/// `OBS_DEINTERLACE_MODE_DISABLE` (`obs.h`).
pub const DEINTERLACE_DISABLE: i32 = 0;

/// What `obs_load_source_type()` does after reading `"monitoring_type"`
/// (`struct obs_source_load_monitoring`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct MonitoringPlan {
    /// `obs_source_set_audio_mixers(source, mixers)`: a monitor-by-default
    /// source saved before 23.2.2.
    pub set_mixers: bool,
    /// [`ALL_MIXERS`] when [`set_mixers`](Self::set_mixers), else 0.
    pub mixers: u32,
    /// `obs_data_set_bool(source_data, "monitoring_enabled", enabled)`: a
    /// source saved before 33.0.0.
    pub store_enabled: bool,
    /// When [`store_enabled`](Self::store_enabled), whether the (migrated)
    /// monitoring type is not none; else false.
    pub enabled: bool,
}

/// `uint32_t` of a `u8`.
const fn u32_of(value: u8) -> u32 {
    value as u32
}

/// `uint32_t` of a `u16`.
const fn u32_of16(value: u16) -> u32 {
    value as u32
}

/// `obs_source_load_prev_ver()`: the saved `"prev_ver"` as a `uint32_t`, its
/// low 32 bits (the C's cast).
#[must_use]
#[allow(
    clippy::as_conversions,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
pub const fn prev_ver(stored: i64) -> u32 {
    stored as u32
}

/// `obs_source_load_volume()`: the saved `"volume"`, 0 when negative,
/// infinity past `FLT_MAX`, then as an `f32`. NaN stays NaN.
#[must_use]
#[allow(clippy::as_conversions, clippy::cast_possible_truncation)]
pub fn volume(saved: f64) -> f32 {
    let value = if saved < 0.0 {
        0.0
    } else if saved > f64::from(f32::MAX) {
        f64::INFINITY
    } else {
        saved
    };
    value as f32
}

/// `obs_source_load_balance()`: the saved `"balance"`, 0 when negative, 1
/// past 1, then as an `f32`. NaN stays NaN.
#[must_use]
#[allow(clippy::as_conversions, clippy::cast_possible_truncation)]
pub fn balance(saved: f64) -> f32 {
    // f64::clamp() is the C's two comparisons: NaN and -0.0 pass through
    saved.clamp(0.0, 1.0) as f32
}

/// `obs_source_load_delay()`: a saved push-to-mute or push-to-talk delay, 0
/// when negative.
#[must_use]
pub fn delay(saved: i64) -> u64 {
    u64::try_from(saved).unwrap_or(0)
}

/// `obs_source_load_deinterlace_mode()`: the saved `"deinterlace_mode"`,
/// [`DEINTERLACE_DISABLE`] past the `int` range, else as an `int`.
#[must_use]
pub fn deinterlace_mode(saved: i64) -> i32 {
    i32::try_from(saved).unwrap_or(DEINTERLACE_DISABLE)
}

/// The saved `"monitoring_type"` read as an `int`: its low 32 bits (the C's
/// cast).
#[allow(clippy::as_conversions, clippy::cast_possible_truncation)]
const fn monitoring_type_of(stored: i64) -> i32 {
    stored as i32
}

/// `obs_source_load_monitoring_migration()`: the monitoring migration for a
/// source saved by `prev_ver`, with the output flags `caps` and the saved
/// `"monitoring_type"`.
#[must_use]
pub fn monitoring_migration(prev_ver: u32, caps: u32, monitoring_type: i64) -> MonitoringPlan {
    let mut plan = MonitoringPlan::default();
    let mut monitoring_type = monitoring_type_of(monitoring_type);

    if prev_ver < MONITOR_ONLY_BEFORE && caps & MONITOR_BY_DEFAULT != 0 {
        // updates older sources to enable monitoring automatically if they
        // added monitoring by default in version 24
        monitoring_type = MONITORING_MONITOR_ONLY;
        plan.set_mixers = true;
        plan.mixers = ALL_MIXERS;
    }

    if prev_ver < MONITORING_ENABLED_BEFORE {
        plan.store_enabled = true;
        plan.enabled = monitoring_type != MONITORING_NONE;
    }

    plan
}
