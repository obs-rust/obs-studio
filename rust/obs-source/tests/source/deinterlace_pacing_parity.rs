//! Tier 3: the `obs_source_deinterlace_pacing_*` shims against the original
//! C (`obs-c-oracle`, `oracle/source_deinterlace_pacing.c`: the unmodified
//! `libobs/obs-source-deinterlace-pacing.c.h` with its functions renamed).
//!
//! Every function takes and returns plain integers, so a run compares the
//! bytes of the two results: the C's `bool` as the byte it returns, the
//! `uint32_t` and `uint64_t` results as their little-endian bytes.
//!
//! Runs:
//! - the three thresholds against the C header;
//! - each function over the special values of the full `u64` range and the
//!   thresholds +-1, every combination;
//! - proptests of each function over full-range `u64` (and `u32`) values,
//!   with the clock drawn near the frame's end and the lateness near the
//!   thresholds as often as anywhere;
//! - proptests of sequences through a model of
//!   `deinterlace_get_closest_frames()`, `ready_deinterlace_frames()` and
//!   `first_frame()` (the queue, the buffered walk and the stores, in the
//!   C's order), one copy driven by the C functions and one by the shims:
//!   sources of 25, 29.97, 50 and 59.94 frames a second into 30 and 60 fps
//!   canvases, buffered and unbuffered, with timestamp and arrival jitter,
//!   late deliveries, timestamps that jump or go backwards, queue resets
//!   (the first-frame path) and the buffering toggled. Every call's result
//!   bytes and the model's state after every tick must be equal. A second
//!   test checks that the sequences reach every branch of every function
//!   more than 100 times.
//!
//! No intentional differences: there is no pointer to check.

use std::collections::VecDeque;

use obs_c_oracle::source_deinterlace_pacing as oracle;
use obs_source::deinterlace_pacing::{FRAME_TOLERANCE, TS_JUMP_THRESHOLD, TWOX_TOLERANCE};
use obs_source::ffi::source_deinterlace_pacing as shim;
use proptest::prelude::*;
use proptest::strategy::ValueTree;

const MS: u64 = 1_000_000;
const TWOX: u64 = TWOX_TOLERANCE;
const JUMP: u64 = TS_JUMP_THRESHOLD;
const INT64_MAX: u64 = 0x7fff_ffff_ffff_ffff;
const TWO_32: u64 = 1 << 32;
/// `MAX_TS_VAR` (`obs-internal.h`), for the model's buffered walk.
const MAX_TS_VAR: u64 = 2_000_000_000;

#[test]
fn constants_match_the_c_header() {
    let rust = [FRAME_TOLERANCE, TWOX_TOLERANCE, TS_JUMP_THRESHOLD];
    for (which, (name, value)) in oracle::CONSTANT_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: a pure lookup.
        let c = unsafe { oracle::oracle_source_deinterlace_pacing_constant(which) };
        assert_eq!(value, c, "{name}");
    }
}

// --------------------------------------------------------------------------
// One call on each side, as bytes

