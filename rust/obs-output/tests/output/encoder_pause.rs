//! Tier 1: `obs_output::encoder_pause`, the pause checks of
//! `obs-encoder.c`.
//!
//! The `steps_*` cases port `test/cmocka/test_encoder_pause_steps.c` 1:1:
//! its 11 cases under the same names, in the same order, with the same
//! values and expectations, the "characterized, not endorsed" ones
//! included. The C calls the hidden `obs_encoder_pause_*` functions through
//! its `video()` and `audio()`; here [`video`] and [`audio`] call the safe
//! core the same way, as `obs-encoder.c` does (with the
//! [`AudioPaused::Resume`] branch the port adds).
//!
//! The `edge_*` cases are new: the two intentional differences (a divided
//! encoder's unpause between its frames, a pause and unpause inside one
//! audio buffer) through whole pauses, proptests of every decision against
//! a model of the C's expressions where the port keeps them, of the
//! differences exactly where they apply, and any input without a panic.

// The steps set the record field by field after `{0}`, as the C does.
#![allow(
    clippy::as_conversions,
    clippy::cast_possible_truncation,
    clippy::field_reassign_with_default
)]

use obs_output::encoder_pause::{
    AudioPaused, AudioStart, audio_cut_ns, audio_drop, audio_end, audio_kept, audio_left,
    audio_paused, audio_start, off, video_drop, video_end,
};
use obs_output::output_begin::mul_div64;
use proptest::prelude::*;

const RATE: u64 = 48000;
const FRAMES: u32 = 1024;
/// `audio_frames_to_ns(48000, 1024)`: 21333333.33 ns, truncated.
const BUFFER_NS: u64 = 21_333_333;
const T0: u64 = 1_000_000_000;
const NS: u64 = 1_000_000_000;
const FLOAT: usize = std::mem::size_of::<f32>();

// --------------------------------------------------------------------------
// the record and the callers, as obs-encoder.c asks the core

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct Record {
    last_video_ts: u64,
    ts_start: u64,
    ts_end: u64,
    ts_offset: u64,
}

/// One buffer of two float planes: `advanced` is how far each plane's data
/// pointer moved, in bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Buffer {
    advanced: [usize; 2],
    frames: u32,
    timestamp: u64,
}

fn fill(frames: u32, timestamp: u64) -> Buffer {
    Buffer {
        advanced: [0; 2],
        frames,
        timestamp,
    }
}

/// `obs-output.c`'s `end_pause()`.
fn end(r: &mut Record, ts: u64) {
    r.ts_end = ts;
    r.ts_offset = r.ts_offset.wrapping_add(r.ts_end.wrapping_sub(r.ts_start));
}

fn video(r: &mut Record, ts: u64) -> bool {
    r.last_video_ts = ts;
    if off(r.ts_start) {
        return false;
    }
    if video_end(r.ts_end, ts) {
        r.ts_start = 0;
        r.ts_end = 0;
        false
    } else {
        video_drop(r.ts_start, ts)
    }
}

fn cut_frames(ts: u64, timestamp: u64, rate: u64) -> u64 {
    mul_div64(audio_cut_ns(ts, timestamp), rate, NS)
}

fn audio_internal(r: &mut Record, b: &mut Buffer, rate: u64) -> bool {
    if off(r.ts_start) {
        return false;
    }
    let end_ts = audio_end(b.timestamp, mul_div64(u64::from(b.frames), NS, rate));
    match audio_start(r.ts_start, b.timestamp, end_ts) {
        AudioStart::Inside => {
            b.frames = audio_kept(cut_frames(r.ts_start, b.timestamp, rate));
            audio_drop(b.frames)
        }
        AudioStart::Before => match audio_paused(r.ts_end, b.timestamp, end_ts) {
            AudioPaused::Unpause => {
                let cut = cut_frames(r.ts_end, b.timestamp, rate);
                for advanced in &mut b.advanced {
                    *advanced = advanced.wrapping_add((cut as usize).wrapping_mul(FLOAT));
                }
                b.timestamp = r.ts_start;
                b.frames = audio_left(b.frames, cut);
                r.ts_start = 0;
                r.ts_end = 0;
                audio_drop(b.frames)
            }
            AudioPaused::Resume => {
                r.ts_start = 0;
                r.ts_end = 0;
                audio_drop(b.frames)
            }
            AudioPaused::Drop => true,
        },
        AudioStart::After => false,
    }
}

