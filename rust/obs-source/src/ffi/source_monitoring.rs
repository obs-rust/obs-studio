//! C ABI for `libobs/obs-source-monitoring.c.h`, the monitoring,
//! monitor-only mute, balance, mixer, flag, push-to and audio active rules
//! of the sources, declared in `libobs/obs-source-monitoring.h` without
//! `EXPORT`.
//!
//! `obs-source.c.h` (still C) checks the source with `obs_source_valid()`,
//! reads its fields and the atomics at their points, passes the results as
//! plain values to these shims and acts on what comes back: the mutex, the
//! atomics, `obs_source_muted()` and `obs_source_set_muted()` (each only
//! under the C's own `&&`), the calldata, the signals, the monitor's create
//! and destroy and the logs stay there. So `struct obs_source` is never
//! mirrored, no pointer crosses, and the shims take no lock, call nothing in
//! libobs and touch no memory. All thirteen stay hidden (`rust-hidden.txt`).
//!
//! The monitoring type crosses as `int`, as the C passes
//! `enum obs_monitoring_type` (int-sized, checked in `obs-source.c.h`); the
//! mixers a handler leaves come in as the `int64_t` the calldata holds; the
//! monitor, route and active outcomes are the C's enums.

use std::os::raw::c_int;

use crate::monitoring::{self as core, Active, Monitor, Route};

/// `enum obs_source_monitoring_monitor`, as the C's enum is returned (an
/// `int` or `unsigned int`; every value fits both).
#[allow(non_camel_case_types)]
pub type obs_source_monitoring_monitor = u32;
/// `OBS_SOURCE_MONITORING_MONITOR_KEEP`.
pub const OBS_SOURCE_MONITORING_MONITOR_KEEP: obs_source_monitoring_monitor = 0;
/// `OBS_SOURCE_MONITORING_MONITOR_CREATE`.
pub const OBS_SOURCE_MONITORING_MONITOR_CREATE: obs_source_monitoring_monitor = 1;
/// `OBS_SOURCE_MONITORING_MONITOR_DESTROY`.
pub const OBS_SOURCE_MONITORING_MONITOR_DESTROY: obs_source_monitoring_monitor = 2;

/// `enum obs_source_monitoring_route`, as the C's enum is returned.
#[allow(non_camel_case_types)]
pub type obs_source_monitoring_route = u32;
/// `OBS_SOURCE_MONITORING_ROUTE_OFF`.
pub const OBS_SOURCE_MONITORING_ROUTE_OFF: obs_source_monitoring_route = 0;
/// `OBS_SOURCE_MONITORING_ROUTE_MONITOR_ONLY`.
pub const OBS_SOURCE_MONITORING_ROUTE_MONITOR_ONLY: obs_source_monitoring_route = 1;
/// `OBS_SOURCE_MONITORING_ROUTE_MONITOR_AND_OUTPUT`.
pub const OBS_SOURCE_MONITORING_ROUTE_MONITOR_AND_OUTPUT: obs_source_monitoring_route = 2;
/// `OBS_SOURCE_MONITORING_ROUTE_ON`.
pub const OBS_SOURCE_MONITORING_ROUTE_ON: obs_source_monitoring_route = 3;

/// `enum obs_source_monitoring_active`, as the C's enum is returned.
#[allow(non_camel_case_types)]
pub type obs_source_monitoring_active = u32;
/// `OBS_SOURCE_MONITORING_ACTIVE_NONE`.
pub const OBS_SOURCE_MONITORING_ACTIVE_NONE: obs_source_monitoring_active = 0;
/// `OBS_SOURCE_MONITORING_ACTIVE_ACTIVATE`.
pub const OBS_SOURCE_MONITORING_ACTIVE_ACTIVATE: obs_source_monitoring_active = 1;
/// `OBS_SOURCE_MONITORING_ACTIVE_DEACTIVATE`.
pub const OBS_SOURCE_MONITORING_ACTIVE_DEACTIVATE: obs_source_monitoring_active = 2;