fn c_hold(frame_ts: u64, offset: u64, half: u32, sys_time: u64) -> [u8; 1] {
    // SAFETY: plain values.
    [
        unsafe {
            oracle::oracle_obs_source_deinterlace_pacing_hold(frame_ts, offset, half, sys_time)
        },
    ]
}
fn r_hold(frame_ts: u64, offset: u64, half: u32, sys_time: u64) -> [u8; 1] {
    [u8::from(shim::obs_source_deinterlace_pacing_hold(
        frame_ts, offset, half, sys_time,
    ))]
}
fn c_wait(ts: u64, next_ts: u64, offset: u64, sys_time: u64) -> [u8; 1] {
    // SAFETY: plain values.
    [unsafe {
        oracle::oracle_obs_source_deinterlace_pacing_wait_for_next(ts, next_ts, offset, sys_time)
    }]
}
fn r_wait(ts: u64, next_ts: u64, offset: u64, sys_time: u64) -> [u8; 1] {
    [u8::from(shim::obs_source_deinterlace_pacing_wait_for_next(
        ts, next_ts, offset, sys_time,
    ))]
}
fn c_waiting_ts(ts: u64, next_ts: u64) -> [u8; 8] {
    // SAFETY: plain values.
    unsafe { oracle::oracle_obs_source_deinterlace_pacing_waiting_frame_ts(ts, next_ts) }
        .to_le_bytes()
}
fn r_waiting_ts(ts: u64, next_ts: u64) -> [u8; 8] {
    shim::obs_source_deinterlace_pacing_waiting_frame_ts(ts, next_ts).to_le_bytes()
}
fn c_half(ts: u64, prev_ts: u64) -> [u8; 4] {
    // SAFETY: plain values.
    unsafe { oracle::oracle_obs_source_deinterlace_pacing_half_duration(ts, prev_ts) }.to_le_bytes()
}
fn r_half(ts: u64, prev_ts: u64) -> [u8; 4] {
    shim::obs_source_deinterlace_pacing_half_duration(ts, prev_ts).to_le_bytes()
}
fn c_buffered(video_time: u64, last_frame_ts: u64) -> [u8; 8] {
    // SAFETY: plain values.
    unsafe {
        oracle::oracle_obs_source_deinterlace_pacing_buffered_offset(video_time, last_frame_ts)
    }
    .to_le_bytes()
}
fn r_buffered(video_time: u64, last_frame_ts: u64) -> [u8; 8] {
    shim::obs_source_deinterlace_pacing_buffered_offset(video_time, last_frame_ts).to_le_bytes()
}
fn c_reanchor(offset: u64, video_time: u64, frame_ts: u64, half: u32, interval: u64) -> [u8; 8] {
    // SAFETY: plain values.
    unsafe {
        oracle::oracle_obs_source_deinterlace_pacing_reanchor(
            offset, video_time, frame_ts, half, interval,
        )
    }
    .to_le_bytes()
}
fn r_reanchor(offset: u64, video_time: u64, frame_ts: u64, half: u32, interval: u64) -> [u8; 8] {
    shim::obs_source_deinterlace_pacing_reanchor(offset, video_time, frame_ts, half, interval)
        .to_le_bytes()
}

const SPECIAL: [u64; 24] = [
    0,
    1,
    2,
    TWOX - 1,
    TWOX,
    TWOX + 1,
    FRAME_TOLERANCE,
    16_666_666,
    20 * MS,
    33_333_333,
    40 * MS,
    JUMP - 1,
    JUMP,
    JUMP + 1,
    TWO_32 - 1,
    TWO_32,
    1_000_000_000_000,
    INT64_MAX,
    INT64_MAX + 1,
    INT64_MAX + 2,
    u64::MAX - JUMP,
    u64::MAX - TWOX,
    u64::MAX - 1,
    u64::MAX,
];

const SPECIAL_HALF: [u32; 10] = [
    0,
    1,
    500_000,
    1_000_000,
    8_341_666,
    16_666_666,
    20_000_000,
    35_000_000,
    u32::MAX - 1,
    u32::MAX,
];

#[test]
fn special_values_hold() {
    let mut runs = 0usize;
    for &frame_ts in &SPECIAL {
        for &offset in &SPECIAL {
            for &half in &SPECIAL_HALF {
                for &sys_time in &SPECIAL {
                    assert_eq!(
                        c_hold(frame_ts, offset, half, sys_time),
                        r_hold(frame_ts, offset, half, sys_time),
                        "hold({frame_ts}, {offset}, {half}, {sys_time})"
                    );
                    runs += 1;
                }
            }
        }
    }
    assert_eq!(runs, 24 * 24 * 10 * 24);
}

#[test]
fn special_values_wait() {
    for &ts in &SPECIAL {
        for &next_ts in &SPECIAL {
            assert_eq!(
                c_waiting_ts(ts, next_ts),
                r_waiting_ts(ts, next_ts),
                "{ts} {next_ts}"
            );
            for &offset in &SPECIAL {
                for &sys_time in &SPECIAL {
                    assert_eq!(
                        c_wait(ts, next_ts, offset, sys_time),
                        r_wait(ts, next_ts, offset, sys_time),
                        "wait_for_next({ts}, {next_ts}, {offset}, {sys_time})"
                    );
                }
            }
        }
    }
}

