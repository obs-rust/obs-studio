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
    let _ = (cur, value);
    todo!("monitoring::changes")
}

/// Whether output flags name an audio source: [`AUDIO`] is set.
#[must_use]
pub fn has_audio(output_flags: u32) -> bool {
    let _ = output_flags;
    todo!("monitoring::has_audio")
}

/// The mixers `obs_source_set_audio_mixers()` stores from the `int64_t` a
/// handler leaves: its low 32 bits.
#[must_use]
pub fn mixers_store(mixers: i64) -> u32 {
    let _ = mixers;
    todo!("monitoring::mixers_store")
}

/// Whether the push-to enables log the change: an audio source, and
/// changed.
#[must_use]
pub fn push_to_logs(output_flags: u32, changed: bool) -> bool {
    let _ = (output_flags, changed);
    todo!("monitoring::push_to_logs")
}

/// The monitoring type of a source with monitoring on:
/// [`TYPE_MONITOR_ONLY`] when muted, else [`TYPE_MONITOR_AND_OUTPUT`].
#[must_use]
pub fn on_type(muted: bool) -> i32 {
    let _ = muted;
    todo!("monitoring::on_type")
}

/// The type `obs_source_set_monitoring_enabled()` stores:
/// [`TYPE_MONITOR_AND_OUTPUT`] when on, else [`TYPE_NONE`].
#[must_use]
pub fn stored_type(enabled: bool) -> i32 {
    let _ = enabled;
    todo!("monitoring::stored_type")
}

/// What `obs_source_set_monitoring_enabled()` does with the monitor, from
/// the value it read after the signals and the new one.
#[must_use]
pub fn monitor(was_on: bool, enabled: bool) -> Monitor {
    let _ = (was_on, enabled);
    todo!("monitoring::monitor")
}

/// The steps `obs_source_set_monitoring_type()` takes for a type.
#[must_use]
pub fn route(monitoring_type: i32) -> Route {
    let _ = monitoring_type;
    todo!("monitoring::route")
}

/// Whether leaving for none looks at the mute: only when Monitor Only set
/// it.
#[must_use]
pub fn off_reads_mute(muted_for_monitor_only: bool) -> bool {
    let _ = muted_for_monitor_only;
    todo!("monitoring::off_reads_mute")
}

/// Whether leaving for none unmutes, given the mute read after monitoring
/// turned off: it is still on.
#[must_use]
pub fn off_unmutes(muted: bool) -> bool {
    let _ = muted;
    todo!("monitoring::off_unmutes")
}

/// Whether Monitor Only mutes the source before monitoring turns on: it is
/// not muted.
#[must_use]
pub fn only_mutes(muted: bool) -> bool {
    let _ = muted;
    todo!("monitoring::only_mutes")
}

/// The balance a source stores and sends: NaN is 0.5, else clamped to
/// [0, 1]; -0.0 stays -0.0.
#[must_use]
pub fn balance(value: f32) -> f32 {
    let _ = value;
    todo!("monitoring::balance")
}

/// Which signal `obs_source_set_audio_active()` sends, from the previous
/// value `os_atomic_set_bool()` returned and the new one.
#[must_use]
pub fn audio_active(prev: bool, active: bool) -> Active {
    let _ = (prev, active);
    todo!("monitoring::audio_active")
}
