//! Tier 2 layout and Tier 3 parity: the shims against the original
//! `pulseaudio-stream-flags.c.h` (oracle/pulse_stream_flags.c).
//!
//! The unchanged C tests `test_pulse_monitor_flags` and
//! `test_pulse_monitor_backlog` compile the C into themselves, so they never
//! see the Rust swap; this file is what compares the two.
//!
//! Intentional difference, excluded here: a null `backlog` returns 0 in Rust
//! (C dereferences it). Nothing else is excluded.
//!
//! `<pulse/def.h>` is libpulse's own where installed (the values are then
//! libpulse's), else the oracle's stand-in.

use std::ffi::CString;
use std::mem::{align_of, offset_of, size_of};

use obs_audio_monitoring::ffi::pulse_stream_flags::{
    pulseaudio_monitor_backlog, pulseaudio_monitor_backlog_excess,
    pulseaudio_monitor_reset_on_sink_event, pulseaudio_monitor_stream_flags,
};
use obs_audio_monitoring::pulse_stream_flags::{
    BACKLOG_SLACK_USEC, BACKLOG_WINDOW_NS, INVALID_INDEX, STREAM_AUTO_TIMING_UPDATE,
    STREAM_DONT_MOVE, STREAM_INTERPOLATE_TIMING, STREAM_START_CORKED, SUBSCRIPTION_EVENT_CHANGE,
    SUBSCRIPTION_EVENT_FACILITY_MASK, SUBSCRIPTION_EVENT_NEW, SUBSCRIPTION_EVENT_REMOVE,
    SUBSCRIPTION_EVENT_SINK, SUBSCRIPTION_EVENT_SINK_INPUT, SUBSCRIPTION_EVENT_SOURCE,
    SUBSCRIPTION_EVENT_TYPE_MASK,
};
use obs_c_oracle::pulse_stream_flags::{
    DEF_NAMES, OracleBacklog, oracle_pulse_backlog_layout, oracle_pulse_backlog_slack,
    oracle_pulse_backlog_window, oracle_pulse_def_is_real, oracle_pulse_def_value,
    oracle_pulseaudio_monitor_backlog_excess, oracle_pulseaudio_monitor_reset_on_sink_event,
    oracle_pulseaudio_monitor_stream_flags,
};
use proptest::prelude::*;

#[test]
fn pulse_def_values_match_oracle() {
    let rust = [
        STREAM_START_CORKED,
        STREAM_INTERPOLATE_TIMING,
        STREAM_AUTO_TIMING_UPDATE,
        STREAM_DONT_MOVE,
        SUBSCRIPTION_EVENT_SINK,
        SUBSCRIPTION_EVENT_SOURCE,
        SUBSCRIPTION_EVENT_SINK_INPUT,
        SUBSCRIPTION_EVENT_FACILITY_MASK,
        SUBSCRIPTION_EVENT_NEW,
        SUBSCRIPTION_EVENT_CHANGE,
        SUBSCRIPTION_EVENT_REMOVE,
        SUBSCRIPTION_EVENT_TYPE_MASK,
        INVALID_INDEX,
    ];
    // SAFETY: plain oracle getters.
    let real = unsafe { oracle_pulse_def_is_real() } != 0;
    for (i, (name, value)) in DEF_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: `i` is below the table length; out of range returns 0.
        let c = unsafe { oracle_pulse_def_value(i) };
        assert_eq!(
            value, c,
            "{name}: Rust {value:#x}, C {c:#x} (libpulse header: {real})"
        );
    }
    // SAFETY: plain oracle getters.
    unsafe {
        assert_eq!(BACKLOG_WINDOW_NS, oracle_pulse_backlog_window());
        assert_eq!(BACKLOG_SLACK_USEC, oracle_pulse_backlog_slack());
    }
}

#[test]
fn backlog_layout_matches_c() {
    // SAFETY: plain oracle getters.
    let c = |which| unsafe { oracle_pulse_backlog_layout(which) };
    assert_eq!(size_of::<pulseaudio_monitor_backlog>(), c(0));
    assert_eq!(align_of::<pulseaudio_monitor_backlog>(), c(1));
    assert_eq!(offset_of!(pulseaudio_monitor_backlog, window_start), c(2));
    assert_eq!(offset_of!(pulseaudio_monitor_backlog, window_min), c(3));
    assert_eq!(size_of::<OracleBacklog>(), c(0));
}

