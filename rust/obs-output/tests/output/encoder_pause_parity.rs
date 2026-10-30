//! Tier 3 parity: the shims of `obs_output::ffi::encoder_pause` (and the
//! safe core under them) against the original `libobs/obs-encoder-pause.c.h`,
//! compiled as `oracle_obs_encoder_pause_*` (obs-c-oracle,
//! oracle/encoder_pause.c).
//!
//! Every call runs the shim, the C and the safe core, and compares the
//! results exactly where the port keeps the C's behaviour.
//!
//! Runs: exhaustive over a grid of times (0 to 3, the 60 fps frame grid at
//! frame and half-frame steps from 1 s, the 48 kHz buffer edges, each with
//! its neighbours, and the top of the `u64` range) for every decision with
//! two or three times; the cut frames against buffers of 0, 1, 480, 1023,
//! 1024, 4096 and `u32::MAX` frames; whole pauses through `video_pause_check()`
//! and `audio_pause_check()` as `obs-encoder.c` asks them, with the C and
//! the shims side by side: video at frame rate divisors 1 to 4 with the
//! pause and its end on the frame grid, and audio buffers of 0, 1, 480 and
//! 1024 frames at 44100, 48000 and 96000 Hz with the start and end on the
//! frame grid and at half-frame steps, the end set before the first buffer
//! or after the pause began; and proptests over times near the top of the
//! range and any input on the shim.
//!
//! At a reduced `OBS_PARITY_CASES` level the three-time grid and the whole
//! audio pauses run a sampled share, and their edges always: a time at 0 or
//! the top of the range or two times equal or one apart, and a pause that
//! ends where it starts, starts at the first buffer or meets empty or
//! one-frame buffers. The two-time grids, the cuts and the video pauses
//! always run whole.
//!
//! Intentional differences, checked exactly where they apply:
//! - a video frame after a set `ts_end` (not 0) ends the pause, where the C
//!   ends it only on a frame stamped exactly `ts_end`. Over a whole pause,
//!   the two agree up to that frame; from it on the C drops every frame and
//!   keeps its record set, and the port passes every frame with the record
//!   clear;
//! - a paused audio buffer that starts after a set `ts_end` (not 0) ends
//!   the pause and passes as it is (`OBS_ENCODER_PAUSE_AUDIO_PAUSED_RESUME`),
//!   where the C drops it. Over a whole pause, the two agree up to that
//!   buffer; from it on the C drops every buffer and keeps its record set,
//!   and the port passes every buffer (with its own timestamp, less the
//!   offset) with the record clear. The audio from `ts_end` to the end of
//!   the buffer that was cut at the start is lost on both.
//!
//! `test_encoder_pause_steps` runs the C test against the C; the real-libobs
//! output tests run against the hybrid libobs, where these shims replace
//! the C.

#![allow(clippy::as_conversions, clippy::cast_possible_truncation)]

use std::ffi::c_int;

use obs_c_oracle::encoder_pause::{
    oracle_obs_encoder_pause_audio_cut_ns, oracle_obs_encoder_pause_audio_drop,
    oracle_obs_encoder_pause_audio_end, oracle_obs_encoder_pause_audio_kept,
    oracle_obs_encoder_pause_audio_left, oracle_obs_encoder_pause_audio_paused,
    oracle_obs_encoder_pause_audio_start, oracle_obs_encoder_pause_off,
    oracle_obs_encoder_pause_video_drop, oracle_obs_encoder_pause_video_end,
};
use obs_output::encoder_pause::{self as core, AudioPaused, AudioStart};
use obs_output::ffi::encoder_pause::{
    OBS_ENCODER_PAUSE_AUDIO_PAUSED_DROP, OBS_ENCODER_PAUSE_AUDIO_PAUSED_RESUME,
    OBS_ENCODER_PAUSE_AUDIO_PAUSED_UNPAUSE, OBS_ENCODER_PAUSE_AUDIO_START_AFTER,
    OBS_ENCODER_PAUSE_AUDIO_START_BEFORE, OBS_ENCODER_PAUSE_AUDIO_START_INSIDE,
    obs_encoder_pause_audio_cut_ns, obs_encoder_pause_audio_drop, obs_encoder_pause_audio_end,
    obs_encoder_pause_audio_kept, obs_encoder_pause_audio_left, obs_encoder_pause_audio_paused,
    obs_encoder_pause_audio_start, obs_encoder_pause_off, obs_encoder_pause_video_drop,
    obs_encoder_pause_video_end,
};
use obs_output::output_begin::mul_div64;
use proptest::prelude::*;

