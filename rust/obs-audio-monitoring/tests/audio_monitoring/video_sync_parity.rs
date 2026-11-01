//! Tier 3 parity: `monitor_delay::track_video_ts` and its shim against the
//! original `monitor-video-sync.c.h` (oracle/monitor_delay.c).
//!
//! The unchanged C tests `test_monitor_video_sync`,
//! `test_monitor_wasapi_steps` and `test_media_cover_art_monitor` compile the
//! C into themselves; the first two's `_rust` runs use the shim, and this
//! file compares all three sides (oracle, shim, core) exactly, both values
//! the tracker writes after every step: every start, timestamp and duration
//! among +-3 around the edge values, and proptest runs of up to 64 packets
//! from any start, with timestamps that are 0, stand still, step back, jump
//! or are anything, and durations of 0, a packet, near the stall or
//! anything. Both pointers at the same `uint64_t`, which the C allows, are
//! compared too: the C writes only what its branch writes, in its order.
//!
//! No intentional difference. The count wraps, as the C's `uint64_t` does.
//! Null pointers, which the C dereferences, are checked on the Rust side
//! only: each reads as 0 and is not written.

use std::ptr;

use obs_audio_monitoring::ffi::monitor_delay as shim;
use obs_audio_monitoring::monitor_delay::{self as core, VIDEO_STALL_NS};
use obs_c_oracle::monitor_delay::oracle_audio_monitor_track_video_ts;
use proptest::prelude::*;

/// A 1024-frame packet at 48 kHz.
const PACKET_NS: u64 = 1024 * 1_000_000_000 / 48_000;

/// `(prev_video_ts, time_since_prev)`.
type State = (u64, u64);

fn oracle(s: State, ts: u64, ns: u64) -> State {
    let (mut prev, mut count) = s;
    // SAFETY: two valid u64s to write.
    unsafe { oracle_audio_monitor_track_video_ts(&raw mut prev, &raw mut count, ts, ns) };
    (prev, count)
}

fn shim_step(s: State, ts: u64, ns: u64) -> State {
    let (mut prev, mut count) = s;
    // SAFETY: two valid u64s to write.
    unsafe { shim::audio_monitor_track_video_ts(&raw mut prev, &raw mut count, ts, ns) };
    (prev, count)
}

fn core_step(s: State, ts: u64, ns: u64) -> State {
    let (mut prev, mut count) = s;
    core::track_video_ts(&mut prev, &mut count, ts, ns);
    (prev, count)
}

/// One step on all three sides from `s`; returns the C's state.
fn check(s: State, ts: u64, ns: u64) -> State {
    let want = oracle(s, ts, ns);
    assert_eq!(
        shim_step(s, ts, ns),
        want,
        "shim from {s:?}, video {ts}, packet {ns}"
    );
    assert_eq!(
        core_step(s, ts, ns),
        want,
        "core from {s:?}, video {ts}, packet {ns}"
    );
    want
}

/// +-3 around 0, a packet, the stall, 2^63 and `UINT64_MAX`.
fn edges() -> Vec<u64> {
    let mut v = Vec::new();
    for around in [0u64, PACKET_NS, VIDEO_STALL_NS, 1 << 63, u64::MAX] {
        v.extend((0..=6).map(|d| around.wrapping_sub(3).wrapping_add(d)));
    }
    v.sort_unstable();
    v.dedup();
    v
}

#[test]
fn every_step_among_the_edges() {
    let edges = edges();
    let mut branches = [0u32; 3];
    let mut i = 0u64;
    for &prev in &edges {
        for &count in &edges {
            for &ts in &edges {
                for &ns in &edges {
                    i += 1;
                    // the first-packet branch on every level, the rest
                    // sampled at the reduced one
                    if prev != 0 && !obs_c_oracle::parity::keep(i) {
                        continue;
                    }
                    let after = check((prev, count), ts, ns);
                    let branch = if prev == 0 {
                        0
                    } else if prev == ts {
                        1
                    } else {
                        2
                    };
                    branches[branch] += 1;
                    if branch == 0 {
                        assert_eq!(after, (ts, count));
                    }
                }
            }
        }
    }
    assert!(branches.iter().all(|&n| n > 0), "{branches:?}");
}

/// Both pointers at one `uint64_t`: the C reads and writes it in its
/// branch's order (the first packet stores the timestamp and leaves the
/// count, so the timestamp stays; a new one stores it, then zeroes it).
#[test]
fn aliased_pointers_match_the_c() {
    let edges = edges();
    for &x in &edges {
        for &ts in &edges {
            for ns in [0, 1, PACKET_NS, u64::MAX] {
                let mut want = x;
                // SAFETY: one valid u64, which the C reads and writes
                // through both pointers.
                unsafe {
                    oracle_audio_monitor_track_video_ts(&raw mut want, &raw mut want, ts, ns)
                };
                let mut got = x;
                // SAFETY: as above.
                unsafe { shim::audio_monitor_track_video_ts(&raw mut got, &raw mut got, ts, ns) };
                assert_eq!(got, want, "x {x}, video {ts}, packet {ns}");
            }
        }
    }
}

/// Null pointers: each reads as 0 and is not written (Rust side only).
#[test]
fn null_pointers_read_as_zero() {
    // no video timestamp: as 0, so the first packet; the count stays
    let mut count = 42u64;
    // SAFETY: null, or a valid u64 to write.
    unsafe { shim::audio_monitor_track_video_ts(ptr::null_mut(), &raw mut count, 7, PACKET_NS) };
    assert_eq!(count, 42);

    // no count: read as 0, the stored timestamp still moves
    let mut prev = 5u64;
    // SAFETY: as above.
    unsafe { shim::audio_monitor_track_video_ts(&raw mut prev, ptr::null_mut(), 9, PACKET_NS) };
    assert_eq!(prev, 9);
    // SAFETY: as above.
    unsafe { shim::audio_monitor_track_video_ts(&raw mut prev, ptr::null_mut(), 9, PACKET_NS) };
    assert_eq!(prev, 9);

    // SAFETY: both null; nothing is written.
    unsafe { shim::audio_monitor_track_video_ts(ptr::null_mut(), ptr::null_mut(), 1, 1) };
}

/// A packet: the kind of video timestamp it sees, a value for it and its
/// duration.
fn packet() -> impl Strategy<Value = (u8, u64, u64)> {
    (
        0u8..7,
        any::<u64>(),
        prop_oneof![
            Just(0u64),
            Just(PACKET_NS),
            VIDEO_STALL_NS - 3..VIDEO_STALL_NS + 4,
            0u64..200_000_000,
            any::<u64>(),
        ],
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(4_000)))]

    #[test]
    fn runs_from_any_start(
        prev in prop_oneof![Just(0u64), any::<u64>()],
        count in prop_oneof![Just(0u64), 0u64..VIDEO_STALL_NS * 2, u64::MAX - 1_000_000_000..=u64::MAX, any::<u64>()],
        packets in proptest::collection::vec(packet(), 1..64),
    ) {
        let mut state = (prev, count);
        let mut last = prev;
        for (kind, value, ns) in packets {
            let ts = match kind {
                0 => 0,
                1..=3 => last,
                4 => last.wrapping_sub(value % 1_000),
                5 => last.wrapping_add(value % 100_000_000),
                _ => value,
            };
            last = ts;
            state = check(state, ts, ns);
        }
    }
}
