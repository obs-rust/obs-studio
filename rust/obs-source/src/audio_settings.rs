//! Safe core of `libobs/obs-source-audio-settings.c.h`: the volume, sync
//! offset, balance, mute and monitoring setters and getters of a source,
//! moved out of `libobs/obs-source.c.h`.
//!
//! The functions make the C's calls in its order and under its locks, generic
//! over a [`Source`]: one method per field read or write, lock, signal and
//! call into the rest of libobs. The C shims live in the `obs-source-seam`
//! crate, which implements [`Source`] over
//! `libobs/obs-source-audio-settings-seam.h`, so this crate calls nothing in
//! libobs. The fields stay in `struct obs_source`, and each read and write is
//! C, so ThreadSanitizer keeps seeing them.
//!
//! What the C does and these keep (characterized, not endorsed;
//! `test_source_audio_settings` pins most of it):
//!
//! - The volume, sync offset and balance are stored after their signal, and a
//!   handler can change them. The mute is stored before its signal.
//! - The volume's queued action has the value from before the handlers, and
//!   the stored volume has the value from after them.
//! - A set of the volume or the mute to the same value signals and queues.
//!   A set of monitoring to the same value does nothing.
//! - `obs_source_set_muted()` reads the clock before it checks its source.
//!   `obs_source_get_monitoring_type()` logs before it checks its source.
//! - No signal goes out with a lock held.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::as_conversions,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable,
)]

use std::ffi::CStr;


/// What the getter answers for a NULL source.
pub const VOLUME_INVALID: f32 = 0.0;
/// What the getter answers for a NULL source.
pub const BALANCE_INVALID: f32 = 0.5;

/// The warning of `obs_source_set_monitoring_type()`.
pub const SET_TYPE_DEPRECATED: &CStr =
    c"obs_source_set_monitoring_type is deprecated. Use obs_source_set_monitoring_enabled instead.";
/// The warning of `obs_source_get_monitoring_type()`.
pub const GET_TYPE_DEPRECATED: &CStr =
    c"obs_source_get_monitoring_type is deprecated. Use obs_source_get_monitoring_enabled instead.";