fn check_flags(id: Option<&[u8]>) {
    let owned = id.map(|b| CString::new(b).expect("no interior NUL"));
    let ptr = owned.as_ref().map_or(std::ptr::null(), |s| s.as_ptr());
    // SAFETY: null or a live NUL-terminated string.
    let (rust, c) = unsafe {
        (
            pulseaudio_monitor_stream_flags(ptr),
            oracle_pulseaudio_monitor_stream_flags(ptr),
        )
    };
    assert_eq!(
        rust,
        c,
        "{:?}: rust {rust:#x}, oracle {c:#x}",
        id.map(String::from_utf8_lossy)
    );
}

fn check_reset(event: u32, event_idx: u32, sink_idx: u32, fallback: bool) {
    let rust = pulseaudio_monitor_reset_on_sink_event(event, event_idx, sink_idx, fallback);
    // SAFETY: plain values.
    let c = unsafe {
        oracle_pulseaudio_monitor_reset_on_sink_event(event, event_idx, sink_idx, fallback)
    };
    assert_eq!(
        rust, c,
        "event {event:#x} idx {event_idx} sink {sink_idx} fallback {fallback}"
    );
}

/// Runs `steps` from the same state on both sides and compares every result
/// and the state after each step.
fn check_backlog(start: (u64, u64), steps: &[(u64, u64, u64)]) {
    let mut rust = pulseaudio_monitor_backlog {
        window_start: start.0,
        window_min: start.1,
    };
    let mut c = OracleBacklog {
        window_start: start.0,
        window_min: start.1,
    };
    for (i, &(now, queued, target)) in steps.iter().enumerate() {
        // SAFETY: both point to live, exclusive locals.
        let (r, o) = unsafe {
            (
                pulseaudio_monitor_backlog_excess(&raw mut rust, now, queued, target),
                oracle_pulseaudio_monitor_backlog_excess(&raw mut c, now, queued, target),
            )
        };
        assert_eq!(
            (r, rust.window_start, rust.window_min),
            (o, c.window_start, c.window_min),
            "step {i} ({now}, {queued}, {target}) from {start:?}"
        );
    }
}

#[test]
fn cmocka_cases_match_oracle() {
    for id in [
        &b"alsa_output.usb-headphones.analog-stereo"[..],
        b"obs_record",
        b"sinkB",
        b"default",
        b"",
        b"Default",
        b"default ",
        b"defaults",
    ] {
        check_flags(Some(id));
    }
    check_flags(None);

    let sink_input = SUBSCRIPTION_EVENT_SINK_INPUT | SUBSCRIPTION_EVENT_REMOVE;
    for (event, event_idx, sink_idx, fallback) in [
        (SUBSCRIPTION_EVENT_REMOVE, 7, 7, false),
        (SUBSCRIPTION_EVENT_NEW, 9, 3, true),
        (SUBSCRIPTION_EVENT_REMOVE, 8, 7, false),
        (SUBSCRIPTION_EVENT_NEW, 9, 7, false),
        (SUBSCRIPTION_EVENT_CHANGE, 7, 7, false),
        (SUBSCRIPTION_EVENT_REMOVE, 3, 3, true),
        (sink_input, 7, 7, false),
        (SUBSCRIPTION_EVENT_REMOVE, 7, INVALID_INDEX, false),
        (SUBSCRIPTION_EVENT_TYPE_MASK, 7, 7, true),
        (
            SUBSCRIPTION_EVENT_REMOVE,
            INVALID_INDEX,
            INVALID_INDEX,
            false,
        ),
    ] {
        check_reset(event, event_idx, sink_idx, fallback);
    }

    let w = BACKLOG_WINDOW_NS;
    check_backlog(
        (0, 0),
        &[
            (1, 900_000, 85_000),
            (w, 900_000, 85_000),
            (w + 1, 700_000, 85_000),
            (w + 2, 100_000, 85_000),
        ],
    );
    check_backlog((0, 0), &[(0, 900_000, 85_000), (3 * w, 100_000, 85_000)]);
    check_backlog((10 * w, 900_000), &[(w, 900_000, 85_000)]);
    check_backlog((1, 5), &[(1 + w, 5, u64::MAX - 10)]);
    check_backlog((1, 3), &[(1 + w, 3, u64::MAX / 2 + 1)]);
    check_backlog((1, 2_000_001), &[(1 + w, 2_000_001, 1_000_000)]);
}

