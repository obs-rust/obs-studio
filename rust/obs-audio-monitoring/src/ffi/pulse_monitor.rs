//! C ABI for `libobs/audio-monitoring/pulse/pulseaudio-monitor-steps.c.h`.
//!
//! `pulseaudio-output.c` calls these inside libobs. The C file has no
//! `EXPORT`, so the symbols stay hidden (`rust-hidden.txt`). The twenty
//! functions keep the C signatures: `pa_sample_format_t` (which has
//! `PA_SAMPLE_INVALID`, -1) crosses as `int`; `pa_stream_flags_t`,
//! `pa_subscription_event_type_t` and `pa_context_state_t` have only
//! non-negative values below 2^32, so they cross as `unsigned int`, as does
//! `enum pulseaudio_monitor_sink_result` (0 to 2) on the way back. `long`
//! (activate refs) is widened to `i64`, and `uint_fast32_t` is the host's
//! (`pulse_format`'s alias). The id strings are read only up to their NUL.
//!
//! Only Linux and FreeBSD with `ENABLE_PULSEAUDIO` call them. Elsewhere the
//! shims are built but unused, and hidden all the same.

use std::ffi::{CStr, c_char, c_int, c_long, c_uint};

use crate::ffi::pulse_format::uint_fast32_t;
use crate::pulse_monitor::{
    context_good, event_queued, fallback, format_invalid, grow, grown_tlength, has_device,
    is_default, last, needs_volume, open_flags, playback_bytes, playback_idle, self_monitor_check,
    sink_named, sink_result, tlength_usec, uncork, write_more, write_short,
};

/// `uint_fast32_t` is 64 bits wide on this host.
const FAST32_WIDE: bool = size_of::<uint_fast32_t>() == size_of::<u64>();

/// The bytes of a NUL-terminated string, or `None` for NULL.
///
/// # Safety
///
/// `s` is null or a NUL-terminated string that outlives the result.
unsafe fn c_str<'a>(s: *const c_char) -> Option<&'a [u8]> {
    if s.is_null() {
        None
    } else {
        // SAFETY: the caller passes a NUL-terminated string.
        Some(unsafe { CStr::from_ptr(s) }.to_bytes())
    }
}

/// Same contract as C `pulseaudio_monitor_grow`.
#[unsafe(no_mangle)]
pub extern "C" fn pulseaudio_monitor_grow(queued: usize, tlength: u32) -> bool {
    grow(queued, tlength)
}

/// Same contract as C `pulseaudio_monitor_grown_tlength`.
#[unsafe(no_mangle)]
pub extern "C" fn pulseaudio_monitor_grown_tlength(queued: usize) -> u32 {
    grown_tlength(queued)
}

/// Same contract as C `pulseaudio_monitor_uncork`.
#[unsafe(no_mangle)]
pub extern "C" fn pulseaudio_monitor_uncork(queued: usize, tlength: u32) -> bool {
    uncork(queued, tlength)
}

/// Same contract as C `pulseaudio_monitor_write_more`.
#[unsafe(no_mangle)]
pub extern "C" fn pulseaudio_monitor_write_more(queued: usize) -> bool {
    write_more(queued)
}

/// Same contract as C `pulseaudio_monitor_write_short`.
#[unsafe(no_mangle)]
pub extern "C" fn pulseaudio_monitor_write_short(asked: usize, queued: usize) -> bool {
    write_short(asked, queued)
}

/// Same contract as C `pulseaudio_monitor_playback_idle`.
// `long` is `i64` on Linux and macOS and `i32` on Windows.
#[allow(clippy::useless_conversion)]
#[unsafe(no_mangle)]
pub extern "C" fn pulseaudio_monitor_playback_idle(activate_refs: c_long) -> bool {
    playback_idle(i64::from(activate_refs))
}