fn audio_at(r: &mut Record, b: &mut Buffer, rate: u64) -> bool {
    let drop = audio_internal(r, b, rate);
    b.timestamp = b.timestamp.wrapping_sub(r.ts_offset);
    drop
}

fn audio(r: &mut Record, b: &mut Buffer) -> bool {
    audio_at(r, b, RATE)
}

fn check_record(r: &Record, ts_start: u64, ts_end: u64) {
    assert_eq!((r.ts_start, r.ts_end), (ts_start, ts_end), "{r:?}");
}

fn check_buffer(b: &Buffer, frames: u32, timestamp: u64, advanced: usize) {
    assert_eq!(
        *b,
        Buffer {
            advanced: [advanced; 2],
            frames,
            timestamp,
        }
    );
}

// --------------------------------------------------------------------------
// steps: test_encoder_pause_steps.c 1:1

#[test]
fn steps_a_pause_is_off_only_at_zero() {
    assert!(off(0));
    assert!(!off(1));
    assert!(!off(T0));
    assert!(!off(u64::MAX));
}

#[test]
fn steps_video_steps() {
    let (s, e) = (2 * T0, 3 * T0);

    // exactly at the end: ends the pause
    assert!(video_end(e, e));
    // an unpause in the same frame as the pause
    assert!(video_end(s, s));
    // before the end, or with no end yet: does not
    assert!(!video_end(e, s));
    assert!(!video_end(e, e - 1));
    assert!(!video_end(0, s));
    assert!(!video_end(0, u64::MAX));
    // a frame stamped 0 while the end is not set ends the pause
    // (characterized, not endorsed: frames are never stamped 0 here)
    assert!(video_end(0, 0));

    // before the start: passes
    assert!(!video_drop(s, s - 1));
    assert!(!video_drop(s, 1));
    // at the start and after it: dropped
    assert!(video_drop(s, s));
    assert!(video_drop(s, e - 1));
    assert!(video_drop(s, u64::MAX));
}

#[test]
fn steps_audio_buffer_end() {
    assert_eq!(audio_end(T0, BUFFER_NS), T0 + BUFFER_NS);
    assert_eq!(audio_end(T0, 0), T0);
    // a uint64_t sum: wraps (characterized, not endorsed)
    assert_eq!(audio_end(u64::MAX, 2), 1);
}

#[test]
fn steps_audio_start_against_the_buffer() {
    let (t, e) = (T0, T0 + BUFFER_NS);

    assert_eq!(audio_start(e + 1, t, e), AudioStart::After);
    assert_eq!(audio_start(u64::MAX, t, e), AudioStart::After);
    assert_eq!(audio_start(t, t, e), AudioStart::Inside);
    assert_eq!(audio_start(t + 1, t, e), AudioStart::Inside);
    assert_eq!(audio_start(e, t, e), AudioStart::Inside);
    assert_eq!(audio_start(t - 1, t, e), AudioStart::Before);
    assert_eq!(audio_start(1, t, e), AudioStart::Before);
    // an empty buffer holds only its start
    assert_eq!(audio_start(t, t, t), AudioStart::Inside);
    assert_eq!(audio_start(t + 1, t, t), AudioStart::After);
}