#[test]
fn special_values_half_and_buffered() {
    for &a in &SPECIAL {
        for &b in &SPECIAL {
            assert_eq!(c_half(a, b), r_half(a, b), "half_duration({a}, {b})");
            assert_eq!(
                c_buffered(a, b),
                r_buffered(a, b),
                "buffered_offset({a}, {b})"
            );
        }
    }
}

#[test]
fn special_values_reanchor() {
    let mut runs = 0usize;
    for &offset in &SPECIAL {
        for &video_time in &SPECIAL {
            for &frame_ts in &SPECIAL {
                for &half in &SPECIAL_HALF {
                    for &interval in &SPECIAL {
                        assert_eq!(
                            c_reanchor(offset, video_time, frame_ts, half, interval),
                            r_reanchor(offset, video_time, frame_ts, half, interval),
                            "reanchor({offset}, {video_time}, {frame_ts}, {half}, {interval})"
                        );
                        runs += 1;
                    }
                }
            }
        }
    }
    assert_eq!(runs, 24 * 24 * 24 * 10 * 24);
}

fn near(points: &'static [u64]) -> impl Strategy<Value = u64> {
    (proptest::sample::select(points), -3i64..=3).prop_map(|(p, d)| p.wrapping_add_signed(d))
}

fn any_time() -> impl Strategy<Value = u64> {
    prop_oneof![any::<u64>(), near(&SPECIAL)]
}

fn any_half() -> impl Strategy<Value = u32> {
    prop_oneof![
        any::<u32>(),
        0u32..40_000_000,
        proptest::sample::select(&SPECIAL_HALF[..])
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20_000))]

    #[test]
    fn proptest_hold(frame_ts in any_time(), offset in any_time(), half in any_half(), back in any_time()) {
        let end = frame_ts
            .wrapping_add(offset)
            .wrapping_add(2 * u64::from(half))
            .wrapping_sub(TWOX);
        for sys_time in [back, end.wrapping_sub(back % (2 * JUMP))] {
            prop_assert_eq!(c_hold(frame_ts, offset, half, sys_time), r_hold(frame_ts, offset, half, sys_time));
        }
    }

    #[test]
    fn proptest_wait(ts in any_time(), next_ts in any_time(), offset in any_time(), back in any_time()) {
        let end = next_ts.wrapping_add(offset);
        for sys_time in [back, end.wrapping_sub(back % 4)] {
            prop_assert_eq!(c_wait(ts, next_ts, offset, sys_time), r_wait(ts, next_ts, offset, sys_time));
        }
        prop_assert_eq!(c_waiting_ts(ts, next_ts), r_waiting_ts(ts, next_ts));
    }

    #[test]
    fn proptest_half_and_buffered(a in any_time(), b in any_time()) {
        prop_assert_eq!(c_half(a, b), r_half(a, b));
        prop_assert_eq!(c_buffered(a, b), r_buffered(a, b));
    }

    #[test]
    fn proptest_reanchor(
        offset in any_time(),
        frame_ts in any_time(),
        late in prop_oneof![any::<u64>(), 0u64..40_000_000, near(&SPECIAL)],
        video_time in any_time(),
        half in any_half(),
        interval in prop_oneof![any::<u64>(), Just(16_666_666u64), Just(33_333_333u64), near(&SPECIAL)],
    ) {
        // the lateness drawn as often as anywhere, and a free clock
        for video_time in [frame_ts.wrapping_add(offset).wrapping_add(late), video_time] {
            prop_assert_eq!(
                c_reanchor(offset, video_time, frame_ts, half, interval),
                r_reanchor(offset, video_time, frame_ts, half, interval)
            );
        }
    }
}

// --------------------------------------------------------------------------
// Sequences through a model of the caller

