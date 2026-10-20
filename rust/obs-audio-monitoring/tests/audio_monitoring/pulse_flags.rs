//! Tier 1 mirror of `test/cmocka/test_pulse_monitor_flags.c`, plus edge
//! cases. The C test compiles `pulseaudio-stream-flags.c.h` into itself, so
//! the Rust swap is invisible to it; parity is in `pulse_parity`.

use obs_audio_monitoring::pulse_stream_flags::{
    INVALID_INDEX, STREAM_AUTO_TIMING_UPDATE, STREAM_DONT_MOVE, STREAM_INTERPOLATE_TIMING,
    STREAM_START_CORKED, SUBSCRIPTION_EVENT_CHANGE, SUBSCRIPTION_EVENT_FACILITY_MASK,
    SUBSCRIPTION_EVENT_NEW, SUBSCRIPTION_EVENT_REMOVE, SUBSCRIPTION_EVENT_SINK,
    SUBSCRIPTION_EVENT_SINK_INPUT, SUBSCRIPTION_EVENT_SOURCE, SUBSCRIPTION_EVENT_TYPE_MASK,
};
use obs_audio_monitoring::{reset_on_sink_event, stream_flags};

/// `BASE_FLAGS` in the C test.
const BASE_FLAGS: u32 = STREAM_INTERPOLATE_TIMING | STREAM_AUTO_TIMING_UPDATE | STREAM_START_CORKED;

const SINK_NEW: u32 = SUBSCRIPTION_EVENT_SINK | SUBSCRIPTION_EVENT_NEW;
const SINK_CHANGE: u32 = SUBSCRIPTION_EVENT_SINK | SUBSCRIPTION_EVENT_CHANGE;
const SINK_REMOVE: u32 = SUBSCRIPTION_EVENT_SINK | SUBSCRIPTION_EVENT_REMOVE;
const SINK_INPUT_REMOVE: u32 = SUBSCRIPTION_EVENT_SINK_INPUT | SUBSCRIPTION_EVENT_REMOVE;

fn check_base_flags(id: &str, flags: u32) {
    assert_eq!(
        flags & BASE_FLAGS,
        BASE_FLAGS,
        "monitoring stream for '{id}' lost its timing/corked flags: {flags:#x}"
    );
}

/// `pulse/def.h` values, as libpulse has had them since 0.9.
#[test]
fn pulse_def_values() {
    assert_eq!(STREAM_START_CORKED, 0x0001);
    assert_eq!(STREAM_INTERPOLATE_TIMING, 0x0002);
    assert_eq!(STREAM_AUTO_TIMING_UPDATE, 0x0008);
    assert_eq!(STREAM_DONT_MOVE, 0x0200);
    assert_eq!(SUBSCRIPTION_EVENT_SINK, 0x0000);
    assert_eq!(SUBSCRIPTION_EVENT_SOURCE, 0x0001);
    assert_eq!(SUBSCRIPTION_EVENT_SINK_INPUT, 0x0002);
    assert_eq!(SUBSCRIPTION_EVENT_FACILITY_MASK, 0x000F);
    assert_eq!(SUBSCRIPTION_EVENT_NEW, 0x0000);
    assert_eq!(SUBSCRIPTION_EVENT_CHANGE, 0x0010);
    assert_eq!(SUBSCRIPTION_EVENT_REMOVE, 0x0020);
    assert_eq!(SUBSCRIPTION_EVENT_TYPE_MASK, 0x0030);
    assert_eq!(INVALID_INDEX, u32::MAX);
}

#[test]
fn chosen_sink_is_not_moved() {
    for id in [
        "alsa_output.usb-headphones.analog-stereo",
        "obs_record",
        "sinkB",
    ] {
        let flags = stream_flags(Some(id.as_bytes()));
        check_base_flags(id, flags);
        assert_ne!(
            flags & STREAM_DONT_MOVE,
            0,
            "monitoring stream for chosen sink '{id}' lacks PA_STREAM_DONT_MOVE (flags {flags:#x})"
        );
    }
}

#[test]
fn default_device_follows_default_sink() {
    let flags = stream_flags(Some(b"default"));
    check_base_flags("default", flags);
    assert_eq!(
        flags & STREAM_DONT_MOVE,
        0,
        "monitoring stream for 'default' must follow the default sink (flags {flags:#x})"
    );
}

