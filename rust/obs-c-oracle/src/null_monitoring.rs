//! `oracle_*` build of the original null audio monitoring backend
//! (`libobs/audio-monitoring/null/null-audio-monitoring-reference.c.h`,
//! oracle/null_monitoring.c).

use core::ffi::{c_char, c_void};

/// `obs_enum_audio_device_cb`.
pub type EnumDeviceCallback =
    Option<unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> bool>;

unsafe extern "C" {
    pub fn oracle_null_audio_monitoring_available() -> bool;
    pub fn oracle_null_audio_monitoring_enum_devices(cb: EnumDeviceCallback, data: *mut c_void);
    pub fn oracle_null_audio_monitor_create(source: *mut c_void) -> *mut c_void;
    pub fn oracle_null_audio_monitor_reset(monitor: *mut c_void);
    pub fn oracle_null_audio_monitor_destroy(monitor: *mut c_void);
    pub fn oracle_null_audio_monitoring_devices_match(
        id1: *const c_char,
        id2: *const c_char,
    ) -> bool;
}
