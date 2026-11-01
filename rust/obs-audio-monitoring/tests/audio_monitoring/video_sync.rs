//! Tier 1: `obs_audio_monitoring::monitor_delay::track_video_ts`, the
//! WASAPI monitor's video stall tracker.
//!
//! The `sync_*` cases port `test/cmocka/test_monitor_video_sync.c` and the
//! `steps_*` cases the tracker half of `test/cmocka/test_monitor_wasapi_steps.c`,
//! 1:1: the same cases under the same names, in the same order, with the same
//! values and expectations, the "characterized, not endorsed" ones included.
//! The C calls the hidden `audio_monitor_track_video_ts()`; here the safe
//! core is called directly.
//!
//! The `edge_*` cases are new: the core against a model of the origin/master
//! C (its branches inline) over runs of packets from any start.

use obs_audio_monitoring::monitor_delay::{VIDEO_STALL_NS, stalled, track_video_ts};
use proptest::prelude::*;

const MS: u64 = 1_000_000;
/// A 1024-frame packet at 48 kHz.
const PACKET_NS: u64 = 1024 * 1_000_000_000 / 48_000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Tracker {
    prev_video_ts: u64,
    time_since_prev: u64,
}

fn track(t: &mut Tracker, video_ts: u64, packet_ns: u64) {
    track_video_ts(
        &mut t.prev_video_ts,
        &mut t.time_since_prev,
        video_ts,
        packet_ns,
    );
}

// ---- test_monitor_video_sync.c ----

/// The C test's `feed()`: `ns` of packets with the video at `video_ts`.
fn feed(m: &mut Tracker, video_ts: u64, ns: u64) {
    let mut t = 0;
    while t < ns {
        track(m, video_ts, PACKET_NS);
        t += PACKET_NS;
    }
}

fn expect_stalled(m: &Tracker, what: &str) {
    assert!(
        m.time_since_prev > VIDEO_STALL_NS,
        "{what}: video stood still but only {} ms counted (stall after {} ms)",
        m.time_since_prev / MS,
        VIDEO_STALL_NS / MS
    );
}

/// First play: the cover's timestamp stands still, so the audio is played
/// without syncing to video.
#[test]
fn sync_cover_art_first_play_stalls() {
    let mut m = Tracker::default();
    feed(&mut m, 1000 * MS, 500 * MS);
    expect_stalled(&m, "first play");
}

/// The report: the source restarts and the cover gets a new timestamp,
/// which then stands still too.
#[test]
fn sync_cover_art_restart_stalls_again() {
    let mut m = Tracker::default();
    feed(&mut m, 1000 * MS, 500 * MS);
    feed(&mut m, 9000 * MS, 500 * MS);
    expect_stalled(&m, "after a restart");
    assert_eq!(
        m.prev_video_ts,
        9000 * MS,
        "after a restart: the monitor kept the old video timestamp"
    );
}

/// Each restart, and a looped file, again.
#[test]
fn sync_cover_art_every_restart_stalls() {
    let mut m = Tracker::default();
    for i in 1..=4 {
        feed(&mut m, i * 7000 * MS, 300 * MS);
        expect_stalled(&m, "after repeated restarts");
    }
}

/// Guard: real video, a new timestamp every 33 ms, never counts as still.
#[test]
fn sync_moving_video_never_stalls() {
    let mut m = Tracker::default();
    for frame in 1..=300 {
        feed(&mut m, frame * 33 * MS, 33 * MS);
        assert!(
            m.time_since_prev <= VIDEO_STALL_NS,
            "moving video: counted as still at frame {frame}"
        );
    }
}

// ---- test_monitor_wasapi_steps.c, tracker ----

/// The first packet stores the video timestamp and leaves the count.
#[test]
fn steps_first_packet_stores_the_timestamp() {
    let mut t = Tracker::default();
    track(&mut t, 1000 * MS, PACKET_NS);
    assert_eq!(t.prev_video_ts, 1000 * MS);
    assert_eq!(t.time_since_prev, 0);

    // a count already there is left as it is
    t.prev_video_ts = 0;
    t.time_since_prev = 42;
    track(&mut t, 7, PACKET_NS);
    assert_eq!(t.prev_video_ts, 7);
    assert_eq!(t.time_since_prev, 42);
}

/// A video timestamp of 0 is stored as 0, so every packet is the first one
/// again: the count never moves, the video never counts as still.
/// Characterized, not endorsed.
#[test]
fn steps_zero_timestamp_is_always_first() {
    let mut t = Tracker::default();
    for _ in 0..100 {
        track(&mut t, 0, PACKET_NS);
        assert_eq!(t.prev_video_ts, 0);
        assert_eq!(t.time_since_prev, 0);
    }

    // a count already there stays
    t.time_since_prev = 500 * MS;
    track(&mut t, 0, PACKET_NS);
    assert_eq!(t.time_since_prev, 500 * MS);

    // video timestamp 0 after a real one: new, so the count resets and 0 is
    // stored, and the next packet is the first again
    t.prev_video_ts = 9000 * MS;
    t.time_since_prev = 300 * MS;
    track(&mut t, 0, PACKET_NS);
    assert_eq!(t.prev_video_ts, 0);
    assert_eq!(t.time_since_prev, 0);
    track(&mut t, 9000 * MS, PACKET_NS);
    assert_eq!(t.prev_video_ts, 9000 * MS);
    assert_eq!(t.time_since_prev, 0);
}