#[test]
fn steps_audio_end_against_the_buffer() {
    let (t, e) = (T0, T0 + BUFFER_NS);

    // the end not set yet: dropped
    assert_eq!(audio_paused(0, t, e), AudioPaused::Drop);
    // the end inside the buffer, at either end of it: unpause
    assert_eq!(audio_paused(t, t, e), AudioPaused::Unpause);
    assert_eq!(audio_paused(t + 1, t, e), AudioPaused::Unpause);
    assert_eq!(audio_paused(e, t, e), AudioPaused::Unpause);
    // the end after the buffer: dropped
    assert_eq!(audio_paused(e + 1, t, e), AudioPaused::Drop);
    assert_eq!(audio_paused(u64::MAX, t, e), AudioPaused::Drop);
}

#[test]
fn steps_audio_cuts() {
    assert_eq!(audio_cut_ns(T0 + 10_000_000, T0), 10_000_000);
    assert_eq!(audio_cut_ns(T0, T0), 0);

    assert_eq!(audio_kept(480), 480);
    assert_eq!(audio_kept(0), 0);
    // truncated to uint32_t (characterized, not endorsed: a cut is never
    // longer than its buffer)
    assert_eq!(audio_kept((1_u64 << 32) + 5), 5);

    assert_eq!(audio_left(FRAMES, 240), FRAMES - 240);
    assert_eq!(audio_left(FRAMES, 0), FRAMES);
    assert_eq!(audio_left(FRAMES, u64::from(FRAMES)), 0);
    assert_eq!(audio_left(FRAMES, (1_u64 << 32) + 24), FRAMES - 24);
    // a uint32_t difference: wraps (characterized, not endorsed)
    assert_eq!(audio_left(0, 1), u32::MAX);

    assert!(audio_drop(0));
    assert!(!audio_drop(1));
    assert!(!audio_drop(FRAMES));
}

#[test]
fn steps_video_through_a_pause() {
    let mut r = Record::default();
    let frame = 16_666_667;
    let (s, e) = (T0 + 3 * frame, T0 + 9 * frame);

    // no pause: every frame passes, and its time is kept
    assert!(!video(&mut r, T0));
    assert_eq!(r.last_video_ts, T0);

    r.ts_start = s;
    assert!(!video(&mut r, s - frame));
    assert!(video(&mut r, s));
    assert!(video(&mut r, s + frame));
    check_record(&r, s, 0);

    end(&mut r, e);
    assert!(video(&mut r, e - frame));
    check_record(&r, s, e);
    assert!(!video(&mut r, e));
    check_record(&r, 0, 0);
    assert_eq!(r.last_video_ts, e);
    assert!(!video(&mut r, e + frame));
    assert_eq!(r.ts_offset, e - s);
}

#[test]
fn steps_audio_through_a_pause() {
    let mut r = Record::default();
    let s = T0 + 10_000_000;
    let (b2, b3, b4) = (T0 + BUFFER_NS, T0 + 2 * BUFFER_NS, T0 + 3 * BUFFER_NS);
    let e = b3 + 5_000_000;

    // no pause: passes as it is
    let mut b = fill(FRAMES, T0 - BUFFER_NS);
    assert!(!audio(&mut r, &mut b));
    check_buffer(&b, FRAMES, T0 - BUFFER_NS, 0);

    // the pause starts 10 ms in: 480 frames kept
    r.ts_start = s;
    let mut b = fill(FRAMES, T0);
    assert!(!audio(&mut r, &mut b));
    check_buffer(&b, 480, T0, 0);
    check_record(&r, s, 0);

    // wholly inside the pause, before and after its end is set: dropped
    let mut b = fill(FRAMES, b2);
    assert!(audio(&mut r, &mut b));
    check_record(&r, s, 0);
    end(&mut r, e);
    let mut b = fill(FRAMES, b2);
    assert!(audio(&mut r, &mut b));
    check_record(&r, s, e);

    // the pause ends 5 ms in: 240 frames cut from the front, the rest
    // passes stamped with the pause start, then the whole offset is taken
    // off (characterized, not endorsed: one pause length earlier than the
    // audio before the pause continues)
    let mut b = fill(FRAMES, b3);
    assert!(!audio(&mut r, &mut b));
    check_buffer(&b, FRAMES - 240, s - (e - s), 240 * FLOAT);
    check_record(&r, 0, 0);

    // after the pause: passes, with the offset taken off
    let mut b = fill(FRAMES, b4);
    assert!(!audio(&mut r, &mut b));
    check_buffer(&b, FRAMES, b4 - (e - s), 0);
}

