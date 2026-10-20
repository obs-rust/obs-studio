//! Safe core of `libobs/audio-monitoring/pulse/pulseaudio-stream-flags.c.h`:
//! the PulseAudio monitoring stream's flags, when a sink event reopens it,
//! and how much queued audio to drop after a stall.
//!
//! No libpulse here: the values below are the ones `<pulse/def.h>` gives
//! the C, which the layout test compares against the oracle.

/// `PA_STREAM_START_CORKED`.
pub const STREAM_START_CORKED: u32 = 0x0001;
/// `PA_STREAM_INTERPOLATE_TIMING`.
pub const STREAM_INTERPOLATE_TIMING: u32 = 0x0002;
/// `PA_STREAM_AUTO_TIMING_UPDATE`.
pub const STREAM_AUTO_TIMING_UPDATE: u32 = 0x0008;
/// `PA_STREAM_DONT_MOVE`.
pub const STREAM_DONT_MOVE: u32 = 0x0200;

/// `PA_SUBSCRIPTION_EVENT_SINK`: the sink facility.
pub const SUBSCRIPTION_EVENT_SINK: u32 = 0x0000;
/// `PA_SUBSCRIPTION_EVENT_SOURCE`.
pub const SUBSCRIPTION_EVENT_SOURCE: u32 = 0x0001;
/// `PA_SUBSCRIPTION_EVENT_SINK_INPUT`.
pub const SUBSCRIPTION_EVENT_SINK_INPUT: u32 = 0x0002;
/// `PA_SUBSCRIPTION_EVENT_FACILITY_MASK`.
pub const SUBSCRIPTION_EVENT_FACILITY_MASK: u32 = 0x000F;
/// `PA_SUBSCRIPTION_EVENT_NEW`.
pub const SUBSCRIPTION_EVENT_NEW: u32 = 0x0000;
/// `PA_SUBSCRIPTION_EVENT_CHANGE`.
pub const SUBSCRIPTION_EVENT_CHANGE: u32 = 0x0010;
/// `PA_SUBSCRIPTION_EVENT_REMOVE`.
pub const SUBSCRIPTION_EVENT_REMOVE: u32 = 0x0020;
/// `PA_SUBSCRIPTION_EVENT_TYPE_MASK`.
pub const SUBSCRIPTION_EVENT_TYPE_MASK: u32 = 0x0030;

/// `PA_INVALID_INDEX`: no sink.
pub const INVALID_INDEX: u32 = u32::MAX;

/// `PULSEAUDIO_MONITOR_BACKLOG_WINDOW`, in ns.
pub const BACKLOG_WINDOW_NS: u64 = 2_000_000_000;
/// `PULSEAUDIO_MONITOR_BACKLOG_SLACK`, in usec.
pub const BACKLOG_SLACK_USEC: u64 = 200_000;

/// `pulseaudio_monitor_stream_flags`: the timing and corked flags, plus
/// `PA_STREAM_DONT_MOVE` unless the device is `"default"`.
///
/// `device_id` is the C string without its NUL; `None` is a null pointer,
/// which C treats like a chosen sink.
#[must_use]
pub fn stream_flags(device_id: Option<&[u8]>) -> u32 {
    let _ = device_id;
    todo!()
}

/// `pulseaudio_monitor_reset_on_sink_event`: whether a subscription event
/// means the monitoring stream must be reopened.
#[must_use]
pub fn reset_on_sink_event(event: u32, event_idx: u32, sink_idx: u32, fallback: bool) -> bool {
    let _ = (event, event_idx, sink_idx, fallback);
    todo!()
}

/// `struct pulseaudio_monitor_backlog`: the stall detector's window.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Backlog {
    /// ns; 0 until the first sample of a window.
    pub window_start: u64,
    /// usec; lowest `queued_usec` in the window.
    pub window_min: u64,
}

impl Backlog {
    /// `pulseaudio_monitor_backlog_excess`: feed one sample per write and
    /// get how much queued audio to drop, in usec (0: nothing).
    pub fn excess(&mut self, now_ns: u64, queued_usec: u64, target_usec: u64) -> u64 {
        let _ = (now_ns, queued_usec, target_usec);
        todo!()
    }
}