/// The same timestamp adds the packet's duration, 0 included.
#[test]
fn steps_same_timestamp_counts() {
    let mut t = Tracker::default();
    track(&mut t, 1000 * MS, PACKET_NS);
    track(&mut t, 1000 * MS, PACKET_NS);
    assert_eq!(t.time_since_prev, PACKET_NS);
    track(&mut t, 1000 * MS, 0);
    assert_eq!(t.time_since_prev, PACKET_NS);
    track(&mut t, 1000 * MS, 3);
    assert_eq!(t.time_since_prev, PACKET_NS + 3);
    assert_eq!(t.prev_video_ts, 1000 * MS);
}

/// The count wraps at 2^64. Characterized, not endorsed.
#[test]
fn steps_count_wraps() {
    let mut t = Tracker {
        prev_video_ts: 5,
        time_since_prev: u64::MAX - 1,
    };
    track(&mut t, 5, 3);
    assert_eq!(t.time_since_prev, 1);
    assert_eq!(t.prev_video_ts, 5);
}

/// Any other timestamp is new, earlier ones too: stored, count reset.
#[test]
fn steps_timestamps_going_back_reset() {
    let mut t = Tracker::default();
    track(&mut t, 5000 * MS, PACKET_NS);
    for _ in 0..10 {
        track(&mut t, 5000 * MS, PACKET_NS);
    }
    assert_eq!(t.time_since_prev, 10 * PACKET_NS);

    track(&mut t, 4000 * MS, PACKET_NS);
    assert_eq!(t.prev_video_ts, 4000 * MS);
    assert_eq!(t.time_since_prev, 0);

    track(&mut t, 4000 * MS - 1, PACKET_NS);
    assert_eq!(t.prev_video_ts, 4000 * MS - 1);
    assert_eq!(t.time_since_prev, 0);

    track(&mut t, u64::MAX, PACKET_NS);
    assert_eq!(t.prev_video_ts, u64::MAX);
    assert_eq!(t.time_since_prev, 0);
    track(&mut t, u64::MAX, PACKET_NS);
    assert_eq!(t.time_since_prev, PACKET_NS);

    track(&mut t, 1, PACKET_NS);
    assert_eq!(t.prev_video_ts, 1);
    assert_eq!(t.time_since_prev, 0);
}

/// Stalled is past `AUDIO_MONITOR_VIDEO_STALL_NS`: still video counts up to
/// it packet by packet.
#[test]
fn steps_still_video_reaches_the_stall() {
    let mut t = Tracker::default();
    let mut packets = 0;
    track(&mut t, 2000 * MS, PACKET_NS);
    while t.time_since_prev <= VIDEO_STALL_NS {
        track(&mut t, 2000 * MS, PACKET_NS);
        packets += 1;
    }
    // 21,333,333 ns a packet: 5 packets after the first
    assert_eq!(packets, 5);
    assert_eq!(t.time_since_prev, 5 * PACKET_NS);
    assert!(stalled(t.time_since_prev));
}

// ---- new: against a model of the origin/master C ----

/// origin/master's `audio_monitor_track_video_ts()`, its branches inline.
fn model(t: &mut Tracker, last_frame_ts: u64, packet_ns: u64) {
    if t.prev_video_ts == 0 {
        t.prev_video_ts = last_frame_ts;
    } else if t.prev_video_ts == last_frame_ts {
        t.time_since_prev = t.time_since_prev.wrapping_add(packet_ns);
    } else {
        t.prev_video_ts = last_frame_ts;
        t.time_since_prev = 0;
    }
}

const EDGE: [u64; 8] = [
    0,
    1,
    2,
    PACKET_NS,
    VIDEO_STALL_NS,
    VIDEO_STALL_NS + 1,
    u64::MAX - 1,
    u64::MAX,
];

/// Every start, timestamp and duration among the edge values.
#[test]
fn edge_every_step_against_the_model() {
    for prev in EDGE {
        for count in EDGE {
            for ts in EDGE {
                for ns in EDGE {
                    let start = Tracker {
                        prev_video_ts: prev,
                        time_since_prev: count,
                    };
                    let mut want = start;
                    model(&mut want, ts, ns);
                    let mut got = start;
                    track(&mut got, ts, ns);
                    assert_eq!(got, want, "from {start:?}, video {ts}, packet {ns}");
                }
            }
        }
    }
}

/// A video timestamp: 0, the last one (several times, so it stands still),
/// near it or anything.
fn step() -> impl Strategy<Value = (u8, u64, u64)> {
    (
        0u8..6,
        any::<u64>(),
        prop_oneof![Just(0u64), Just(PACKET_NS), 0u64..200 * MS, any::<u64>()],
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(2048)))]

    #[test]
    fn edge_runs_against_the_model(
        prev in prop_oneof![Just(0u64), any::<u64>()],
        count in prop_oneof![Just(0u64), 0u64..VIDEO_STALL_NS * 2, any::<u64>()],
        steps in proptest::collection::vec(step(), 1..40),
    ) {
        let mut got = Tracker { prev_video_ts: prev, time_since_prev: count };
        let mut want = got;
        let mut last = prev;
        for (kind, any_ts, ns) in steps {
            let ts = match kind {
                0 => 0,
                1..=3 => last,
                4 => last.wrapping_sub(any_ts % 1000),
                _ => any_ts,
            };
            last = ts;
            model(&mut want, ts, ns);
            track(&mut got, ts, ns);
            prop_assert_eq!(got, want);
        }
    }
}
