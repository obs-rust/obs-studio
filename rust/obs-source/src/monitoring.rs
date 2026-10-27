//! Safe core of `libobs/obs-source-monitoring.c.h`: the monitoring,
//! monitor-only mute, balance, mixer, flag, push-to and audio active rules
//! of the sources in `libobs/obs-source.c.h`.
//!
//! - [`changes`]: the same-value rule of `obs_source_set_flags()`,
//!   `obs_source_set_audio_mixers()`, `obs_source_set_monitoring_enabled()`
//!   and the push-to enables' signal;
//! - [`has_audio`] and [`mixers_store`]: the audio gate of the mixers and
//!   the value an `audio_mixers` handler leaves;
//! - [`push_to_logs`]: the push-to enables log a change on an audio source;
//! - [`on_type`], [`stored_type`] and [`monitor`]: the deprecated and stored
//!   monitoring types (#2447) and the monitor route of
//!   `obs_source_set_monitoring_enabled()`;
//! - [`route`], [`off_reads_mute`], [`off_unmutes`] and [`only_mutes`]: the
//!   steps of `obs_source_set_monitoring_type()` (#2444, #2447);
//! - [`balance`]: `clamp_balance()` (#2051);
//! - [`audio_active`]: the signal of `obs_source_set_audio_active()`.
//!
//! Each decision is its own function, asked at its own point, so
//! `obs-source.c` keeps every read and call where it was and under its
//! condition: `obs_source_valid()`, `audio_mutex`, every `os_atomic_*()` on
//! `monitor_only_muted` and `audio_active`, the reads of the source's
//! fields, `obs_source_get_output_flags()`, `obs_source_muted()` and
//! `obs_source_set_muted()` (the C's own `&&` before each), every
//! `calldata_*()`, signal and log, `audio_monitor_create()` and
//! `audio_monitor_destroy()`, and the getters' answers for an invalid source
//! as C literals. Nothing here keeps state, takes a lock or calls libobs.
//!
//! Characterized, not endorsed (`test_source_monitoring_steps`): the mixers
//! a handler leaves are truncated to their low 32 bits, a balance of -0.0
//! stays -0.0, and Monitor and Output unmutes a mute the user set.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::as_conversions,
    clippy::float_cmp,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

/// `OBS_SOURCE_AUDIO` (`obs-source.h`).
pub const AUDIO: u32 = 1 << 1;

/// `OBS_MONITORING_TYPE_NONE` (`enum obs_monitoring_type`).
pub const TYPE_NONE: i32 = 0;
/// `OBS_MONITORING_TYPE_MONITOR_ONLY`.
pub const TYPE_MONITOR_ONLY: i32 = 1;
/// `OBS_MONITORING_TYPE_MONITOR_AND_OUTPUT`.
pub const TYPE_MONITOR_AND_OUTPUT: i32 = 2;

/// `enum obs_source_monitoring_monitor`: what
/// `obs_source_set_monitoring_enabled()` does with the monitor. The
/// discriminants are the C's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Monitor {
    /// Leave it.
    Keep = 0,
    /// `audio_monitor_create()`.
    Create = 1,
    /// `audio_monitor_destroy()`, and none.
    Destroy = 2,
}

/// `enum obs_source_monitoring_route`: the steps
/// `obs_source_set_monitoring_type()` takes for a type. The discriminants
/// are the C's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Route {
    /// None: monitoring off, then undo the mute Monitor Only set if it is
    /// still on ([`off_reads_mute`], [`off_unmutes`]), and clear the mark.
    Off = 0,
    /// Monitor Only: mute first if not muted ([`only_mutes`]) and mark it,
    /// monitoring on, and store the mark.
    MonitorOnly = 1,
    /// Monitor and Output: unmute, and monitoring on.
    MonitorAndOutput = 2,
    /// Any other value: monitoring on, as before.
    On = 3,
}

/// `enum obs_source_monitoring_active`: which signal
/// `obs_source_set_audio_active()` sends. The discriminants are the C's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Active {
    /// None: the value did not change.
    None = 0,
    /// `source_audio_activate` and `audio_activate`.
    Activate = 1,
    /// `source_audio_deactivate` and `audio_deactivate`.
    Deactivate = 2,
}