#[test]
fn steps_a_start_at_either_end_of_a_buffer() {
    let mut r = Record::default();

    // at its start: nothing kept, dropped
    r.ts_start = T0;
    let mut b = fill(FRAMES, T0);
    assert!(audio(&mut r, &mut b));
    check_buffer(&b, 0, T0, 0);
    check_record(&r, T0, 0);

    // at its end: 1023 of 1024 frames kept, as both conversions truncate
    // (characterized, not endorsed)
    r.ts_start = T0 + BUFFER_NS;
    let mut b = fill(FRAMES, T0);
    assert!(!audio(&mut r, &mut b));
    check_buffer(&b, FRAMES - 1, T0, 0);
    check_record(&r, T0 + BUFFER_NS, 0);

    // an empty buffer at the start: still empty, dropped
    r.ts_start = T0;
    let mut b = fill(0, T0);
    assert!(audio(&mut r, &mut b));
    check_buffer(&b, 0, T0, 0);
}

#[test]
fn steps_an_end_at_either_end_of_a_buffer() {
    let mut r = Record::default();
    let s = T0 - 3 * BUFFER_NS;

    // at its start: nothing cut, the whole buffer passes, stamped with the
    // pause start before the offset
    r.ts_start = s;
    end(&mut r, T0);
    let mut b = fill(FRAMES, T0);
    assert!(!audio(&mut r, &mut b));
    check_buffer(&b, FRAMES, s - (T0 - s), 0);
    check_record(&r, 0, 0);

    // at its end: 1023 frames cut, one passes
    r.ts_start = s;
    r.ts_offset = 0;
    end(&mut r, T0 + BUFFER_NS);
    let mut b = fill(FRAMES, T0);
    assert!(!audio(&mut r, &mut b));
    check_buffer(
        &b,
        1,
        s - (T0 + BUFFER_NS - s),
        (FRAMES as usize - 1) * FLOAT,
    );
    check_record(&r, 0, 0);
}

#[test]
fn steps_the_offset_is_taken_off_every_buffer() {
    let mut r = Record {
        ts_offset: 5000,
        ..Record::default()
    };

    // passed with no pause on
    let mut b = fill(FRAMES, T0);
    assert!(!audio(&mut r, &mut b));
    check_buffer(&b, FRAMES, T0 - 5000, 0);

    // dropped inside a pause: the offset is taken off it too
    r.ts_start = T0 - 1;
    let mut b = fill(FRAMES, T0);
    assert!(audio(&mut r, &mut b));
    check_buffer(&b, FRAMES, T0 - 5000, 0);
}

// --------------------------------------------------------------------------
// edge: new cases

/// The C's end of a video pause in `video_pause_check_internal()`.
fn c_video_end(ts_end: u64, ts: u64) -> bool {
    ts == ts_end
}

/// The C's paused-buffer decision of `audio_pause_check_internal()`.
fn c_audio_paused(ts_end: u64, timestamp: u64, end_ts: u64) -> AudioPaused {
    if ts_end >= timestamp && ts_end <= end_ts {
        AudioPaused::Unpause
    } else {
        AudioPaused::Drop
    }
}

#[test]
fn edge_a_divided_encoder_unpauses_between_its_frames() {
    // An encoder with a frame rate divisor of 2 gets every other frame of
    // the 60 fps grid; the unpause time lands on a frame it never gets.
    let frame = 16_666_667;
    let mut r = Record::default();
    r.ts_start = T0 + 4 * frame;
    for n in (0..4).step_by(2) {
        assert!(!video(&mut r, T0 + n * frame));
    }
    for n in (4..10).step_by(2) {
        assert!(video(&mut r, T0 + n * frame));
    }
    end(&mut r, T0 + 11 * frame);
    assert!(video(&mut r, T0 + 10 * frame));
    // The first frame after the end ends the pause (the C drops it and
    // every later one, and keeps the record set).
    assert!(!video(&mut r, T0 + 12 * frame));
    check_record(&r, 0, 0);
    for n in (14..30).step_by(2) {
        assert!(!video(&mut r, T0 + n * frame));
    }
    assert!(video_end(T0 + 1, T0 + 2));
    assert!(!c_video_end(T0 + 1, T0 + 2));
}

