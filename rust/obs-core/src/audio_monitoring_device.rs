//! Safe core of `libobs/obs-audio-monitoring-device.c.h`:
//! `obs_set_audio_monitoring_device()` and `obs_reset_audio_monitoring()`
//! (#5491).
//!
//! Setting the device refuses, before anything else, a missing name, a
//! missing id, an empty name and an empty id, asked in that order through
//! [`monitor_missing`]; then refuses when monitoring is unavailable. Under
//! the monitoring mutex an id equal to the stored one
//! ([`monitor_same`] of the `strcmp()`) unlocks and answers true with
//! nothing changed. Otherwise the stored name and id are replaced by copies,
//! the mutex is released, every live monitor is reset
//! ([`reset_monitoring`]), and each source the core enumerates is offered to
//! [`aoc_source`], which tells an Audio Output Capture source
//! ([`aoc_check`] of its output flags) about the change with its own
//! `device_id` setting.
//!
//! Resetting does nothing when monitoring is unavailable; otherwise it
//! resets each live monitor once under the monitoring mutex, reading the
//! monitor count again before each step, as the C loop does.
//!
//! The monitoring fields, their mutex and copies, the monitor list,
//! `audio_monitor_reset()`, the source enumeration and the notice stay with
//! the caller ([`Monitors`], [`Device`]).

#![deny(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

use crate::core_setup_rules::{aoc_check, monitor_missing, monitor_same};

/// The live monitors as `obs_reset_audio_monitoring()` sees them:
/// `obs_audio_monitoring_available()`, `obs->audio.monitoring_mutex`,
/// `monitors.num` and `audio_monitor_reset(monitors.array[i])`.
pub trait Monitors {
    /// One call of `obs_audio_monitoring_available()`.
    fn available(&mut self) -> bool;
    fn lock(&mut self);
    fn unlock(&mut self);
    /// One read of `monitors.num`.
    fn count(&mut self) -> usize;
    /// Reset monitor `index`, for `index < count()`.
    fn reset(&mut self, index: usize);
}

/// The stored device and the sources, as `obs_set_audio_monitoring_device()`
/// sees them.
pub trait Device: Monitors {
    /// A caller's `const char *`, non-NULL.
    type Str: Copy;
    /// An `obs_source_t *` the enumeration hands over.
    type Source: Copy;
    /// `*s == '\0'`.
    fn empty(&mut self, s: Self::Str) -> bool;
    /// `strcmp(id, obs->audio.monitoring_device_id)`.
    fn compare(&mut self, id: Self::Str) -> i32;
    /// Free the stored name and id, then store copies of `name` and `id`.
    fn store(&mut self, name: Self::Str, id: Self::Str);
    /// `obs_enum_sources()`: hand each source to [`aoc_source`] in order,
    /// stopping at the first false.
    fn enum_sources(&mut self);
    /// `source->info.output_flags`.
    fn output_flags(&mut self, source: Self::Source) -> u32;
    /// Tell `source` about the new device with its own `device_id` setting.
    fn notify(&mut self, source: Self::Source);
}

/// `obs_reset_audio_monitoring()`.
pub fn reset_monitoring<M: Monitors + ?Sized>(monitors: &mut M) {
    if !monitors.available() {
        return;
    }
    monitors.lock();
    let mut index: usize = 0;
    while index < monitors.count() {
        monitors.reset(index);
        // `index < count()`, a `usize`, so this never wraps.
        index = index.wrapping_add(1);
    }
    monitors.unlock();
}

/// The enumeration callback: tell an Audio Output Capture source; always
/// continue.
pub fn aoc_source<D: Device + ?Sized>(device: &mut D, source: D::Source) -> bool {
    if aoc_check(device.output_flags(source)) {
        device.notify(source);
    }
    true
}

/// `obs_set_audio_monitoring_device(name, id)`; `None` is NULL.
pub fn set_device<D: Device + ?Sized>(
    device: &mut D,
    name: Option<D::Str>,
    id: Option<D::Str>,
) -> bool {
    if monitor_missing(name.is_some()) || monitor_missing(id.is_some()) {
        return false;
    }
    let (Some(name), Some(id)) = (name, id) else {
        return false;
    };
    if monitor_missing(!device.empty(name)) || monitor_missing(!device.empty(id)) {
        return false;
    }
    if monitor_missing(device.available()) {
        return false;
    }
    device.lock();
    if monitor_same(device.compare(id)) {
        device.unlock();
        return true;
    }
    device.store(name, id);
    device.unlock();
    reset_monitoring(device);
    device.enum_sources();
    true
}