const T0: u64 = 1_000_000_000;
const NS: u64 = 1_000_000_000;
/// The 60 fps frame interval, truncated (`video_frame_interval_ns`).
const FRAME: u64 = 16_666_666;
/// `audio_frames_to_ns(48000, 1024)`.
const BUFFER_NS: u64 = 21_333_333;
const FLOAT: usize = std::mem::size_of::<f32>();

fn start_of(start: c_int) -> AudioStart {
    match start {
        OBS_ENCODER_PAUSE_AUDIO_START_AFTER => AudioStart::After,
        OBS_ENCODER_PAUSE_AUDIO_START_INSIDE => AudioStart::Inside,
        OBS_ENCODER_PAUSE_AUDIO_START_BEFORE => AudioStart::Before,
        other => panic!("audio start {other}"),
    }
}

fn paused_of(paused: c_int) -> AudioPaused {
    match paused {
        OBS_ENCODER_PAUSE_AUDIO_PAUSED_DROP => AudioPaused::Drop,
        OBS_ENCODER_PAUSE_AUDIO_PAUSED_UNPAUSE => AudioPaused::Unpause,
        OBS_ENCODER_PAUSE_AUDIO_PAUSED_RESUME => AudioPaused::Resume,
        other => panic!("audio paused {other}"),
    }
}

/// The times of the grid: each edge with its neighbours.
fn grid() -> Vec<u64> {
    let mut edges = vec![0, 1, 2, 3, u64::MAX, u64::MAX - FRAME, u64::MAX - BUFFER_NS];
    edges.extend((0..=24).map(|k| T0 + k * FRAME / 2));
    edges.extend((0..=4).map(|k| T0 + k * BUFFER_NS));
    let mut times: Vec<u64> = edges
        .iter()
        .flat_map(|&t| [t.wrapping_sub(1), t, t.wrapping_add(1)])
        .collect();
    times.sort_unstable();
    times.dedup();
    times
}

// --------------------------------------------------------------------------
// one decision at a time

fn check_video_end(ts_end: u64, ts: u64) {
    let c = unsafe { oracle_obs_encoder_pause_video_end(ts_end, ts) };
    let shim = obs_encoder_pause_video_end(ts_end, ts);
    let port = core::video_end(ts_end, ts);
    assert_eq!(shim, port, "shim, {ts_end} {ts}");
    if ts_end != 0 && ts > ts_end {
        // Intentional difference: a frame after a set end ends the pause.
        assert!(port && !c, "{ts_end} {ts}");
    } else {
        assert_eq!(port, c, "{ts_end} {ts}");
    }
}

fn check_video_drop(ts_start: u64, ts: u64) {
    let c = unsafe { oracle_obs_encoder_pause_video_drop(ts_start, ts) };
    let shim = obs_encoder_pause_video_drop(ts_start, ts);
    assert_eq!(
        (shim, core::video_drop(ts_start, ts)),
        (c, c),
        "{ts_start} {ts}"
    );
}

fn check_audio_start(ts_start: u64, timestamp: u64, end_ts: u64) {
    let c = start_of(unsafe { oracle_obs_encoder_pause_audio_start(ts_start, timestamp, end_ts) });
    let shim = start_of(obs_encoder_pause_audio_start(ts_start, timestamp, end_ts));
    let port = core::audio_start(ts_start, timestamp, end_ts);
    assert_eq!((shim, port), (c, c), "{ts_start} {timestamp} {end_ts}");
}

