//! Safe core of `libobs/audio-monitoring/null/null-audio-monitoring.c.h`, the
//! audio monitoring backend Linux and FreeBSD select when PulseAudio is off.
//!
//! The backend has no output device: monitoring is unavailable, the device
//! list is empty, no monitor is ever created, and no two device IDs match.
//! None of its six functions reads, writes, keeps or frees what the caller
//! passes, logs or allocates; every argument here is generic and dropped
//! unread, so the C ABI shims ([`crate::ffi::null_monitoring`]) pass their
//! pointers through without dereferencing them.

/// A monitor of the null backend. It has no value: [`create`] never returns
/// one, so [`reset`] and [`destroy`] never get one either.
#[derive(Debug)]
pub enum Monitor {}

/// `obs_audio_monitoring_available()`: always false.
#[must_use]
pub const fn available() -> bool {
    false
}

/// `obs_enum_audio_monitoring_devices()`: lists no device, so `visit` (the
/// callback and its data) is never called or touched.
pub fn enum_devices<V>(visit: V) {
    drop(visit);
}

/// `audio_monitor_create()`: no monitor for any source, NULL or not.
#[must_use]
pub fn create<S>(source: S) -> Option<Monitor> {
    drop(source);
    None
}

/// `audio_monitor_reset()`: does nothing with any handle, NULL or not.
pub fn reset<M>(monitor: M) {
    drop(monitor);
}

/// `audio_monitor_destroy()`: does nothing with any handle, NULL or not; it
/// owns nothing to free.
pub fn destroy<M>(monitor: M) {
    drop(monitor);
}

/// `devices_match()`: false for every pair of IDs, NULL, equal or not. The
/// IDs are never read, so they need not be terminated.
#[must_use]
pub fn devices_match<I, J>(id1: I, id2: J) -> bool {
    drop((id1, id2));
    false
}