/// The six functions, from one side.
trait Pacing {
    fn hold(&self, frame_ts: u64, offset: u64, half: u32, sys_time: u64) -> [u8; 1];
    fn wait(&self, ts: u64, next_ts: u64, offset: u64, sys_time: u64) -> [u8; 1];
    fn waiting_ts(&self, ts: u64, next_ts: u64) -> [u8; 8];
    fn half(&self, ts: u64, prev_ts: u64) -> [u8; 4];
    fn buffered(&self, video_time: u64, last_frame_ts: u64) -> [u8; 8];
    fn reanchor(
        &self,
        offset: u64,
        video_time: u64,
        frame_ts: u64,
        half: u32,
        interval: u64,
    ) -> [u8; 8];
}

struct C;
struct Rust;

impl Pacing for C {
    fn hold(&self, a: u64, b: u64, h: u32, s: u64) -> [u8; 1] {
        c_hold(a, b, h, s)
    }
    fn wait(&self, a: u64, b: u64, o: u64, s: u64) -> [u8; 1] {
        c_wait(a, b, o, s)
    }
    fn waiting_ts(&self, a: u64, b: u64) -> [u8; 8] {
        c_waiting_ts(a, b)
    }
    fn half(&self, a: u64, b: u64) -> [u8; 4] {
        c_half(a, b)
    }
    fn buffered(&self, a: u64, b: u64) -> [u8; 8] {
        c_buffered(a, b)
    }
    fn reanchor(&self, o: u64, v: u64, f: u64, h: u32, i: u64) -> [u8; 8] {
        c_reanchor(o, v, f, h, i)
    }
}

impl Pacing for Rust {
    fn hold(&self, a: u64, b: u64, h: u32, s: u64) -> [u8; 1] {
        r_hold(a, b, h, s)
    }
    fn wait(&self, a: u64, b: u64, o: u64, s: u64) -> [u8; 1] {
        r_wait(a, b, o, s)
    }
    fn waiting_ts(&self, a: u64, b: u64) -> [u8; 8] {
        r_waiting_ts(a, b)
    }
    fn half(&self, a: u64, b: u64) -> [u8; 4] {
        r_half(a, b)
    }
    fn buffered(&self, a: u64, b: u64) -> [u8; 8] {
        r_buffered(a, b)
    }
    fn reanchor(&self, o: u64, v: u64, f: u64, h: u32, i: u64) -> [u8; 8] {
        r_reanchor(o, v, f, h, i)
    }
}

/// Which branches a run took, for the coverage test.
#[derive(Debug, Default, Clone, Copy)]
struct Reached {
    held: usize,
    not_held: usize,
    jumped: usize,
    waited: usize,
    not_waited: usize,
    half_from_pair: usize,
    half_from_frame_ts: usize,
    half_truncated: usize,
    buffered: usize,
    reanchor_none: usize,
    reanchor_early: usize,
    reanchor_late: usize,
    reanchor_kept: usize,
    first_frame: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Frame {
    timestamp: u64,
    prev_frame: bool,
}

/// The fields of `struct obs_source` the deinterlace pacing reads and
/// writes, and the frames it holds.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Source {
    async_frames: VecDeque<Frame>,
    async_unbuffered: bool,
    last_frame_ts: u64,
    last_sys_timestamp: u64,
    deinterlace_offset: u64,
    deinterlace_frame_ts: u64,
    deinterlace_half_duration: u32,
    cur_async_frame: Option<Frame>,
    prev_async_frame: Option<Frame>,
}

fn u64_of(bytes: [u8; 8]) -> u64 {
    u64::from_le_bytes(bytes)
}

/// Runs one side through the model, logging every call's result bytes.
struct Run<'a, P: Pacing> {
    p: &'a P,
    s: Source,
    log: Vec<u8>,
    reached: Reached,
}

