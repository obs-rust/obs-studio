//! C ABI for `libobs/audio-monitoring/pulse/pulseaudio-stream-flags.c.h`.
//!
//! `pulseaudio-output.c` calls these inside libobs. The C file has no
//! `EXPORT`, so the symbols stay hidden (`rust-hidden.txt`). The enums
//! `pa_stream_flags_t` and `pa_subscription_event_type_t` have only
//! non-negative values below 2^32, so C passes them as `unsigned int`.
//!
//! Only Linux and FreeBSD with `ENABLE_PULSEAUDIO` call them. Elsewhere the
//! shims are built but unused, and hidden all the same.

use std::ffi::{CStr, c_char, c_uint};

use crate::pulse_stream_flags::{Backlog, reset_on_sink_event, stream_flags};

/// `struct pulseaudio_monitor_backlog` in `pulseaudio-stream-flags.h`.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub struct pulseaudio_monitor_backlog {
    pub window_start: u64,
    pub window_min: u64,
}

/// Same contract as C `pulseaudio_monitor_stream_flags`.
///
/// # Safety
///
/// `device_id` is null or a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pulseaudio_monitor_stream_flags(device_id: *const c_char) -> c_uint {
    let id = if device_id.is_null() {
        None
    } else {
        // SAFETY: the caller passes a NUL-terminated string.
        Some(unsafe { CStr::from_ptr(device_id) }.to_bytes())
    };
    stream_flags(id)
}

/// Same contract as C `pulseaudio_monitor_reset_on_sink_event`.
#[unsafe(no_mangle)]
pub extern "C" fn pulseaudio_monitor_reset_on_sink_event(
    event: c_uint,
    event_idx: u32,
    sink_idx: u32,
    fallback: bool,
) -> bool {
    reset_on_sink_event(event, event_idx, sink_idx, fallback)
}

/// Same contract as C `pulseaudio_monitor_backlog_excess`. A null
/// `backlog` returns 0 (C dereferences it).
///
/// # Safety
///
/// `backlog` is null or points to a readable and writable
/// `struct pulseaudio_monitor_backlog`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pulseaudio_monitor_backlog_excess(
    backlog: *mut pulseaudio_monitor_backlog,
    now_ns: u64,
    queued_usec: u64,
    target_usec: u64,
) -> u64 {
    // SAFETY: the caller passes null or a valid, exclusive pointer.
    let Some(c) = (unsafe { backlog.as_mut() }) else {
        return 0;
    };
    let mut state = Backlog {
        window_start: c.window_start,
        window_min: c.window_min,
    };
    let drop = state.excess(now_ns, queued_usec, target_usec);
    c.window_start = state.window_start;
    c.window_min = state.window_min;
    drop
}
