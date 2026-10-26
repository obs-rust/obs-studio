//! Safe core of `libobs/audio-monitoring/pulse/pulseaudio-monitor-steps.c.h`:
//! the PulseAudio monitor's open and write steps of `pulseaudio-output.c`.
//!
//! One function per decision, each asked where the C makes it.
//! `pulseaudio-output.c` keeps `pulseaudio_lock()` and `_unlock()`, the
//! playback mutex (and its trylock in the capture callback), the monitoring
//! mutex, every read of the monitor, the source and the settings, every
//! `pa_*` call, `pulseaudio_get_server_info()` and `_get_sink_info()`,
//! `get_default_id()`, `devices_match()`, the resampler, the deque, the
//! buffer attributes it writes, the task queue, every allocation and every
//! log. It passes their results here. These keep no state, take no lock and
//! call nothing in PulseAudio or libobs.
//!
//! - `do_stream_write()`: [`grow`] and [`grown_tlength`] (the server buffer
//!   grows to what is queued), [`uncork`] (a corked stream starts once a
//!   whole tlength is queued), [`write_more`] (the write loop) and
//!   [`write_short`] (the server asks for more than is queued: cancel and
//!   wait).
//! - `on_audio_playback()`: [`playback_idle`] (a source with no activate
//!   refs is skipped), [`playback_bytes`] and [`needs_volume`] (the volume
//!   step runs unless the volume is 1 within [`EPSILON`]).
//! - `pulseaudio_sink_info()`: [`sink_result`].
//! - `audio_monitor_init()`: [`has_device`], [`self_monitor_check`],
//!   [`is_default`], [`fallback`] (the chosen sink is gone: play on the
//!   default), [`sink_named`], [`format_invalid`], [`tlength_usec`] (25 ms)
//!   and [`open_flags`] (the fallback stream follows the default sink).
//! - `on_sink_event()`: [`event_queued`].
//! - `audio_monitor_reset()`: [`context_good`].
//! - `audio_monitor_destroy()`: [`last`].
//!
//! Sizes, counts and ids come from libobs, the server and the user's
//! settings; nothing here can panic on any input.

/// `OBS_SOURCE_DO_NOT_SELF_MONITOR` (`obs-source.h`).
pub const DO_NOT_SELF_MONITOR: u32 = 1 << 9;
/// `EPSILON` (`graphics/math-defs.h`).
pub const EPSILON: f32 = 1e-4;
/// The stream's target buffer, in usec.
pub const TLENGTH_USEC: u64 = 25_000;

/// `PA_CONTEXT_UNCONNECTED`.
pub const CONTEXT_UNCONNECTED: u32 = 0;
/// `PA_CONTEXT_CONNECTING`.
pub const CONTEXT_CONNECTING: u32 = 1;
/// `PA_CONTEXT_AUTHORIZING`.
pub const CONTEXT_AUTHORIZING: u32 = 2;
/// `PA_CONTEXT_SETTING_NAME`.
pub const CONTEXT_SETTING_NAME: u32 = 3;
/// `PA_CONTEXT_READY`.
pub const CONTEXT_READY: u32 = 4;
/// `PA_CONTEXT_FAILED`.
pub const CONTEXT_FAILED: u32 = 5;
/// `PA_CONTEXT_TERMINATED`.
pub const CONTEXT_TERMINATED: u32 = 6;

/// `enum pulseaudio_monitor_sink_result`: what a sink info callback is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum SinkResult {
    /// `eol` is 0: a sink.
    Info = 0,
    /// `eol` is above 0: the end of the list.
    End = 1,
    /// `eol` is below 0: an error; the format becomes `PA_SAMPLE_INVALID`.
    Error = 2,
}

/// `do_stream_write()`: more than twice `tlength` is queued, so the server
/// buffer grows to match. `tlength * 2` wraps as the C's `uint32_t` product.
#[must_use]
pub fn grow(queued: usize, tlength: u32) -> bool {
    let _ = (queued, tlength);
    todo!("pulse_monitor::grow")
}

/// `do_stream_write()`: the tlength the buffer grows to, the low 32 bits of
/// what is queued (the C's `uint32_t` conversion).
#[must_use]
pub fn grown_tlength(queued: usize) -> u32 {
    let _ = queued;
    todo!("pulse_monitor::grown_tlength")
}

/// `do_stream_write()`, for a corked stream: a whole `tlength` is queued, so
/// it is uncorked.
#[must_use]
pub fn uncork(queued: usize, tlength: u32) -> bool {
    let _ = (queued, tlength);
    todo!("pulse_monitor::uncork")
}

