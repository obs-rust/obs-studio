//! `obs_set_audio_monitoring_device()` and `obs_reset_audio_monitoring()`
//! over C-owned state: the monitoring fields, their mutex and bmem copies,
//! the monitor list and `audio_monitor_reset()`, the source enumeration and
//! the Audio Output Capture notice, behind the private adapters of
//! `libobs/obs-audio-monitoring-device-fields.c.h` (#5491). Which run, and
//! in what order, is the safe core's ([`crate::audio_monitoring_device`]).
use core::ffi::{c_char, c_int, c_void};
use core::ptr::{NonNull, null_mut};

use crate::audio_monitoring_device::{Device, Monitors, aoc_source, reset_monitoring, set_device};

/// `bool (*)(void *, obs_source_t *)`, the `obs_enum_sources()` callback.
pub type Visit = unsafe extern "C" fn(*mut c_void, *mut c_void) -> bool;

unsafe extern "C" {
    fn obs_audio_monitoring_device_available() -> bool;
    fn obs_audio_monitoring_device_lock();
    fn obs_audio_monitoring_device_unlock();
    fn obs_audio_monitoring_device_compare(id: *const c_char) -> c_int;
    fn obs_audio_monitoring_device_store(name: *const c_char, id: *const c_char);
    fn obs_audio_monitoring_device_monitor_count() -> usize;
    fn obs_audio_monitoring_device_reset_monitor(index: usize);
    fn obs_audio_monitoring_device_enum_sources(visit: Visit, param: *mut c_void);
    fn obs_audio_monitoring_device_output_flags(source: *const c_void) -> u32;
    fn obs_audio_monitoring_device_notify(source: *mut c_void);
}

/// The live core, reached only through the adapters.
struct Libobs;

impl Monitors for Libobs {
    fn available(&mut self) -> bool {
        // SAFETY: asks the monitoring backend only.
        unsafe { obs_audio_monitoring_device_available() }
    }
    fn lock(&mut self) {
        // SAFETY: the live core's monitoring_mutex, which the core takes once
        // and releases once.
        unsafe { obs_audio_monitoring_device_lock() }
    }
    fn unlock(&mut self) {
        // SAFETY: called once, after `lock`.
        unsafe { obs_audio_monitoring_device_unlock() }
    }
    fn count(&mut self) -> usize {
        // SAFETY: read under monitoring_mutex, which `lock` holds.
        unsafe { obs_audio_monitoring_device_monitor_count() }
    }
    fn reset(&mut self, index: usize) {
        // SAFETY: `index < count()` under the same hold of monitoring_mutex.
        unsafe { obs_audio_monitoring_device_reset_monitor(index) }
    }
}

impl Device for Libobs {
    type Str = NonNull<c_char>;
    type Source = *mut c_void;
    fn empty(&mut self, s: NonNull<c_char>) -> bool {
        // SAFETY: the caller's non-NULL string, readable at its first byte.
        unsafe { *s.as_ptr() == 0 }
    }
    fn compare(&mut self, id: NonNull<c_char>) -> i32 {
        // SAFETY: the caller's NUL-terminated id; the stored id is read under
        // monitoring_mutex, which `lock` holds.
        unsafe { obs_audio_monitoring_device_compare(id.as_ptr()) }
    }
    fn store(&mut self, name: NonNull<c_char>, id: NonNull<c_char>) {
        // SAFETY: the caller's NUL-terminated strings, copied under
        // monitoring_mutex; the stored ones are freed once.
        unsafe { obs_audio_monitoring_device_store(name.as_ptr(), id.as_ptr()) }
    }
    fn enum_sources(&mut self) {
        // SAFETY: `visit` only reads the flags of and notifies each source
        // the enumeration hands it; the original passes no parameter.
        unsafe { obs_audio_monitoring_device_enum_sources(visit, null_mut()) }
    }
    fn output_flags(&mut self, source: *mut c_void) -> u32 {
        // SAFETY: a live source handed over by `obs_enum_sources()`.
        unsafe { obs_audio_monitoring_device_output_flags(source) }
    }
    fn notify(&mut self, source: *mut c_void) {
        // SAFETY: as in `output_flags`.
        unsafe { obs_audio_monitoring_device_notify(source) }
    }
}

/// The `obs_enum_sources()` callback: tells an Audio Output Capture source
/// about the new device; always continues.
///
/// # Safety
/// `source` is a live source handed over by the enumeration.
unsafe extern "C" fn visit(_param: *mut c_void, source: *mut c_void) -> bool {
    aoc_source(&mut Libobs, source)
}

/// Reset every live monitor onto the current device.
///
/// # Safety
/// As for the C function: the core is live and its monitoring_mutex is not
/// held by this thread.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_reset_audio_monitoring() {
    reset_monitoring(&mut Libobs);
}

/// Store a new monitoring device, reset the monitors onto it and tell the
/// Audio Output Capture sources.
///
/// # Safety
/// As for the C function: `name` and `id` are NULL or NUL-terminated
/// strings; once both are non-empty the core is live, its monitoring_mutex
/// is not held by this thread and its stored id is a string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_set_audio_monitoring_device(
    name: *const c_char,
    id: *const c_char,
) -> bool {
    set_device(
        &mut Libobs,
        NonNull::new(name.cast_mut()),
        NonNull::new(id.cast_mut()),
    )
}