const fn monitor_value(monitor: Monitor) -> obs_source_monitoring_monitor {
    match monitor {
        Monitor::Keep => OBS_SOURCE_MONITORING_MONITOR_KEEP,
        Monitor::Create => OBS_SOURCE_MONITORING_MONITOR_CREATE,
        Monitor::Destroy => OBS_SOURCE_MONITORING_MONITOR_DESTROY,
    }
}

const fn route_value(route: Route) -> obs_source_monitoring_route {
    match route {
        Route::Off => OBS_SOURCE_MONITORING_ROUTE_OFF,
        Route::MonitorOnly => OBS_SOURCE_MONITORING_ROUTE_MONITOR_ONLY,
        Route::MonitorAndOutput => OBS_SOURCE_MONITORING_ROUTE_MONITOR_AND_OUTPUT,
        Route::On => OBS_SOURCE_MONITORING_ROUTE_ON,
    }
}

const fn active_value(active: Active) -> obs_source_monitoring_active {
    match active {
        Active::None => OBS_SOURCE_MONITORING_ACTIVE_NONE,
        Active::Activate => OBS_SOURCE_MONITORING_ACTIVE_ACTIVATE,
        Active::Deactivate => OBS_SOURCE_MONITORING_ACTIVE_DEACTIVATE,
    }
}

/// `bool obs_source_monitoring_changes(uint32_t cur, uint32_t value)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_monitoring_changes(cur: u32, value: u32) -> bool {
    core::changes(cur, value)
}

/// `bool obs_source_monitoring_has_audio(uint32_t output_flags)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_monitoring_has_audio(output_flags: u32) -> bool {
    core::has_audio(output_flags)
}

/// `uint32_t obs_source_monitoring_mixers_store(int64_t mixers)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_monitoring_mixers_store(mixers: i64) -> u32 {
    core::mixers_store(mixers)
}

/// `bool obs_source_monitoring_push_to_logs(uint32_t output_flags, bool
/// changed)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_monitoring_push_to_logs(output_flags: u32, changed: bool) -> bool {
    core::push_to_logs(output_flags, changed)
}

/// `int obs_source_monitoring_on_type(bool muted)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_monitoring_on_type(muted: bool) -> c_int {
    core::on_type(muted)
}

/// `int obs_source_monitoring_stored_type(bool enabled)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_monitoring_stored_type(enabled: bool) -> c_int {
    core::stored_type(enabled)
}

/// `enum obs_source_monitoring_monitor obs_source_monitoring_monitor(bool
/// was_on, bool enabled)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_monitoring_monitor(
    was_on: bool,
    enabled: bool,
) -> obs_source_monitoring_monitor {
    monitor_value(core::monitor(was_on, enabled))
}

/// `enum obs_source_monitoring_route obs_source_monitoring_route(int
/// type)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_monitoring_route(
    monitoring_type: c_int,
) -> obs_source_monitoring_route {
    route_value(core::route(monitoring_type))
}

/// `bool obs_source_monitoring_off_reads_mute(bool
/// muted_for_monitor_only)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_monitoring_off_reads_mute(muted_for_monitor_only: bool) -> bool {
    core::off_reads_mute(muted_for_monitor_only)
}

/// `bool obs_source_monitoring_off_unmutes(bool muted)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_monitoring_off_unmutes(muted: bool) -> bool {
    core::off_unmutes(muted)
}

/// `bool obs_source_monitoring_only_mutes(bool muted)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_monitoring_only_mutes(muted: bool) -> bool {
    core::only_mutes(muted)
}

/// `float obs_source_monitoring_balance(float balance)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_monitoring_balance(balance: f32) -> f32 {
    core::balance(balance)
}

/// `enum obs_source_monitoring_active obs_source_monitoring_audio_active(bool
/// prev, bool active)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_monitoring_audio_active(
    prev: bool,
    active: bool,
) -> obs_source_monitoring_active {
    active_value(core::audio_active(prev, active))
}