#[test]
fn edge_a_pause_and_unpause_inside_one_buffer() {
    let mut r = Record::default();
    let s = T0 + 5_000_000;
    let e = T0 + 10_000_000;
    r.ts_start = s;
    end(&mut r, e);

    // Cut at the start (240 frames kept); the end is not looked at.
    let mut b = fill(FRAMES, T0);
    assert!(!audio(&mut r, &mut b));
    check_buffer(&b, 240, T0 - (e - s), 0);
    check_record(&r, s, e);

    // The next buffer starts after the end: the pause ends and it passes
    // with its own timestamp (the C drops it and every later one, and
    // keeps the record set).
    let mut b = fill(FRAMES, T0 + BUFFER_NS);
    assert!(!audio(&mut r, &mut b));
    check_buffer(&b, FRAMES, T0 + BUFFER_NS - (e - s), 0);
    check_record(&r, 0, 0);
    let mut b = fill(FRAMES, T0 + 2 * BUFFER_NS);
    assert!(!audio(&mut r, &mut b));
    check_buffer(&b, FRAMES, T0 + 2 * BUFFER_NS - (e - s), 0);

    // An empty buffer after the end still ends the pause, and is dropped.
    let mut r = Record {
        ts_start: s,
        ts_end: e,
        ..Record::default()
    };
    let mut b = fill(0, T0 + BUFFER_NS);
    assert!(audio(&mut r, &mut b));
    check_record(&r, 0, 0);

    // An unpause in the pause's own video frame (ts_end == ts_start).
    let mut r = Record::default();
    r.ts_start = s;
    end(&mut r, s);
    let mut b = fill(FRAMES, T0);
    assert!(!audio(&mut r, &mut b));
    check_buffer(&b, 240, T0, 0);
    let mut b = fill(FRAMES, T0 + BUFFER_NS);
    assert!(!audio(&mut r, &mut b));
    check_record(&r, 0, 0);

    assert_eq!(audio_paused(1, 2, 3), AudioPaused::Resume);
    assert_eq!(c_audio_paused(1, 2, 3), AudioPaused::Drop);
}

#[test]
fn edge_a_paused_buffer_waits_for_a_set_end() {
    // The end is 0 until the unpause, and 0 is before every buffer: still
    // dropped.
    assert_eq!(audio_paused(0, T0, T0 + BUFFER_NS), AudioPaused::Drop);
    assert_eq!(audio_paused(0, 1, 1), AudioPaused::Drop);
    // A frame stamped 0 with the end unset ends the pause, as in the C.
    assert!(video_end(0, 0));
    assert!(!video_end(0, 1) && !video_drop(T0, 1));
    assert!(!video_end(0, T0 + 1) && video_drop(T0, T0 + 1));
}