impl<P: Pacing> Run<'_, P> {
    /// `frame_out_of_bounds()` (`obs-internal.h`).
    fn frame_out_of_bounds(&self, ts: u64) -> bool {
        let last = self.s.last_frame_ts;
        if ts < last {
            last - ts > MAX_TS_VAR
        } else {
            ts - last > MAX_TS_VAR
        }
    }

    /// `ready_deinterlace_frames()`. The C's `next_frame`, `frame` and
    /// `prev_frame` pointers are indices into the queue, moved down when
    /// the front is erased.
    fn ready(&mut self, sys_time: u64) -> bool {
        let sys_offset = sys_time.wrapping_sub(self.s.last_sys_timestamp);

        if self.s.async_unbuffered {
            let s = &mut self.s;
            while s.async_frames.len() > 2 {
                s.async_frames.pop_front();
            }
            if s.async_frames.len() == 2 {
                let mut has_prev_frame = true;
                if s.async_unbuffered && s.deinterlace_offset != 0 {
                    let timestamp = s.async_frames[0].timestamp;
                    let after = s.async_frames[1].timestamp;
                    let wait = self
                        .p
                        .wait(timestamp, after, s.deinterlace_offset, sys_time);
                    self.log.extend(wait);
                    if wait[0] != 0 {
                        self.reached.waited += 1;
                        has_prev_frame = false;
                        let ts = self.p.waiting_ts(timestamp, after);
                        self.log.extend(ts);
                        s.deinterlace_frame_ts = u64_of(ts);
                    } else {
                        self.reached.not_waited += 1;
                    }
                }
                s.async_frames[0].prev_frame = has_prev_frame;
            }
            s.last_frame_ts = s.async_frames[0].timestamp;
            return true;
        }

        let mut frame_time = self.s.async_frames[0].timestamp;
        if self.frame_out_of_bounds(frame_time) {
            self.s.last_frame_ts = self.s.async_frames[0].timestamp;
            self.s.deinterlace_offset = 0;
            return true;
        }
        let s = &mut self.s;
        let mut frame_offset = frame_time.wrapping_sub(s.last_frame_ts);
        s.last_frame_ts = s.last_frame_ts.wrapping_add(sys_offset);

        let mut next = 0usize;
        let mut frame: Option<usize> = None;
        // Some(None): the C's prev_frame points to a frame already erased
        let mut prev: Option<Option<usize>> = None;
        while s.last_frame_ts > s.async_frames[next].timestamp {
            if s.last_frame_ts - s.async_frames[next].timestamp < FRAME_TOLERANCE {
                break;
            }
            if prev.is_some() {
                // da_erase(0) and remove_async_frame(prev_frame): it is
                // the front
                s.async_frames.pop_front();
                prev = Some(None);
                frame = frame.map(|f| f - 1);
                next -= 1;
            }
            if s.async_frames.len() <= 2 {
                let mut exit = true;
                if prev.is_some() {
                    // prev_frame->prev_frame = true on the erased frame
                } else if frame.is_none() && s.async_frames.len() == 2 {
                    exit = false;
                }
                if exit {
                    s.deinterlace_offset = 0;
                    return true;
                }
            }
            let idx = if frame.is_some() { 2 } else { 1 };
            prev = frame.map(Some);
            frame = Some(next);
            next = idx;

            if s.async_frames[next].timestamp.wrapping_sub(frame_time) > MAX_TS_VAR {
                s.last_frame_ts = s.async_frames[next].timestamp.wrapping_sub(frame_offset);
                s.deinterlace_offset = 0;
            }
            frame_time = s.async_frames[next].timestamp;
            frame_offset = frame_time.wrapping_sub(s.last_frame_ts);
        }
        if let Some(Some(p)) = prev {
            s.async_frames[p].prev_frame = true;
        }
        frame.is_some()
    }

    /// `deinterlace_get_closest_frames()`, then `async_tick()`'s store of
    /// `last_sys_timestamp`.
    fn tick(&mut self, sys_time: u64, video_time: u64, interval: u64) {
        self.closest(sys_time, video_time, interval);
        self.s.last_sys_timestamp = sys_time;
    }

    fn closest(&mut self, sys_time: u64, video_time: u64, interval: u64) {
        // deinterlace_process_last_frame(): the shown frames go
        self.s.prev_async_frame = None;
        self.s.cur_async_frame = None;

        if self.s.async_unbuffered && self.s.deinterlace_offset != 0 {
            let s = &self.s;
            let held = self.p.hold(
                s.deinterlace_frame_ts,
                s.deinterlace_offset,
                s.deinterlace_half_duration,
                sys_time,
            );
            self.log.extend(held);
            let end = s
                .deinterlace_frame_ts
                .wrapping_add(s.deinterlace_offset)
                .wrapping_add(2 * u64::from(s.deinterlace_half_duration))
                .wrapping_sub(TWOX);
            if held[0] != 0 {
                self.reached.held += 1;
                // the C keeps the frames it shows; the model only needs
                // the pacing state
                return;
            }
            if sys_time < end {
                self.reached.jumped += 1;
            } else {
                self.reached.not_held += 1;
            }
        }

        if self.s.async_frames.is_empty() {
            return;
        }

        // first_frame()
        let first = if self.s.last_frame_ts != 0 {
            false
        } else {
            if self.s.async_frames.len() >= 2 {
                self.s.async_frames[0].prev_frame = true;
            }
            self.reached.first_frame += 1;
            true
        };
        if !(first || self.ready(sys_time)) {
            return;
        }

        let s = &mut self.s;
        let mut cur = s.async_frames.pop_front().expect("a frame");
        let half;
        if !s.async_frames.is_empty() && cur.prev_frame {
            let prev = cur;
            cur = s.async_frames.pop_front().expect("a second frame");
            s.prev_async_frame = Some(prev);
            half = self.p.half(cur.timestamp, prev.timestamp);
            self.reached.half_from_pair += 1;
        } else {
            half = self.p.half(cur.timestamp, s.deinterlace_frame_ts);
            self.reached.half_from_frame_ts += 1;
        }
        self.log.extend(half);
        s.cur_async_frame = Some(cur);
        s.deinterlace_half_duration = u32::from_le_bytes(half);
        let full = if let Some(prev) = s.prev_async_frame {
            cur.timestamp.wrapping_sub(prev.timestamp)
        } else {
            cur.timestamp.wrapping_sub(s.deinterlace_frame_ts)
        };
        if full / 2 > u64::from(u32::MAX) {
            self.reached.half_truncated += 1;
        }

        if s.last_frame_ts == 0 {
            s.last_frame_ts = cur.timestamp;
        }
        s.deinterlace_frame_ts = cur.timestamp;

        if !s.async_unbuffered {
            let offset = self.p.buffered(video_time, s.last_frame_ts);
            self.log.extend(offset);
            s.deinterlace_offset = u64_of(offset);
            self.reached.buffered += 1;
            return;
        }

        let before = s.deinterlace_offset;
        let new_offset = video_time.wrapping_sub(s.deinterlace_frame_ts);
        let offset = self.p.reanchor(
            s.deinterlace_offset,
            video_time,
            s.deinterlace_frame_ts,
            s.deinterlace_half_duration,
            interval,
        );
        self.log.extend(offset);
        s.deinterlace_offset = u64_of(offset);
        if before == 0 {
            self.reached.reanchor_none += 1;
        } else if new_offset.wrapping_sub(before).cast_signed() < 0 {
            self.reached.reanchor_early += 1;
        } else if s.deinterlace_offset == before {
            self.reached.reanchor_kept += 1;
        } else {
            self.reached.reanchor_late += 1;
        }
    }
}

