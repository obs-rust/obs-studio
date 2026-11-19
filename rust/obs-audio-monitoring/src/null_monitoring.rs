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
pub fn available() -> bool {
    todo!("null monitoring: available")
}

/// `obs_enum_audio_monitoring_devices()`: lists no device, so `visit` (the
/// callback and its data) is never called or touched.
pub fn enum_devices<V>(visit: V) {
    let _ = visit;
    todo!("null monitoring: enum_devices")
}

/// `audio_monitor_create()`: no monitor for any source, NULL or not.
#[must_use]
pub fn create<S>(source: S) -> Option<Monitor> {
    let _ = source;
    todo!("null monitoring: create")
}

/// `audio_monitor_reset()`: does nothing with any handle, NULL or not.
pub fn reset<M>(monitor: M) {
    let _ = monitor;
    todo!("null monitoring: reset")
}

/// `audio_monitor_destroy()`: does nothing with any handle, NULL or not; it
/// owns nothing to free.
pub fn destroy<M>(monitor: M) {
    let _ = monitor;
    todo!("null monitoring: destroy")
}

/// `devices_match()`: false for every pair of IDs, NULL, equal or not. The
/// IDs are never read, so they need not be terminated.
#[must_use]
pub fn devices_match<I, J>(id1: I, id2: J) -> bool {
    let _ = (id1, id2);
    todo!("null monitoring: devices_match")
}