/// Times that hit each other's edges: small, on a grid near `T0`, and near
/// the top of the range.
fn time() -> impl Strategy<Value = u64> {
    prop_oneof![
        0_u64..4,
        (0_u64..400).prop_map(|n| T0 + n * 10_416_667 / 3),
        (0_u64..100_000_000).prop_map(|n| u64::MAX - n),
        any::<u64>(),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(4096),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn edge_video_matches_c_except_after_a_set_end(
        ts_start in time(),
        ts_end in time(),
        ts in time(),
        near in -2_i64..=2,
    ) {
        let ts = if near == 0 { ts } else { ts_end.wrapping_add_signed(near) };
        let want = if ts_end != 0 && ts > ts_end { true } else { c_video_end(ts_end, ts) };
        prop_assert_eq!(video_end(ts_end, ts), want);
        prop_assert_eq!(video_drop(ts_start, ts), ts >= ts_start);
    }

    #[test]
    fn edge_audio_paused_matches_c_except_after_a_set_end(
        ts_end in time(),
        timestamp in time(),
        end_ts in time(),
    ) {
        let want = if ts_end != 0 && ts_end < timestamp {
            AudioPaused::Resume
        } else {
            c_audio_paused(ts_end, timestamp, end_ts)
        };
        prop_assert_eq!(audio_paused(ts_end, timestamp, end_ts), want);
    }

    #[test]
    fn edge_audio_start_matches_c(ts_start in time(), timestamp in time(), end_ts in time()) {
        let want = if ts_start >= timestamp {
            if ts_start <= end_ts { AudioStart::Inside } else { AudioStart::After }
        } else {
            AudioStart::Before
        };
        prop_assert_eq!(audio_start(ts_start, timestamp, end_ts), want);
    }

    #[test]
    fn edge_the_sums_and_cuts_wrap_as_c(a in time(), b in time(), frames in any::<u32>()) {
        prop_assert_eq!(off(a), a == 0);
        prop_assert_eq!(audio_end(a, b), a.wrapping_add(b));
        prop_assert_eq!(audio_cut_ns(a, b), a.wrapping_sub(b));
        prop_assert_eq!(audio_kept(b), b as u32);
        prop_assert_eq!(audio_left(frames, b), frames.wrapping_sub(b as u32));
        prop_assert_eq!(audio_drop(frames), frames == 0);
    }

    #[test]
    fn edge_a_whole_pause_always_ends(
        start_frame in 1_u64..200,
        length in 0_u64..200,
        divisor in 1_u64..=4,
        rate in prop::sample::select(vec![44100_u64, 48000, 96000]),
        frames in prop::sample::select(vec![480_u32, 1024]),
    ) {
        // A pause and its unpause on the 60 fps grid, an encoder taking
        // every divisor-th frame and buffers of a fixed length: once both
        // are set, the record clears within one frame and one buffer of
        // the end, and the audio and video after it pass.
        let frame = 16_666_667;
        let s = T0 + start_frame * frame;
        let e = s + length * frame;
        let mut v = Record { ts_start: s, ..Record::default() };
        end(&mut v, e);
        let mut a = v;
        let mut n = 0;
        while T0 + n * frame <= e {
            n += divisor;
        }
        video(&mut v, T0 + n * frame);
        prop_assert_eq!((v.ts_start, v.ts_end), (0, 0));
        prop_assert!(!video(&mut v, T0 + (n + divisor) * frame));

        let len = mul_div64(u64::from(frames), NS, rate);
        let mut ts = s - 3 * len;
        let mut cleared = None;
        for _ in 0..2_000 {
            let mut b = fill(frames, ts);
            audio_at(&mut a, &mut b, rate);
            if a.ts_start == 0 {
                cleared = Some(ts);
                break;
            }
            ts += len;
        }
        let cleared = cleared.unwrap_or(u64::MAX);
        prop_assert!(cleared <= e.saturating_add(len), "{} > {} + {}", cleared, e, len);
        let mut b = fill(frames, cleared + len);
        prop_assert!(!audio_at(&mut a, &mut b, rate));
    }

    #[test]
    fn edge_any_input_without_a_panic(
        ts_start in any::<u64>(),
        ts_end in any::<u64>(),
        ts in any::<u64>(),
        frames in any::<u32>(),
        cut in any::<u64>(),
    ) {
        let _ = off(ts_start);
        let _ = video_end(ts_end, ts);
        let _ = video_drop(ts_start, ts);
        let end_ts = audio_end(ts, cut);
        let _ = audio_start(ts_start, ts, end_ts);
        let _ = audio_paused(ts_end, ts, end_ts);
        let _ = audio_cut_ns(ts_end, ts);
        let _ = audio_kept(cut);
        let _ = audio_left(frames, cut);
        let _ = audio_drop(frames);
    }
}