/// One event between ticks.
#[derive(Debug, Clone, Copy)]
enum Event {
    /// The next frame, `jitter_ns` off its time, arriving `delay_ns` late.
    Frame { jitter_ns: i64, delay_ns: u64 },
    /// A timestamp jump of the source's clock.
    Jump { by: u64, backwards: bool },
    /// A frame stamped anywhere.
    Wild { timestamp: u64 },
    /// `obs_source_output_video(NULL)` or a full queue: `last_frame_ts`
    /// 0 and the queue emptied.
    Reset,
    /// `obs_source_set_async_unbuffered()`.
    ToggleBuffering,
    /// A canvas tick late by `late_ns`.
    LateTick { late_ns: u64 },
}

#[derive(Debug, Clone)]
struct Sequence {
    frame_rate: (u64, u64),
    canvas_rate: (u64, u64),
    unbuffered: bool,
    /// Where the frame and canvas clocks start.
    frame_base: u64,
    canvas_base: u64,
    /// One entry per tick: the events before it.
    ticks: Vec<Vec<Event>>,
}

const FRAME_RATES: [(u64, u64); 4] = [(25, 1), (30000, 1001), (50, 1), (60000, 1001)];
const CANVAS_RATES: [(u64, u64); 2] = [(30, 1), (60, 1)];