/// `do_stream_write()`: something is queued, so the write loop runs again.
#[must_use]
pub fn write_more(queued: usize) -> bool {
    let _ = queued;
    todo!("pulse_monitor::write_more")
}

/// `do_stream_write()`: the server asked for more than is queued, so the
/// write is cancelled and the stream waits.
#[must_use]
pub fn write_short(asked: usize, queued: usize) -> bool {
    let _ = (asked, queued);
    todo!("pulse_monitor::write_short")
}

/// `on_audio_playback()`: the source has no activate refs, so the packet is
/// skipped. A negative count is not idle.
#[must_use]
pub fn playback_idle(activate_refs: i64) -> bool {
    let _ = activate_refs;
    todo!("pulse_monitor::playback_idle")
}

/// `on_audio_playback()`: `bytes_per_frame * frames` in `uint_fast32_t`
/// (`wide`: 64 bits, as on glibc; otherwise 32, as on FreeBSD), wrapping,
/// then as a `size_t` (its low bits).
#[must_use]
pub fn playback_bytes(bytes_per_frame: u64, frames: u32, wide: bool) -> usize {
    let _ = (bytes_per_frame, frames, wide);
    todo!("pulse_monitor::playback_bytes")
}

/// `on_audio_playback()`: the volume is not 1 within [`EPSILON`]
/// (`!close_float(vol, 1.0f, EPSILON)`), so it is applied. NaN is not close.
#[must_use]
pub fn needs_volume(vol: f32) -> bool {
    let _ = vol;
    todo!("pulse_monitor::needs_volume")
}

/// `pulseaudio_sink_info()`: what the callback with this `eol` is.
#[must_use]
pub fn sink_result(eol: i32) -> SinkResult {
    let _ = eol;
    todo!("pulse_monitor::sink_result")
}

/// `audio_monitor_init()`: a monitoring device is set (`None` is NULL).
#[must_use]
pub fn has_device(id: Option<&[u8]>) -> bool {
    let _ = id;
    todo!("pulse_monitor::has_device")
}

/// `audio_monitor_init()`: the source has `OBS_SOURCE_DO_NOT_SELF_MONITOR`.
#[must_use]
pub fn self_monitor_check(output_flags: u32) -> bool {
    let _ = output_flags;
    todo!("pulse_monitor::self_monitor_check")
}

/// `audio_monitor_init()`: the monitoring device is `"default"`.
#[must_use]
pub fn is_default(id: &[u8]) -> bool {
    let _ = id;
    todo!("pulse_monitor::is_default")
}

/// `audio_monitor_init()`: the chosen sink gave no format and the device is
/// not `"default"`, so the stream falls back to the default sink.
#[must_use]
pub fn fallback(format: i32, id: &[u8]) -> bool {
    let _ = (format, id);
    todo!("pulse_monitor::fallback")
}

/// `audio_monitor_init()`, on the fallback: the default sink's name is set
/// and not empty.
#[must_use]
pub fn sink_named(device: Option<&[u8]>) -> bool {
    let _ = device;
    todo!("pulse_monitor::sink_named")
}

/// `audio_monitor_init()`: no sink gave a format (`PA_SAMPLE_INVALID`).
#[must_use]
pub fn format_invalid(format: i32) -> bool {
    let _ = format;
    todo!("pulse_monitor::format_invalid")
}

/// `audio_monitor_init()`: the stream's target buffer, [`TLENGTH_USEC`].
#[must_use]
pub fn tlength_usec() -> u64 {
    todo!("pulse_monitor::tlength_usec")
}

/// `audio_monitor_init()`: the stream's flags, those of `"default"` on the
/// fallback and of `id` otherwise ([`crate::stream_flags`]).
#[must_use]
pub fn open_flags(fallback: bool, id: Option<&[u8]>) -> u32 {
    let _ = (fallback, id);
    todo!("pulse_monitor::open_flags")
}

/// `on_sink_event()`: the event's type is new or remove, so it is queued.
#[must_use]
pub fn event_queued(event: u32) -> bool {
    let _ = event;
    todo!("pulse_monitor::event_queued")
}

/// `audio_monitor_reset()`: `PA_CONTEXT_IS_GOOD(state)`: connecting,
/// authorizing, setting the name or ready.
#[must_use]
pub fn context_good(state: u32) -> bool {
    let _ = state;
    todo!("pulse_monitor::context_good")
}

/// `audio_monitor_destroy()`: no monitor is left, so the sink events stop.
#[must_use]
pub fn last(num: usize) -> bool {
    let _ = num;
    todo!("pulse_monitor::last")
}