fn check_audio_paused(ts_end: u64, timestamp: u64, end_ts: u64) {
    let c = paused_of(unsafe { oracle_obs_encoder_pause_audio_paused(ts_end, timestamp, end_ts) });
    let shim = paused_of(obs_encoder_pause_audio_paused(ts_end, timestamp, end_ts));
    let port = core::audio_paused(ts_end, timestamp, end_ts);
    assert_eq!(shim, port, "shim, {ts_end} {timestamp} {end_ts}");
    assert_ne!(c, AudioPaused::Resume);
    if ts_end != 0 && ts_end < timestamp {
        // Intentional difference: a buffer after a set end ends the pause.
        assert_eq!(
            (port, c),
            (AudioPaused::Resume, AudioPaused::Drop),
            "{ts_end} {timestamp} {end_ts}"
        );
    } else {
        assert_eq!(port, c, "{ts_end} {timestamp} {end_ts}");
    }
}

fn check_pair(a: u64, b: u64) {
    unsafe {
        assert_eq!(
            obs_encoder_pause_audio_end(a, b),
            oracle_obs_encoder_pause_audio_end(a, b)
        );
        assert_eq!(
            obs_encoder_pause_audio_cut_ns(a, b),
            oracle_obs_encoder_pause_audio_cut_ns(a, b)
        );
    }
    assert_eq!(core::audio_end(a, b), obs_encoder_pause_audio_end(a, b));
    assert_eq!(
        core::audio_cut_ns(a, b),
        obs_encoder_pause_audio_cut_ns(a, b)
    );
}

fn check_frames(frames: u32, cut: u64) {
    unsafe {
        assert_eq!(
            obs_encoder_pause_audio_kept(cut),
            oracle_obs_encoder_pause_audio_kept(cut)
        );
        assert_eq!(
            obs_encoder_pause_audio_left(frames, cut),
            oracle_obs_encoder_pause_audio_left(frames, cut),
            "{frames} {cut}"
        );
        assert_eq!(
            obs_encoder_pause_audio_drop(frames),
            oracle_obs_encoder_pause_audio_drop(frames)
        );
    }
    assert_eq!(core::audio_kept(cut), obs_encoder_pause_audio_kept(cut));
    assert_eq!(
        core::audio_left(frames, cut),
        obs_encoder_pause_audio_left(frames, cut)
    );
    assert_eq!(
        core::audio_drop(frames),
        obs_encoder_pause_audio_drop(frames)
    );
}

fn check_off(ts_start: u64) {
    let c = unsafe { oracle_obs_encoder_pause_off(ts_start) };
    assert_eq!(
        (obs_encoder_pause_off(ts_start), core::off(ts_start)),
        (c, c)
    );
}

#[test]
fn every_video_decision_over_the_grid() {
    let times = grid();
    for &a in &times {
        check_off(a);
        for &t in &times {
            check_video_end(a, t);
            check_video_drop(a, t);
        }
    }
}

/// Whether a grid case is an edge, run on every level outside the sampled
/// part: a time at 0 or at the top of the range, or two times equal or
/// one apart, where every compare of the decisions turns.
fn edge(times: &[u64]) -> bool {
    times.iter().any(|&t| t == 0 || t == u64::MAX)
        || times
            .iter()
            .enumerate()
            .any(|(n, &x)| times.iter().skip(n + 1).any(|&y| x.abs_diff(y) <= 1))
}

#[test]
fn every_audio_route_over_the_grid() {
    let times = grid();
    let mut i = 0;
    for &a in &times {
        for &b in &times {
            check_pair(a, b);
            for &c in &times {
                i += 1;
                if edge(&[a, b, c]) || obs_c_oracle::parity::keep(i) {
                    check_audio_start(a, b, c);
                    check_audio_paused(a, b, c);
                }
            }
        }
    }
}

#[test]
fn every_cut_against_every_buffer_length() {
    let cuts: Vec<u64> = (0..=4100)
        .chain([u64::from(u32::MAX), 1 << 32, (1 << 32) + 1, u64::MAX])
        .collect();
    for frames in [0, 1, 480, 1023, 1024, 4096, u32::MAX] {
        for &cut in &cuts {
            check_frames(frames, cut);
        }
    }
}