/// Whether a set changes the value: `cur != value`.
#[must_use]
pub fn changes(cur: u32, value: u32) -> bool {
    cur != value
}

/// Whether output flags name an audio source: [`AUDIO`] is set.
#[must_use]
pub fn has_audio(output_flags: u32) -> bool {
    output_flags & AUDIO != 0
}

/// The mixers `obs_source_set_audio_mixers()` stores from the `int64_t` a
/// handler leaves: its low 32 bits.
#[must_use]
pub fn mixers_store(mixers: i64) -> u32 {
    let [b0, b1, b2, b3, ..] = mixers.to_le_bytes();
    u32::from_le_bytes([b0, b1, b2, b3])
}

/// Whether the push-to enables log the change: an audio source, and
/// changed.
#[must_use]
pub fn push_to_logs(output_flags: u32, changed: bool) -> bool {
    has_audio(output_flags) && changed
}

/// The monitoring type of a source with monitoring on:
/// [`TYPE_MONITOR_ONLY`] when muted, else [`TYPE_MONITOR_AND_OUTPUT`].
#[must_use]
pub fn on_type(muted: bool) -> i32 {
    if muted {
        TYPE_MONITOR_ONLY
    } else {
        TYPE_MONITOR_AND_OUTPUT
    }
}

/// The type `obs_source_set_monitoring_enabled()` stores:
/// [`TYPE_MONITOR_AND_OUTPUT`] when on, else [`TYPE_NONE`].
#[must_use]
pub fn stored_type(enabled: bool) -> i32 {
    if enabled {
        TYPE_MONITOR_AND_OUTPUT
    } else {
        TYPE_NONE
    }
}

/// What `obs_source_set_monitoring_enabled()` does with the monitor, from
/// the value it read after the signals and the new one.
#[must_use]
pub fn monitor(was_on: bool, enabled: bool) -> Monitor {
    match (was_on, enabled) {
        (false, true) => Monitor::Create,
        (true, false) => Monitor::Destroy,
        _ => Monitor::Keep,
    }
}

/// The steps `obs_source_set_monitoring_type()` takes for a type.
#[must_use]
pub fn route(monitoring_type: i32) -> Route {
    match monitoring_type {
        TYPE_NONE => Route::Off,
        TYPE_MONITOR_ONLY => Route::MonitorOnly,
        TYPE_MONITOR_AND_OUTPUT => Route::MonitorAndOutput,
        // not a type: on, as before
        _ => Route::On,
    }
}

/// Whether leaving for none looks at the mute: only when Monitor Only set
/// it.
#[must_use]
pub fn off_reads_mute(muted_for_monitor_only: bool) -> bool {
    muted_for_monitor_only
}

/// Whether leaving for none unmutes, given the mute read after monitoring
/// turned off: it is still on.
#[must_use]
pub fn off_unmutes(muted: bool) -> bool {
    muted
}

/// Whether Monitor Only mutes the source before monitoring turns on: it is
/// not muted.
#[must_use]
pub fn only_mutes(muted: bool) -> bool {
    !muted
}

/// The balance a source stores and sends: NaN is 0.5, else clamped to
/// [0, 1]; -0.0 stays -0.0. The compares are the C's, in its order, rather than
/// `f32::clamp()`, so the result is the C's for every bit pattern.
#[must_use]
#[allow(clippy::manual_clamp)]
pub fn balance(value: f32) -> f32 {
    if value.is_nan() {
        0.5
    } else if value < 0.0 {
        0.0
    } else if value > 1.0 {
        1.0
    } else {
        value
    }
}

/// Which signal `obs_source_set_audio_active()` sends, from the previous
/// value `os_atomic_set_bool()` returned and the new one.
#[must_use]
pub fn audio_active(prev: bool, active: bool) -> Active {
    match (prev, active) {
        (false, true) => Active::Activate,
        (true, false) => Active::Deactivate,
        _ => Active::None,
    }
}