fn rate_time(n: u64, (num, den): (u64, u64)) -> u64 {
    n * 1_000_000_000 * den / num
}

fn event() -> impl Strategy<Value = Event> {
    prop_oneof![
        40 => (-2_000_000i64..=2_000_000, prop_oneof![Just(0u64), 0u64..5_000_000, 0u64..60_000_000])
            .prop_map(|(jitter_ns, delay_ns)| Event::Frame { jitter_ns, delay_ns }),
        1 => (prop_oneof![1u64..100_000_000, 1u64..3_000_000_000, any::<u64>()], any::<bool>())
            .prop_map(|(by, backwards)| Event::Jump { by, backwards }),
        1 => any::<u64>().prop_map(|timestamp| Event::Wild { timestamp }),
        1 => Just(Event::Reset),
        1 => Just(Event::ToggleBuffering),
        2 => prop_oneof![1u64..20_000_000, 20_000_000u64..200_000_000]
            .prop_map(|late_ns| Event::LateTick { late_ns }),
    ]
}

fn sequence() -> impl Strategy<Value = Sequence> {
    (
        proptest::sample::select(&FRAME_RATES[..]),
        proptest::sample::select(&CANVAS_RATES[..]),
        any::<bool>(),
        prop_oneof![Just(0u64), 1u64..10_000_000_000, any::<u64>()],
        prop_oneof![Just(1_000_000_000_000u64), any::<u64>()],
        proptest::collection::vec(proptest::collection::vec(event(), 0..3), 2..300),
    )
        .prop_map(
            |(frame_rate, canvas_rate, unbuffered, frame_base, canvas_base, ticks)| Sequence {
                frame_rate,
                canvas_rate,
                unbuffered,
                frame_base,
                canvas_base,
                ticks,
            },
        )
}

/// A sequence run: a frame is due every frame duration on the canvas clock
/// (arrivals are queued at the tick at or after them), plus the events.
fn run<P: Pacing>(p: &P, seq: &Sequence) -> (Vec<u8>, Vec<Source>, Reached) {
    let mut r = Run {
        p,
        s: Source {
            async_unbuffered: seq.unbuffered,
            ..Source::default()
        },
        log: Vec::new(),
        reached: Reached::default(),
    };
    let interval = 1_000_000_000 * seq.canvas_rate.1 / seq.canvas_rate.0;
    let mut states = Vec::with_capacity(seq.ticks.len());
    let mut clock = seq.frame_base;
    let mut next_frame = 0u64;
    let mut pending: Vec<(u64, u64)> = Vec::new(); // (arrival, timestamp)
    let mut late = 0u64;

    for (tick, events) in (1u64..).zip(&seq.ticks) {
        let video_time = seq
            .canvas_base
            .wrapping_add(rate_time(tick, seq.canvas_rate))
            .wrapping_add(late);
        // the frames due by this tick, with their jitter and delay
        while rate_time(next_frame, seq.frame_rate) <= rate_time(tick, seq.canvas_rate) + late {
            let arrival = rate_time(next_frame, seq.frame_rate);
            pending.push((
                arrival,
                clock.wrapping_add(rate_time(next_frame, seq.frame_rate)),
            ));
            next_frame += 1;
        }
        for &e in events {
            match e {
                Event::Frame {
                    jitter_ns,
                    delay_ns,
                } => {
                    if let Some(last) = pending.last_mut() {
                        last.1 = last.1.wrapping_add_signed(jitter_ns);
                        last.0 += delay_ns;
                    }
                }
                Event::Jump { by, backwards } => {
                    clock = if backwards {
                        clock.wrapping_sub(by)
                    } else {
                        clock.wrapping_add(by)
                    };
                }
                Event::Wild { timestamp } => {
                    pending.push((0, timestamp));
                }
                Event::Reset => {
                    r.s.last_frame_ts = 0;
                    r.s.async_frames.clear();
                }
                Event::ToggleBuffering => {
                    r.s.async_unbuffered = !r.s.async_unbuffered;
                }
                Event::LateTick { late_ns } => {
                    late += late_ns;
                }
            }
        }
        let now = rate_time(tick, seq.canvas_rate) + late;
        let mut kept = Vec::new();
        for (arrival, timestamp) in pending.drain(..) {
            if arrival <= now {
                if r.s.async_frames.len() < 30 {
                    r.s.async_frames.push_back(Frame {
                        timestamp,
                        prev_frame: false,
                    });
                } else {
                    // MAX_ASYNC_FRAMES: the cache is dropped
                    r.s.last_frame_ts = 0;
                }
            } else {
                kept.push((arrival, timestamp));
            }
        }
        pending = kept;

        r.tick(video_time, video_time, interval);
        states.push(r.s.clone());
    }
    (r.log, states, r.reached)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(3000))]

    #[test]
    fn parity_sequences(seq in sequence()) {
        let (c_log, c_states, _) = run(&C, &seq);
        let (r_log, r_states, _) = run(&Rust, &seq);
        prop_assert_eq!(c_log, r_log);
        prop_assert_eq!(c_states, r_states);
    }
}