/// Same contract as C `pulseaudio_monitor_playback_bytes`.
// `uint_fast32_t` is `u64` on 64-bit glibc and `u32` elsewhere.
#[allow(clippy::useless_conversion)]
#[unsafe(no_mangle)]
pub extern "C" fn pulseaudio_monitor_playback_bytes(
    bytes_per_frame: uint_fast32_t,
    frames: u32,
) -> usize {
    playback_bytes(u64::from(bytes_per_frame), frames, FAST32_WIDE)
}

/// Same contract as C `pulseaudio_monitor_needs_volume`.
#[unsafe(no_mangle)]
pub extern "C" fn pulseaudio_monitor_needs_volume(vol: f32) -> bool {
    needs_volume(vol)
}

/// Same contract as C `pulseaudio_monitor_sink_result`.
#[unsafe(no_mangle)]
pub extern "C" fn pulseaudio_monitor_sink_result(eol: c_int) -> c_uint {
    sink_result(eol) as c_uint
}

/// Same contract as C `pulseaudio_monitor_has_device`.
///
/// # Safety
///
/// `id` is null or a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pulseaudio_monitor_has_device(id: *const c_char) -> bool {
    // SAFETY: as the caller promises.
    has_device(unsafe { c_str(id) })
}

/// Same contract as C `pulseaudio_monitor_self_monitor_check`.
#[unsafe(no_mangle)]
pub extern "C" fn pulseaudio_monitor_self_monitor_check(output_flags: u32) -> bool {
    self_monitor_check(output_flags)
}

/// Same contract as C `pulseaudio_monitor_is_default`. A null `id`, which
/// the C dereferences, is not `"default"`.
///
/// # Safety
///
/// `id` is null or a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pulseaudio_monitor_is_default(id: *const c_char) -> bool {
    // SAFETY: as the caller promises.
    unsafe { c_str(id) }.is_some_and(is_default)
}

/// Same contract as C `pulseaudio_monitor_fallback`. A null `id`, which the
/// C dereferences when the format is invalid, does not fall back.
///
/// # Safety
///
/// `id` is null or a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pulseaudio_monitor_fallback(format: c_int, id: *const c_char) -> bool {
    // SAFETY: as the caller promises.
    unsafe { c_str(id) }.is_some_and(|id| fallback(format, id))
}

/// Same contract as C `pulseaudio_monitor_sink_named`.
///
/// # Safety
///
/// `device` is null or a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pulseaudio_monitor_sink_named(device: *const c_char) -> bool {
    // SAFETY: as the caller promises.
    sink_named(unsafe { c_str(device) })
}

/// Same contract as C `pulseaudio_monitor_format_invalid`.
#[unsafe(no_mangle)]
pub extern "C" fn pulseaudio_monitor_format_invalid(format: c_int) -> bool {
    format_invalid(format)
}

/// Same contract as C `pulseaudio_monitor_tlength_usec`.
#[unsafe(no_mangle)]
pub extern "C" fn pulseaudio_monitor_tlength_usec() -> u64 {
    tlength_usec()
}

/// Same contract as C `pulseaudio_monitor_open_flags`.
///
/// # Safety
///
/// `id` is null or a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pulseaudio_monitor_open_flags(
    fallback: bool,
    id: *const c_char,
) -> c_uint {
    // SAFETY: as the caller promises.
    open_flags(fallback, unsafe { c_str(id) })
}

/// Same contract as C `pulseaudio_monitor_event_queued`.
#[unsafe(no_mangle)]
pub extern "C" fn pulseaudio_monitor_event_queued(event: c_uint) -> bool {
    event_queued(event)
}

/// Same contract as C `pulseaudio_monitor_context_good`.
#[unsafe(no_mangle)]
pub extern "C" fn pulseaudio_monitor_context_good(state: c_uint) -> bool {
    context_good(state)
}

/// Same contract as C `pulseaudio_monitor_last`.
#[unsafe(no_mangle)]
pub extern "C" fn pulseaudio_monitor_last(num: usize) -> bool {
    last(num)
}