/// Bytes that are often "default" or close to it, else anything without NUL.
fn device_id() -> impl Strategy<Value = Option<Vec<u8>>> {
    let near_default = (0usize..=8, any::<u8>(), any::<bool>()).prop_map(|(cut, byte, append)| {
        let mut id = b"default"[..cut.min(7)].to_vec();
        if append || cut == 8 {
            id.push(byte.max(1));
        }
        id
    });
    prop_oneof![
        1 => Just(None),
        2 => Just(Some(b"default".to_vec())),
        3 => near_default.prop_map(Some),
        4 => prop::collection::vec(1u8..=255, 0..48).prop_map(Some),
    ]
}

/// A facility (sink most often) and a type, sometimes with stray high
/// bits, sometimes any `u32`.
fn event() -> impl Strategy<Value = u32> {
    let facility = prop_oneof![
        3 => Just(SUBSCRIPTION_EVENT_SINK),
        1 => 0u32..=SUBSCRIPTION_EVENT_FACILITY_MASK,
    ];
    let kind = prop::sample::select(vec![
        SUBSCRIPTION_EVENT_NEW,
        SUBSCRIPTION_EVENT_CHANGE,
        SUBSCRIPTION_EVENT_REMOVE,
        SUBSCRIPTION_EVENT_TYPE_MASK,
    ]);
    let high = prop_oneof![3 => Just(0u32), 1 => any::<u32>().prop_map(|b| b & !0x3f)];
    prop_oneof![
        4 => (facility, kind, high).prop_map(|(f, k, h)| f | k | h),
        1 => any::<u32>(),
    ]
}

/// Indexes that collide often, including `PA_INVALID_INDEX`.
fn index() -> impl Strategy<Value = u32> {
    prop_oneof![
        3 => 0u32..4,
        1 => Just(INVALID_INDEX),
        1 => any::<u32>(),
    ]
}

/// A clock that mostly moves forward by about a packet, sometimes jumps or
/// steps back; queue and target sizes around the thresholds or anywhere.
fn backlog_steps() -> impl Strategy<Value = Vec<(u64, u64, u64)>> {
    let delta = prop_oneof![
        6 => 0u64..60_000_000,
        2 => Just(BACKLOG_WINDOW_NS - 1),
        2 => Just(BACKLOG_WINDOW_NS),
        1 => any::<u64>(),
    ];
    let usec = prop_oneof![
        6 => 0u64..3_000_000,
        1 => any::<u64>(),
    ];
    let target = prop_oneof![
        6 => 0u64..1_200_000,
        1 => Just(BACKLOG_SLACK_USEC),
        1 => any::<u64>(),
    ];
    (
        any::<u64>(),
        prop::collection::vec((delta, usec, target), 1..200),
    )
        .prop_map(|(start, steps)| {
            let mut now = start;
            steps
                .into_iter()
                .map(|(d, queued, target)| {
                    now = now.wrapping_add(d);
                    (now, queued, target)
                })
                .collect()
        })
}

proptest! {
    #[test]
    fn random_device_ids_match_oracle(id in device_id()) {
        check_flags(id.as_deref());
    }

    #[test]
    fn random_sink_events_match_oracle(
        event in event(),
        event_idx in index(),
        sink_idx in index(),
        fallback in any::<bool>(),
    ) {
        check_reset(event, event_idx, sink_idx, fallback);
    }

    #[test]
    fn random_backlog_runs_match_oracle(
        start in prop_oneof![
            3 => Just((0u64, 0u64)),
            1 => (any::<u64>(), any::<u64>()),
        ],
        steps in backlog_steps(),
    ) {
        check_backlog(start, &steps);
    }
}

/// The one exclusion: a null backlog. Rust returns 0; C is not called.
#[test]
fn null_backlog_returns_zero() {
    // SAFETY: null is the documented no-op.
    let drop = unsafe { pulseaudio_monitor_backlog_excess(std::ptr::null_mut(), 1, 2, 3) };
    assert_eq!(drop, 0);
}