/// The sequences reach every branch of every function, more than 100 times
/// each, at every rate pair.
#[test]
fn sequences_reach_every_branch() {
    let mut runner = proptest::test_runner::TestRunner::deterministic();
    let mut total = Reached::default();
    let mut pairs = std::collections::BTreeMap::new();
    for _ in 0..2000 {
        let seq = sequence()
            .new_tree(&mut runner)
            .expect("a sequence")
            .current();
        let (c_log, c_states, reached) = run(&C, &seq);
        let (r_log, r_states, _) = run(&Rust, &seq);
        assert_eq!(c_log, r_log, "{seq:?}");
        assert_eq!(c_states, r_states, "{seq:?}");
        *pairs
            .entry((seq.frame_rate, seq.canvas_rate))
            .or_insert(0usize) += 1;
        total.held += reached.held;
        total.not_held += reached.not_held;
        total.jumped += reached.jumped;
        total.waited += reached.waited;
        total.not_waited += reached.not_waited;
        total.half_from_pair += reached.half_from_pair;
        total.half_from_frame_ts += reached.half_from_frame_ts;
        total.half_truncated += reached.half_truncated;
        total.buffered += reached.buffered;
        total.reanchor_none += reached.reanchor_none;
        total.reanchor_early += reached.reanchor_early;
        total.reanchor_late += reached.reanchor_late;
        total.reanchor_kept += reached.reanchor_kept;
        total.first_frame += reached.first_frame;
    }
    let counts = [
        ("held", total.held),
        ("not held", total.not_held),
        ("jumped", total.jumped),
        ("waited", total.waited),
        ("not waited", total.not_waited),
        ("half from the pair", total.half_from_pair),
        ("half from frame_ts", total.half_from_frame_ts),
        ("half truncated", total.half_truncated),
        ("buffered offset", total.buffered),
        ("re-anchor with no offset", total.reanchor_none),
        ("re-anchor early", total.reanchor_early),
        ("re-anchor late", total.reanchor_late),
        ("offset kept", total.reanchor_kept),
        ("first frame", total.first_frame),
    ];
    eprintln!("{counts:?} {pairs:?}");
    for (name, count) in counts {
        assert!(count > 100, "{name}: {count} ({total:?})");
    }
    assert_eq!(
        pairs.len(),
        FRAME_RATES.len() * CANVAS_RATES.len(),
        "{pairs:?}"
    );
}