/// Exactly the base flags for `"default"`, base plus `DONT_MOVE` otherwise.
#[test]
fn flags_are_exact() {
    assert_eq!(stream_flags(Some(b"default")), BASE_FLAGS);
    assert_eq!(stream_flags(Some(b"sinkB")), BASE_FLAGS | STREAM_DONT_MOVE);
}

/// A null id (C `!device_id`) and anything that is not exactly `"default"`
/// can't move: strcmp compares the whole string.
#[test]
fn only_exact_default_follows() {
    for id in [
        None,
        Some(&b""[..]),
        Some(b"Default"),
        Some(b"default "),
        Some(b"defaul"),
        Some(b"defaults"),
        Some(b" default"),
    ] {
        assert_eq!(
            stream_flags(id),
            BASE_FLAGS | STREAM_DONT_MOVE,
            "{:?}",
            id.map(String::from_utf8_lossy)
        );
    }
}

fn check_reset(what: &str, want: bool, event: u32, event_idx: u32, sink_idx: u32, fallback: bool) {
    let got = reset_on_sink_event(event, event_idx, sink_idx, fallback);
    assert_eq!(
        got,
        want,
        "{what}: monitoring stream {} be reopened (event {event:#x} for sink {event_idx}, on sink {sink_idx}{})",
        if want { "must" } else { "must not" },
        if fallback { ", fallback" } else { "" }
    );
}

#[test]
fn unplugged_sink_falls_back_to_default() {
    check_reset("chosen sink removed", true, SINK_REMOVE, 7, 7, false);
}

#[test]
fn replugged_sink_is_used_again() {
    check_reset(
        "sink added while on the fallback",
        true,
        SINK_NEW,
        9,
        3,
        true,
    );
}

#[test]
fn unrelated_sink_events_are_ignored() {
    check_reset("another sink removed", false, SINK_REMOVE, 8, 7, false);
    check_reset(
        "sink added while on the chosen sink",
        false,
        SINK_NEW,
        9,
        7,
        false,
    );
    check_reset("chosen sink changed", false, SINK_CHANGE, 7, 7, false);
    // The fallback stream can move, the server takes care of it.
    check_reset("fallback sink removed", false, SINK_REMOVE, 3, 3, true);
    check_reset("sink input removed", false, SINK_INPUT_REMOVE, 7, 7, false);
    check_reset("no stream", false, SINK_REMOVE, 7, INVALID_INDEX, false);
}

/// Bits outside both masks are ignored; the type mask's fourth value (0x30)
/// matches no case. Characterized, not endorsed: a removal of index
/// `PA_INVALID_INDEX` never resets, even when `sink_idx` is unknown too.
#[test]
fn event_edges() {
    check_reset("high bits", true, SINK_REMOVE | 0xFF00, 7, 7, false);
    check_reset("high bits on new", true, SINK_NEW | 0x8000_0000, 1, 2, true);
    check_reset(
        "type 0x30",
        false,
        SUBSCRIPTION_EVENT_TYPE_MASK,
        7,
        7,
        false,
    );
    check_reset(
        "type 0x30 fallback",
        false,
        SUBSCRIPTION_EVENT_TYPE_MASK,
        7,
        7,
        true,
    );
    check_reset(
        "source removed",
        false,
        SUBSCRIPTION_EVENT_SOURCE | SUBSCRIPTION_EVENT_REMOVE,
        7,
        7,
        false,
    );
    check_reset(
        "source new on fallback",
        false,
        SUBSCRIPTION_EVENT_SOURCE,
        7,
        7,
        true,
    );
    check_reset(
        "facility 0xf",
        false,
        0x000F | SUBSCRIPTION_EVENT_NEW,
        7,
        7,
        true,
    );
    check_reset(
        "invalid both",
        false,
        SINK_REMOVE,
        INVALID_INDEX,
        INVALID_INDEX,
        false,
    );
    check_reset(
        "new ignores indexes",
        true,
        SINK_NEW,
        INVALID_INDEX,
        INVALID_INDEX,
        true,
    );
    check_reset("change on fallback", false, SINK_CHANGE, 3, 3, true);
}