// --------------------------------------------------------------------------
// whole pauses, through the callers

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct Record {
    last_video_ts: u64,
    ts_start: u64,
    ts_end: u64,
    ts_offset: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Buffer {
    advanced: usize,
    frames: u32,
    timestamp: u64,
}

/// The ten functions, from the C or the shims.
struct Side {
    off: unsafe extern "C" fn(u64) -> bool,
    video_end: unsafe extern "C" fn(u64, u64) -> bool,
    video_drop: unsafe extern "C" fn(u64, u64) -> bool,
    audio_end: unsafe extern "C" fn(u64, u64) -> u64,
    audio_start: unsafe extern "C" fn(u64, u64, u64) -> c_int,
    audio_paused: unsafe extern "C" fn(u64, u64, u64) -> c_int,
    audio_cut_ns: unsafe extern "C" fn(u64, u64) -> u64,
    audio_kept: unsafe extern "C" fn(u64) -> u32,
    audio_left: unsafe extern "C" fn(u32, u64) -> u32,
    audio_drop: unsafe extern "C" fn(u32) -> bool,
}

const C: Side = Side {
    off: oracle_obs_encoder_pause_off,
    video_end: oracle_obs_encoder_pause_video_end,
    video_drop: oracle_obs_encoder_pause_video_drop,
    audio_end: oracle_obs_encoder_pause_audio_end,
    audio_start: oracle_obs_encoder_pause_audio_start,
    audio_paused: oracle_obs_encoder_pause_audio_paused,
    audio_cut_ns: oracle_obs_encoder_pause_audio_cut_ns,
    audio_kept: oracle_obs_encoder_pause_audio_kept,
    audio_left: oracle_obs_encoder_pause_audio_left,
    audio_drop: oracle_obs_encoder_pause_audio_drop,
};

const SHIM: Side = Side {
    off: obs_encoder_pause_off,
    video_end: obs_encoder_pause_video_end,
    video_drop: obs_encoder_pause_video_drop,
    audio_end: obs_encoder_pause_audio_end,
    audio_start: obs_encoder_pause_audio_start,
    audio_paused: obs_encoder_pause_audio_paused,
    audio_cut_ns: obs_encoder_pause_audio_cut_ns,
    audio_kept: obs_encoder_pause_audio_kept,
    audio_left: obs_encoder_pause_audio_left,
    audio_drop: obs_encoder_pause_audio_drop,
};

impl Side {
    /// `video_pause_check()`.
    fn video(&self, r: &mut Record, ts: u64) -> bool {
        r.last_video_ts = ts;
        if unsafe { (self.off)(r.ts_start) } {
            return false;
        }
        if unsafe { (self.video_end)(r.ts_end, ts) } {
            r.ts_start = 0;
            r.ts_end = 0;
        } else if unsafe { (self.video_drop)(r.ts_start, ts) } {
            return true;
        }
        false
    }

    /// `audio_pause_check()`.
    fn audio(&self, r: &mut Record, b: &mut Buffer, rate: u64) -> bool {
        let drop = self.audio_internal(r, b, rate);
        b.timestamp = b.timestamp.wrapping_sub(r.ts_offset);
        drop
    }

    fn audio_internal(&self, r: &mut Record, b: &mut Buffer, rate: u64) -> bool {
        if unsafe { (self.off)(r.ts_start) } {
            return false;
        }
        let duration_ns = mul_div64(u64::from(b.frames), NS, rate);
        let end_ts = unsafe { (self.audio_end)(b.timestamp, duration_ns) };
        let start = unsafe { (self.audio_start)(r.ts_start, b.timestamp, end_ts) };
        if start == OBS_ENCODER_PAUSE_AUDIO_START_INSIDE {
            let cut = mul_div64(
                unsafe { (self.audio_cut_ns)(r.ts_start, b.timestamp) },
                rate,
                NS,
            );
            b.frames = unsafe { (self.audio_kept)(cut) };
            return unsafe { (self.audio_drop)(b.frames) };
        } else if start == OBS_ENCODER_PAUSE_AUDIO_START_BEFORE {
            let paused = unsafe { (self.audio_paused)(r.ts_end, b.timestamp, end_ts) };
            if paused == OBS_ENCODER_PAUSE_AUDIO_PAUSED_UNPAUSE {
                let cut = mul_div64(
                    unsafe { (self.audio_cut_ns)(r.ts_end, b.timestamp) },
                    rate,
                    NS,
                );
                b.advanced = b.advanced.wrapping_add((cut as usize).wrapping_mul(FLOAT));
                b.timestamp = r.ts_start;
                b.frames = unsafe { (self.audio_left)(b.frames, cut) };
                r.ts_start = 0;
                r.ts_end = 0;
                return unsafe { (self.audio_drop)(b.frames) };
            } else if paused == OBS_ENCODER_PAUSE_AUDIO_PAUSED_RESUME {
                r.ts_start = 0;
                r.ts_end = 0;
                return unsafe { (self.audio_drop)(b.frames) };
            }
            return true;
        }
        false
    }
}

fn end(r: &mut Record, ts: u64) {
    r.ts_end = ts;
    r.ts_offset = r.ts_offset.wrapping_add(r.ts_end.wrapping_sub(r.ts_start));
}

/// One video encoder through a pause from frame `start` to frame `stop` of
/// the grid, taking every `divisor`-th frame.
fn video_pause(divisor: u64, start: u64, stop: u64) -> bool {
    let (s, e) = (T0 + start * FRAME, T0 + stop * FRAME);
    let (mut rc, mut rs) = (Record::default(), Record::default());
    let mut split = false;
    for n in (0..120).step_by(divisor as usize) {
        let ts = T0 + n * FRAME;
        if ts >= s && rc.ts_start == 0 && rc.ts_end == 0 && rc.last_video_ts < s {
            rc.ts_start = s;
            rs.ts_start = s;
        }
        // The unpause comes before the encoder's last frame ahead of the
        // end (or at once, when the pause started past its end).
        if rc.ts_start != 0 && rc.ts_end == 0 && ts + divisor * FRAME > e {
            end(&mut rc, e);
            end(&mut rs, e);
        }
        let differs = !split && rc.ts_start != 0 && rc.ts_end != 0 && ts > rc.ts_end;
        let (dc, ds) = (C.video(&mut rc, ts), SHIM.video(&mut rs, ts));
        split |= differs;
        if split {
            // The C stays paused for good; the port ended the pause.
            assert!(
                dc,
                "C passes {ts} after its missed end, {divisor} {start} {stop}"
            );
            assert_eq!((rc.ts_start, rc.ts_end), (s, e));
            assert!(!ds, "port drops {ts}, {divisor} {start} {stop}");
            assert_eq!((rs.ts_start, rs.ts_end), (0, 0));
        } else {
            assert_eq!((dc, rc), (ds, rs), "{ts}, {divisor} {start} {stop}");
        }
    }
    // The port always ends the pause; the C only when a frame lands on the
    // end.
    assert_eq!((rs.ts_start, rs.ts_end), (0, 0), "{divisor} {start} {stop}");
    assert_eq!(split, rc.ts_start != 0, "{divisor} {start} {stop}");
    split
}

#[test]
fn whole_video_pauses_at_every_divisor() {
    let mut splits = [0; 5];
    for divisor in 1..=4 {
        for start in 1..40 {
            for stop in start..start + 40 {
                splits[divisor as usize] += usize::from(video_pause(divisor, start, stop));
            }
        }
    }
    // Every frame reaches the end at divisor 1; a divided encoder misses
    // it on some pauses, and only there the C stays paused.
    assert_eq!(splits[1], 0);
    for (divisor, &n) in splits.iter().enumerate().skip(2) {
        assert!(n > 0 && n < 39 * 40, "divisor {divisor}: {n}");
    }
}

/// One audio encoder through a pause from `s` to `e`, with buffers of
/// `frames` frames at `rate`; the end is set before the first buffer
/// (`early`) or once the first buffer after the start is through.
fn audio_pause(rate: u64, frames: u32, s: u64, e: u64, early: bool) -> bool {
    let (mut rc, mut rs) = (Record::default(), Record::default());
    rc.ts_start = s;
    rs.ts_start = s;
    if early {
        end(&mut rc, e);
        end(&mut rs, e);
    }
    let len = mul_div64(u64::from(frames), NS, rate).max(1);
    let mut ended = early;
    let mut split = false;
    let mut ts = T0;
    for _ in 0..200 {
        let (mut bc, mut bs) = (
            Buffer {
                advanced: 0,
                frames,
                timestamp: ts,
            },
            Buffer {
                advanced: 0,
                frames,
                timestamp: ts,
            },
        );
        let differs =
            !split && rc.ts_start != 0 && rc.ts_end != 0 && rc.ts_start < ts && rc.ts_end < ts;
        let (dc, ds) = (
            C.audio(&mut rc, &mut bc, rate),
            SHIM.audio(&mut rs, &mut bs, rate),
        );
        let at_split = differs;
        split |= differs;
        let what = format!("{ts}: {rate} {frames} {s} {e} {early}");
        if split {
            // The C drops every buffer from here, with its record set; the
            // port passes each as it is, less the offset.
            assert!(dc, "C passes, {what}");
            assert_eq!((rc.ts_start, rc.ts_end), (s, e), "{what}");
            // An empty buffer ends the pause and is dropped; later ones
            // pass, with no pause on.
            assert_eq!(ds, at_split && frames == 0, "{what}");
            assert_eq!((rs.ts_start, rs.ts_end), (0, 0), "{what}");
            assert_eq!(
                bs,
                Buffer {
                    advanced: 0,
                    frames,
                    timestamp: ts.wrapping_sub(rs.ts_offset)
                },
                "{what}"
            );
        } else {
            assert_eq!((dc, bc, rc), (ds, bs, rs), "{what}");
        }
        if !ended && ts > s {
            end(&mut rc, e);
            end(&mut rs, e);
            ended = true;
        }
        ts += len;
    }
    // Both pauses end once the buffers pass the end: the port's always,
    // the C's only through a buffer that holds the end.
    if e < T0 + 150 * len {
        assert_eq!(
            (rs.ts_start, rs.ts_end),
            (0, 0),
            "{rate} {frames} {s} {e} {early}"
        );
        assert_eq!(split, rc.ts_start != 0, "{rate} {frames} {s} {e} {early}");
    }
    split
}

#[test]
fn whole_audio_pauses_at_every_rate() {
    let mut counts = std::collections::BTreeMap::new();
    let mut i = 0;
    for rate in [44100, 48000, 96000] {
        for frames in [0, 1, 480, 1024] {
            for start in 0..24 {
                let s = T0 + start * FRAME / 2;
                for stop in start..start + 12 {
                    let e = T0 + stop * FRAME / 2;
                    for early in [true, false] {
                        i += 1;
                        // Edges on every level: a pause that ends where it
                        // starts or starts at the first buffer, and the empty
                        // and one-frame buffers.
                        let edge = stop == start || start == 0 || frames <= 1;
                        if edge || obs_c_oracle::parity::keep(i) {
                            let split = audio_pause(rate, frames, s, e, early);
                            *counts.entry((frames, split)).or_insert(0) += 1;
                        }
                    }
                }
            }
        }
    }
    // Each buffer length meets pauses that end inside a buffer (the two
    // agree) and pauses that start and end inside one (only the C stays
    // paused).
    for frames in [1, 480, 1024] {
        for split in [false, true] {
            assert!(
                counts.get(&(frames, split)).copied().unwrap_or(0) > 0,
                "{frames} {split}: {counts:?}"
            );
        }
    }
}

#[test]
fn a_pause_and_unpause_inside_one_buffer_splits_once() {
    // R16-2 as the C meets it: both times inside the first buffer.
    let s = T0 + 5_000_000;
    let e = T0 + 10_000_000;
    let (mut rc, mut rs) = (
        Record {
            ts_start: s,
            ..Record::default()
        },
        Record {
            ts_start: s,
            ..Record::default()
        },
    );
    end(&mut rc, e);
    end(&mut rs, e);
    let mut b = Buffer {
        advanced: 0,
        frames: 1024,
        timestamp: T0,
    };
    let mut b2 = b;
    assert_eq!(
        C.audio(&mut rc, &mut b, 48000),
        SHIM.audio(&mut rs, &mut b2, 48000)
    );
    assert_eq!((b, rc), (b2, rs));
    for k in 1..50 {
        let ts = T0 + k * BUFFER_NS;
        let mut bc = Buffer {
            advanced: 0,
            frames: 1024,
            timestamp: ts,
        };
        let mut bs = bc;
        assert!(C.audio(&mut rc, &mut bc, 48000), "the C drops {ts}");
        assert!(!SHIM.audio(&mut rs, &mut bs, 48000), "the port passes {ts}");
    }
    assert_eq!((rc.ts_start, rc.ts_end), (s, e));
    assert_eq!((rs.ts_start, rs.ts_end), (0, 0));
}

// --------------------------------------------------------------------------
// proptests

/// Times near the top of the range, and near each other.
fn top() -> impl Strategy<Value = u64> {
    prop_oneof![
        (0_u64..=4 * BUFFER_NS).prop_map(|n| u64::MAX - n),
        (0_u64..=4).prop_map(|n| u64::MAX - n),
        any::<u64>(),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(8192),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn any_decision_near_the_top(a in top(), b in top(), c in top(), frames in any::<u32>()) {
        check_off(a);
        check_video_end(b, c);
        check_video_drop(a, c);
        check_audio_start(a, b, c);
        check_audio_paused(a, b, c);
        check_pair(a, b);
        check_frames(frames, c);
    }

    #[test]
    fn any_buffer_near_the_top(
        s in top(),
        e_after in 0_u64..=2 * BUFFER_NS,
        timestamp in top(),
        frames in prop::sample::select(vec![0_u32, 1, 480, 1024]),
        rate in prop::sample::select(vec![44100_u64, 48000, 96000]),
    ) {
        let e = s.saturating_add(e_after);
        let mut rc = Record { ts_start: s, ..Record::default() };
        end(&mut rc, e);
        let mut rs = rc;
        let differs = rc.ts_start < timestamp && rc.ts_end != 0 && rc.ts_end < timestamp && rc.ts_start != 0;
        let mut bc = Buffer { advanced: 0, frames, timestamp };
        let mut bs = bc;
        let (dc, ds) = (C.audio(&mut rc, &mut bc, rate), SHIM.audio(&mut rs, &mut bs, rate));
        if differs {
            prop_assert!(dc);
            prop_assert_eq!(ds, frames == 0);
            prop_assert_eq!((rs.ts_start, rs.ts_end), (0, 0));
        } else {
            prop_assert_eq!((dc, bc, rc), (ds, bs, rs));
        }
    }

    #[test]
    fn any_input_on_the_shim(a in any::<u64>(), b in any::<u64>(), c in any::<u64>(), frames in any::<u32>()) {
        prop_assert_eq!(obs_encoder_pause_off(a), core::off(a));
        prop_assert_eq!(obs_encoder_pause_video_end(b, c), core::video_end(b, c));
        prop_assert_eq!(obs_encoder_pause_video_drop(a, c), core::video_drop(a, c));
        prop_assert_eq!(start_of(obs_encoder_pause_audio_start(a, b, c)), core::audio_start(a, b, c));
        prop_assert_eq!(paused_of(obs_encoder_pause_audio_paused(a, b, c)), core::audio_paused(a, b, c));
        prop_assert_eq!(obs_encoder_pause_audio_end(a, b), core::audio_end(a, b));
        prop_assert_eq!(obs_encoder_pause_audio_cut_ns(a, b), core::audio_cut_ns(a, b));
        prop_assert_eq!(obs_encoder_pause_audio_kept(c), core::audio_kept(c));
        prop_assert_eq!(obs_encoder_pause_audio_left(frames, c), core::audio_left(frames, c));
        prop_assert_eq!(obs_encoder_pause_audio_drop(frames), core::audio_drop(frames));
    }
}
