//! C ABI for `libobs/audio-monitoring/null/null-audio-monitoring.c.h`, the
//! audio monitoring backend Linux and FreeBSD select when PulseAudio is off.
//!
//! The six canonical functions (`obs_audio_monitoring_available()`,
//! `obs_enum_audio_monitoring_devices()`, `audio_monitor_create()`,
//! `audio_monitor_reset()`, `audio_monitor_destroy()` and `devices_match()`)
//! stay thin C forwarders in that file, which only the null backend's group
//! (`libobs.null-audio`) compiles: Windows, macOS and PulseAudio builds
//! define the same names in their own backends, so this always-linked
//! archive must not. The forwarders call these uniquely named functions,
//! declared in `null/null-audio-monitoring.h` without `EXPORT` and hidden
//! (`rust-hidden.d`). Every other build links them unused.
//!
//! No pointer is read, written, kept or freed: the null backend passes the
//! caller's handles, IDs, callback and data through unread.

use core::ffi::{c_char, c_void};

use crate::null_monitoring;

/// `obs_enum_audio_device_cb`: `bool (*)(void *data, const char *name, const
/// char *id)`, never called here.
pub type EnumDeviceCallback =
    Option<unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> bool>;

/// `obs_audio_monitoring_available()` of the null backend: false.
#[unsafe(no_mangle)]
pub extern "C" fn obs_null_audio_monitoring_available() -> bool {
    null_monitoring::available()
}

/// `obs_enum_audio_monitoring_devices()` of the null backend: calls nothing
/// and leaves `data` untouched, for any `cb` and `data`, NULL or not.
#[unsafe(no_mangle)]
pub extern "C" fn obs_null_audio_monitoring_enum_devices(
    cb: EnumDeviceCallback,
    data: *mut c_void,
) {
    null_monitoring::enum_devices((cb, data));
}

/// `audio_monitor_create()` of the null backend: NULL for any `source`, which
/// is never read.
#[unsafe(no_mangle)]
pub extern "C" fn obs_null_audio_monitor_create(source: *mut c_void) -> *mut c_void {
    match null_monitoring::create(source) {
        Some(monitor) => match monitor {},
        None => core::ptr::null_mut(),
    }
}

/// `audio_monitor_reset()` of the null backend: does nothing with `monitor`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_null_audio_monitor_reset(monitor: *mut c_void) {
    null_monitoring::reset(monitor);
}

/// `audio_monitor_destroy()` of the null backend: does nothing with
/// `monitor`, which it does not own.
#[unsafe(no_mangle)]
pub extern "C" fn obs_null_audio_monitor_destroy(monitor: *mut c_void) {
    null_monitoring::destroy(monitor);
}

/// `devices_match()` of the null backend: false for any two IDs, which are
/// never read (they need not be valid or terminated).
#[unsafe(no_mangle)]
pub extern "C" fn obs_null_audio_monitoring_devices_match(
    id1: *const c_char,
    id2: *const c_char,
) -> bool {
    null_monitoring::devices_match(id1, id2)
}