/// The source audio settings seam: one method per field read or write, lock,
/// signal and libobs call. A lock method is one lock call, and none of the
/// others takes a lock of its own.
pub trait Source {
    /// `obs_source_valid()`: logs `func` for `None`.
    fn valid(source: Option<&Self>, func: &'static CStr) -> bool;
    /// `os_gettime_ns()`.
    fn now() -> u64;
    /// `blog(LOG_WARNING, ...)`.
    fn warn(message: &'static CStr);

    /// `source->context.private`.
    fn is_private(&self) -> bool;

    /// Lock `audio_buf_mutex`.
    fn lock_audio_buf(&self);
    /// Unlock `audio_buf_mutex`.
    fn unlock_audio_buf(&self);
    /// Lock `audio_actions_mutex`.
    fn lock_actions(&self);
    /// Unlock `audio_actions_mutex`.
    fn unlock_actions(&self);

    /// Read `user_volume`, under `audio_buf_mutex`.
    fn user_volume(&self) -> f32;
    /// Write `user_volume`, under `audio_buf_mutex`.
    fn set_user_volume(&self, value: f32);
    /// Read `sync_offset`, under `audio_buf_mutex`.
    fn sync_offset(&self) -> i64;
    /// Write `sync_offset`, under `audio_buf_mutex`.
    fn set_sync_offset(&self, value: i64);
    /// Read `balance`, under `audio_buf_mutex`.
    fn balance(&self) -> f32;
    /// Write `balance`, under `audio_buf_mutex`.
    fn set_balance(&self, value: f32);
    /// Write `monitoring_type`, under `audio_buf_mutex`.
    fn set_monitoring_type(&self, value: i32);

    /// Atomic load of `user_muted`.
    fn user_muted(&self) -> bool;
    /// Atomic store of `user_muted`.
    fn set_user_muted(&self, value: bool);
    /// Atomic load of `monitor_only_muted`.
    fn monitor_only_muted(&self) -> bool;
    /// Atomic store of `monitor_only_muted`.
    fn set_monitor_only_muted(&self, value: bool);
    /// Atomic load of `monitoring_enabled`.
    fn monitoring_enabled(&self) -> bool;
    /// Atomic store of `monitoring_enabled`.
    fn set_monitoring_enabled(&self, value: bool);

    /// Queue an `AUDIO_ACTION_VOL`, under `audio_actions_mutex`.
    fn push_volume_action(&self, timestamp: u64, volume: f32);
    /// Queue an `AUDIO_ACTION_MUTE`, under `audio_actions_mutex`.
    fn push_mute_action(&self, timestamp: u64, muted: bool);

    /// The `volume` signal of the source and, with `also_core`, the
    /// `source_volume` signal of libobs, on one calldata. Returns the volume
    /// in the calldata after them.
    fn signal_volume(&self, volume: f64, also_core: bool) -> f64;
    /// The `audio_sync` signal. Returns the offset in the calldata after it.
    fn signal_sync_offset(&self, offset: i64) -> i64;
    /// The `audio_balance` signal. Returns the balance in the calldata after
    /// it.
    fn signal_balance(&self, balance: f64) -> f64;
    /// The `mute` signal.
    fn signal_muted(&self, muted: bool);
    /// The `monitor` signal.
    fn signal_monitor(&self, enabled: bool);
    /// The `audio_monitoring` signal.
    fn signal_audio_monitoring(&self, monitoring_type: i32);

    /// `source->monitor = audio_monitor_create(source)`.
    fn monitor_create(&self);
    /// `audio_monitor_destroy(source->monitor)`, and `source->monitor = NULL`.
    fn monitor_destroy(&self);
}

/// `clamp_volume()`: the volume multiplies every sample, so it is 0 or more.
/// NaN is the default, 1, and +inf is 20 (+26 dB), the most the Advanced Audio
/// Properties (2000%) and obs-websocket's `SetInputVolume` set. Other volumes
/// of 0 or more are kept, above 20 too (#2108).
#[must_use]
pub fn clamp_volume(volume: f32) -> f32 {
    todo!()
}

#[allow(dead_code)]
/// `(float)` of the calldata's double, as the C casts it.
#[allow(clippy::cast_possible_truncation, clippy::as_conversions)]
fn to_f32(value: f64) -> f32 {
    value as f32
}

/// `obs_source_set_volume()`.
pub fn set_volume<S: Source>(source: Option<&S>, volume: f32) {
    todo!()
}

/// `obs_source_get_volume()`.
#[must_use]
pub fn get_volume<S: Source>(source: Option<&S>) -> f32 {
    todo!()
}

/// `obs_source_set_sync_offset()`.
pub fn set_sync_offset<S: Source>(source: Option<&S>, offset: i64) {
    todo!()
}

/// `obs_source_get_sync_offset()`.
#[must_use]
pub fn get_sync_offset<S: Source>(source: Option<&S>) -> i64 {
    todo!()
}

/// `obs_source_muted()`.
#[must_use]
pub fn muted<S: Source>(source: Option<&S>) -> bool {
    todo!()
}

/// `obs_source_set_muted()`.
pub fn set_muted<S: Source>(source: Option<&S>, muted: bool) {
    todo!()
}

/// `obs_source_set_monitoring_enabled()`.
pub fn set_monitoring_enabled<S: Source>(source: Option<&S>, enabled: bool) {
    todo!()
}

/// `obs_source_get_monitoring_enabled()`.
#[must_use]
pub fn get_monitoring_enabled<S: Source>(source: Option<&S>) -> bool {
    todo!()
}

/// `obs_source_set_monitoring_type()`.
///
/// Since 33.0 monitoring is on or off, on means monitor and output, and the
/// monitor ignores mute: monitor-only is monitoring on and muted, as a
/// pre-33.0 Monitor Only source loads (#2444, #2447). The mute comes first, so
/// the source never plays into the outputs in between. Leaving monitor-only
/// for none undoes the mute monitor-only set, as long as nothing has set the
/// mute since; any other mute stays.
pub fn set_monitoring_type<S: Source>(source: Option<&S>, monitoring_type: i32) {
    todo!()
}

/// `obs_source_get_monitoring_type()`.
#[must_use]
pub fn get_monitoring_type<S: Source>(source: Option<&S>) -> i32 {
    todo!()
}

/// `obs_source_set_balance_value()`.
pub fn set_balance_value<S: Source>(source: Option<&S>, balance: f32) {
    todo!()
}

/// `obs_source_get_balance_value()`.
#[must_use]
pub fn get_balance_value<S: Source>(source: Option<&S>) -> f32 {
    todo!()
}
