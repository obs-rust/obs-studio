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
//!   more than 100 times, the half duration's fallbacks (#1965) included;
//! - `first_frame_cadences` (#1965): the first frame of a new source, and
//!   the first after a pause, at those rates and clean cadences, against
//!   the half duration of before #1965 and one that knows the source's
//!   field duration: which fields it shows, and that every later state is
//!   the one of before;
//! - #2009: the model also keeps the async cache (`cache_video()`,
//!   `clean_cache()`, `remove_async_frame()`, `free_async_cache()` and the
//!   render's release of the previous frame), so a frame reused from the
//!   cache starts with the `prev_frame` its entry was left with.
//!   `behind_cadence` runs the `behind` cases of
//!   `test_deinterlace_2x_field_order`, and `buffered_walk_cadences` and
//!   `buffered_walk_sequences` run the cadences and the sequences above
//!   through the walk before #2009 and after: a run changes exactly when a
//!   frame reused with the flag left on was paired, and each half of the
//!   fix alone (the walk, or the clear in `cache_video()`) gives the same
//!   runs.
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
fn c_half(ts: u64, prev_ts: u64, prev_half: u32, interval: u64) -> [u8; 4] {
    // SAFETY: plain values.
    unsafe {
        oracle::oracle_obs_source_deinterlace_pacing_half_duration(ts, prev_ts, prev_half, interval)
    }
    .to_le_bytes()
}
fn r_half(ts: u64, prev_ts: u64, prev_half: u32, interval: u64) -> [u8; 4] {
    shim::obs_source_deinterlace_pacing_half_duration(ts, prev_ts, prev_half, interval)
        .to_le_bytes()
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
            for &prev_half in &SPECIAL_HALF {
                for interval in [0, 16_666_666, 33_333_333, u64::MAX] {
                    assert_eq!(
                        c_half(a, b, prev_half, interval),
                        r_half(a, b, prev_half, interval),
                        "half_duration({a}, {b}, {prev_half}, {interval})"
                    );
                }
            }
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
    fn proptest_half_and_buffered(a in any_time(), b in any_time(), prev_half in any_half(), interval in any_time()) {
        prop_assert_eq!(c_half(a, b, prev_half, interval), r_half(a, b, prev_half, interval));
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
    fn half(&self, ts: u64, prev_ts: u64, prev_half: u32, interval: u64) -> [u8; 4];
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
    fn half(&self, a: u64, b: u64, h: u32, i: u64) -> [u8; 4] {
        c_half(a, b, h, i)
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
    fn half(&self, a: u64, b: u64, h: u32, i: u64) -> [u8; 4] {
        r_half(a, b, h, i)
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
    /// #1965: no usable previous timestamp (0, at or after the frame, or a
    /// gap past twice `TS_JUMP_THRESHOLD`), and which fallback it took.
    half_no_previous: usize,
    half_backwards: usize,
    half_after_jump: usize,
    half_kept: usize,
    half_interval: usize,
    buffered: usize,
    reanchor_none: usize,
    reanchor_early: usize,
    reanchor_late: usize,
    reanchor_kept: usize,
    first_frame: usize,
    /// #2009: the buffered walk's exit with two frames left after it
    /// erased `prev_frame`, and the picks that paired a front frame whose
    /// `prev_frame` no call of this tick set (one reused from the cache
    /// with the flag left on).
    exit_after_erase: usize,
    stale_pairs: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Frame {
    timestamp: u64,
    prev_frame: bool,
    /// The `async_cache` entry the frame is (`Slot::id`).
    slot: u64,
}

/// An `async_cache` entry (`struct async_frame`). A frame's `prev_frame`
/// is kept on its `Frame` while the frame is queued or shown, and here
/// while the entry is unused: what the next frame to reuse it starts with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Slot {
    id: u64,
    used: bool,
    unused_count: u32,
    prev_frame: bool,
}

/// `MAX_UNUSED_FRAME_DURATION` and `MAX_ASYNC_FRAMES` (`obs-source.c.h`).
const MAX_UNUSED_FRAME_DURATION: u32 = 5;
const MAX_ASYNC_FRAMES: usize = 30;

/// The buffered walk's exit with two frames left after it erased
/// `prev_frame` (#2009).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Walk {
    /// Before #2009: `prev_frame->prev_frame = true` on the frame just
    /// erased and returned to the cache.
    Upstream,
    /// No flag: the front frame is shown alone (#2009).
    NoFlag,
    /// The front frame flagged: it is shown as the previous frame of a
    /// pair with the frame after it.
    FlagFront,
}

/// The walk and the cache of the C (#2009).
const FIXED: (Walk, bool) = (Walk::NoFlag, true);

/// The fields of `struct obs_source` the deinterlace pacing reads and
/// writes, and the frames it holds.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Source {
    async_cache: Vec<Slot>,
    next_slot: u64,
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
    /// The buffered walk's exit after an erase, and whether
    /// `cache_video()` clears `prev_frame` on a reused frame (#2009).
    walk: Walk,
    clear_on_reuse: bool,
    /// A call of this tick set the front frame's `prev_frame`.
    flagged: bool,
}

impl<'a, P: Pacing> Run<'a, P> {
    fn new(p: &'a P, unbuffered: bool, (walk, clear_on_reuse): (Walk, bool)) -> Self {
        Run {
            p,
            s: Source {
                async_unbuffered: unbuffered,
                ..Source::default()
            },
            log: Vec::new(),
            reached: Reached::default(),
            walk,
            clear_on_reuse,
            flagged: false,
        }
    }

    /// `obs_source_output_video()`: `cache_video()` (the first unused
    /// cache entry, then `clean_cache()`, else a new entry) and the push.
    /// With a full queue the cache is freed and the frame dropped.
    fn output_video(&mut self, timestamp: u64) {
        if self.s.async_frames.len() >= MAX_ASYNC_FRAMES {
            self.free_async_cache();
            self.s.last_frame_ts = 0;
            return;
        }
        let s = &mut self.s;
        let mut reused = None;
        if let Some(slot) = s.async_cache.iter_mut().find(|slot| !slot.used) {
            slot.used = true;
            slot.unused_count = 0;
            if self.clear_on_reuse {
                slot.prev_frame = false;
            }
            reused = Some((slot.id, slot.prev_frame));
        }
        // clean_cache()
        s.async_cache.retain_mut(|slot| {
            if slot.used {
                return true;
            }
            slot.unused_count += 1;
            slot.unused_count != MAX_UNUSED_FRAME_DURATION
        });
        let (slot, prev_frame) = reused.unwrap_or_else(|| {
            let id = s.next_slot;
            s.next_slot += 1;
            s.async_cache.push(Slot {
                id,
                used: true,
                unused_count: 0,
                prev_frame: false,
            });
            (id, false)
        });
        s.async_frames.push_back(Frame {
            timestamp,
            prev_frame,
            slot,
        });
    }

    /// `remove_async_frame()`: the flag cleared, the entry unused.
    fn remove_async_frame(&mut self, frame: Frame) {
        if let Some(slot) = self.s.async_cache.iter_mut().find(|s| s.id == frame.slot) {
            slot.used = false;
            slot.prev_frame = false;
        }
    }

    /// `free_async_cache()`: `obs_source_output_video(NULL)` or a full
    /// queue.
    fn free_async_cache(&mut self) {
        self.s.async_cache.clear();
        self.s.async_frames.clear();
        self.s.cur_async_frame = None;
        self.s.prev_async_frame = None;
    }

    /// `deinterlace_update_async_video()` at the render after the tick:
    /// the previous frame of a pair goes back to the cache.
    fn render(&mut self) {
        if let Some(prev) = self.s.prev_async_frame.take() {
            self.remove_async_frame(prev);
        }
    }

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
            while self.s.async_frames.len() > 2 {
                let erased = self.s.async_frames.pop_front().expect("a frame");
                self.remove_async_frame(erased);
            }
            let s = &mut self.s;
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
                self.flagged = has_prev_frame;
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
        let mut frame_offset = frame_time.wrapping_sub(self.s.last_frame_ts);
        self.s.last_frame_ts = self.s.last_frame_ts.wrapping_add(sys_offset);

        let mut next = 0usize;
        let mut frame: Option<usize> = None;
        let mut prev: Option<usize> = None;
        while self.s.last_frame_ts > self.s.async_frames[next].timestamp {
            if self.s.last_frame_ts - self.s.async_frames[next].timestamp < FRAME_TOLERANCE {
                break;
            }
            let mut erased = None;
            if prev.is_some() {
                // da_erase(0) and remove_async_frame(prev_frame): it is
                // the front
                let front = self.s.async_frames.pop_front().expect("a frame");
                self.remove_async_frame(front);
                erased = Some(front);
                frame = frame.map(|f| f - 1);
                next -= 1;
            }
            if self.s.async_frames.len() <= 2 {
                let mut exit = true;
                if let Some(erased) = erased {
                    self.reached.exit_after_erase += 1;
                    match self.walk {
                        Walk::Upstream => {
                            // prev_frame->prev_frame = true on the erased
                            // frame, back in the cache
                            if let Some(slot) =
                                self.s.async_cache.iter_mut().find(|s| s.id == erased.slot)
                            {
                                slot.prev_frame = true;
                            }
                        }
                        Walk::NoFlag => {}
                        Walk::FlagFront => {
                            self.s.async_frames[0].prev_frame = true;
                            self.flagged = true;
                        }
                    }
                } else if frame.is_none() && self.s.async_frames.len() == 2 {
                    exit = false;
                }
                if exit {
                    self.s.deinterlace_offset = 0;
                    return true;
                }
            }
            let idx = if frame.is_some() { 2 } else { 1 };
            prev = frame;
            frame = Some(next);
            next = idx;

            let s = &mut self.s;
            if s.async_frames[next].timestamp.wrapping_sub(frame_time) > MAX_TS_VAR {
                s.last_frame_ts = s.async_frames[next].timestamp.wrapping_sub(frame_offset);
                s.deinterlace_offset = 0;
            }
            frame_time = s.async_frames[next].timestamp;
            frame_offset = frame_time.wrapping_sub(s.last_frame_ts);
        }
        if let Some(p) = prev {
            self.s.async_frames[p].prev_frame = true;
            self.flagged = true;
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
        // deinterlace_process_last_frame(): the shown frames go back to
        // the cache
        if let Some(prev) = self.s.prev_async_frame.take() {
            self.remove_async_frame(prev);
        }
        if let Some(cur) = self.s.cur_async_frame.take() {
            self.remove_async_frame(cur);
        }
        self.flagged = false;

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
                self.flagged = true;
            }
            self.reached.first_frame += 1;
            true
        };
        if !(first || self.ready(sys_time)) {
            return;
        }

        let s = &mut self.s;
        let mut cur = s.async_frames.pop_front().expect("a frame");
        let prev_half = s.deinterlace_half_duration;
        let half;
        if !s.async_frames.is_empty() && cur.prev_frame {
            if !self.flagged {
                self.reached.stale_pairs += 1;
            }
            let prev = cur;
            cur = s.async_frames.pop_front().expect("a second frame");
            s.prev_async_frame = Some(prev);
            half = self.p.half(
                cur.timestamp,
                prev.timestamp,
                s.deinterlace_half_duration,
                interval,
            );
            self.reached.half_from_pair += 1;
        } else {
            half = self.p.half(
                cur.timestamp,
                s.deinterlace_frame_ts,
                s.deinterlace_half_duration,
                interval,
            );
            self.reached.half_from_frame_ts += 1;
        }
        self.log.extend(half);
        s.cur_async_frame = Some(cur);
        s.deinterlace_half_duration = u32::from_le_bytes(half);
        let prev_ts = s
            .prev_async_frame
            .map_or(s.deinterlace_frame_ts, |prev| prev.timestamp);
        let fallback = if prev_ts == 0 {
            self.reached.half_no_previous += 1;
            true
        } else if cur.timestamp <= prev_ts {
            self.reached.half_backwards += 1;
            true
        } else if cur.timestamp - prev_ts > 2 * JUMP {
            self.reached.half_after_jump += 1;
            true
        } else {
            false
        };
        if fallback {
            if prev_half != 0 && u64::from(prev_half) <= JUMP {
                self.reached.half_kept += 1;
            } else {
                self.reached.half_interval += 1;
            }
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
    run_with(p, seq, FIXED)
}

/// `run()` with the walk and cache of `variant` (#2009).
fn run_with<P: Pacing>(
    p: &P,
    seq: &Sequence,
    variant: (Walk, bool),
) -> (Vec<u8>, Vec<Source>, Reached) {
    let mut r = Run::new(p, seq.unbuffered, variant);
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
                    r.free_async_cache();
                    r.s.last_frame_ts = 0;
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
                r.output_video(timestamp);
            } else {
                kept.push((arrival, timestamp));
            }
        }
        pending = kept;

        r.tick(video_time, video_time, interval);
        states.push(r.s.clone());
        r.render();
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
        total.half_no_previous += reached.half_no_previous;
        total.half_backwards += reached.half_backwards;
        total.half_after_jump += reached.half_after_jump;
        total.half_kept += reached.half_kept;
        total.half_interval += reached.half_interval;
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
        ("half: no previous timestamp", total.half_no_previous),
        ("half: timestamp backwards", total.half_backwards),
        ("half: gap past the threshold", total.half_after_jump),
        ("half: the one before kept", total.half_kept),
        ("half: half the canvas interval", total.half_interval),
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

// --------------------------------------------------------------------------
// The first frame at real cadences (#1965)

/// The shims with the half duration of before #1965: half the gap from
/// the previous timestamp, whatever it is, truncated to 32 bits.
struct Before;

/// The shims with a half duration that knows the source's field duration
/// when there is no usable previous timestamp: what the first frame would
/// get if it were timed from a frame before it.
struct Ideal {
    field: u32,
}

/// The half duration of before #1965.
fn before_half(ts: u64, prev_ts: u64) -> [u8; 4] {
    let half = ts.wrapping_sub(prev_ts) / 2;
    half.to_le_bytes()[..4].try_into().expect("4 bytes")
}

impl Pacing for Before {
    fn hold(&self, a: u64, b: u64, h: u32, s: u64) -> [u8; 1] {
        r_hold(a, b, h, s)
    }
    fn wait(&self, a: u64, b: u64, o: u64, s: u64) -> [u8; 1] {
        r_wait(a, b, o, s)
    }
    fn waiting_ts(&self, a: u64, b: u64) -> [u8; 8] {
        r_waiting_ts(a, b)
    }
    fn half(&self, a: u64, b: u64, _h: u32, _i: u64) -> [u8; 4] {
        before_half(a, b)
    }
    fn buffered(&self, a: u64, b: u64) -> [u8; 8] {
        r_buffered(a, b)
    }
    fn reanchor(&self, o: u64, v: u64, f: u64, h: u32, i: u64) -> [u8; 8] {
        r_reanchor(o, v, f, h, i)
    }
}

impl Pacing for Ideal {
    fn hold(&self, a: u64, b: u64, h: u32, s: u64) -> [u8; 1] {
        r_hold(a, b, h, s)
    }
    fn wait(&self, a: u64, b: u64, o: u64, s: u64) -> [u8; 1] {
        r_wait(a, b, o, s)
    }
    fn waiting_ts(&self, a: u64, b: u64) -> [u8; 8] {
        r_waiting_ts(a, b)
    }
    fn half(&self, a: u64, b: u64, _h: u32, _i: u64) -> [u8; 4] {
        if b != 0 && a > b && a - b <= 2 * JUMP {
            before_half(a, b)
        } else {
            self.field.to_le_bytes()
        }
    }
    fn buffered(&self, a: u64, b: u64) -> [u8; 8] {
        r_buffered(a, b)
    }
    fn reanchor(&self, o: u64, v: u64, f: u64, h: u32, i: u64) -> [u8; 8] {
        r_reanchor(o, v, f, h, i)
    }
}

/// Where the measured frame comes from.
#[derive(Debug, Clone, Copy)]
enum Start {
    /// A source that never showed a frame, its first frame stamped at
    /// `base`.
    New { base: u64 },
    /// A source that pauses for 500 ms after 20 frames, its timestamps
    /// following its arrivals; with `reset`, `obs_source_output_video(NULL)`
    /// in the pause (`last_frame_ts` 0, the queue emptied).
    Pause { reset: bool },
}

const PAUSE_AFTER: u64 = 20;
const PAUSE: u64 = 500 * MS;
const CANVAS_BASE: u64 = 1_000_000_000_000;
/// A capture timestamp 1000 s after boot (before #1965: 1.78 s), a media
/// timestamp of 5 s (2.5 s), and one that left 30 ms (held too long).
const NEW_BASES: [u64; 3] = [
    1_000_005_000_000,
    5_000_000_000,
    2 * (100 * TWO_32 + 30 * MS),
];
const HELD_TOO_LONG: u64 = 2 * (100 * TWO_32 + 30 * MS);

/// One run at a clean cadence: the model's state after every tick, the
/// field each tick shows (the timestamp of the frame and 1 or 2: the second
/// once the canvas clock reaches the frame's second field time,
/// `deinterlace_frame_ts + deinterlace_offset + deinterlace_half_duration -
/// TWOX_TOLERANCE`, as the render decides), and the ticks that picked a
/// frame.
struct Cadence {
    log: Vec<u8>,
    states: Vec<Source>,
    shown: Vec<Option<(u64, u8)>>,
    picks: Vec<usize>,
}

fn cadence<P: Pacing>(
    p: &P,
    frame_rate: (u64, u64),
    canvas_rate: (u64, u64),
    unbuffered: bool,
    start: Start,
) -> Cadence {
    cadence_with(p, frame_rate, canvas_rate, unbuffered, start, PAUSE, FIXED).0
}

/// `cadence()` with a pause of `pause` and the walk and cache of
/// `variant` (#2009), and what the run reached.
fn cadence_with<P: Pacing>(
    p: &P,
    frame_rate: (u64, u64),
    canvas_rate: (u64, u64),
    unbuffered: bool,
    start: Start,
    pause: u64,
    variant: (Walk, bool),
) -> (Cadence, Reached) {
    let mut r = Run::new(p, unbuffered, variant);
    let interval = 1_000_000_000 * canvas_rate.1 / canvas_rate.0;
    let (base, pause_at) = match start {
        Start::New { base } => (base, u64::MAX),
        Start::Pause { .. } => (5_000_000_000, PAUSE_AFTER),
    };
    let arrival = |k: u64| rate_time(k, frame_rate) + if k >= pause_at { pause } else { 0 };
    let mut out = Cadence {
        log: Vec::new(),
        states: Vec::new(),
        shown: Vec::new(),
        picks: Vec::new(),
    };
    let mut next_frame = 0u64;
    let mut shown_ts = None;
    let mut reset_done = false;
    for tick in 1u64..=200 {
        let now = rate_time(tick, canvas_rate);
        let video_time = CANVAS_BASE + now;
        if let Start::Pause { reset: true } = start
            && !reset_done
            && now >= arrival(PAUSE_AFTER - 1) + pause / 2
        {
            r.free_async_cache();
            r.s.last_frame_ts = 0;
            reset_done = true;
        }
        while arrival(next_frame) <= now {
            r.output_video(base + arrival(next_frame));
            next_frame += 1;
        }
        r.tick(video_time, video_time, interval);
        if let Some(cur) = r.s.cur_async_frame {
            shown_ts = Some(cur.timestamp);
            out.picks.push(out.states.len());
        }
        let s = &r.s;
        let frame2_ts = s
            .deinterlace_frame_ts
            .wrapping_add(s.deinterlace_offset)
            .wrapping_add(u64::from(s.deinterlace_half_duration))
            .wrapping_sub(TWOX);
        out.shown
            .push(shown_ts.map(|ts| (ts, if video_time >= frame2_ts { 2 } else { 1 })));
        out.states.push(r.s.clone());
        r.render();
    }
    out.log = r.log;
    (out, r.reached)
}

/// The measured frame of a run: the first frame picked, or the first after
/// the pause. Its pick, the first tick that shows its second field, if
/// any, and the next pick, as tick indices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Measured {
    pick: usize,
    second: Option<usize>,
    next_pick: usize,
}

fn measured(run: &Cadence, start: Start, frame_rate: (u64, u64)) -> Measured {
    let k = match start {
        Start::New { .. } => 0,
        Start::Pause { .. } => run
            .picks
            .iter()
            .position(|&t| {
                run.shown[t].expect("a frame").0
                    >= 5_000_000_000 + rate_time(PAUSE_AFTER, frame_rate)
            })
            .expect("a frame after the pause"),
    };
    let pick = run.picks[k];
    let ts = run.shown[pick].expect("a frame").0;
    let next_pick = run.picks[k + 1];
    let second = (pick..next_pick).find(|&t| run.shown[t] == Some((ts, 2)));
    Measured {
        pick,
        second,
        next_pick,
    }
}

/// At 25, 29.97, 50 and 59.94 frames a second into 30 and 60 fps, buffered
/// and unbuffered, for a new source (three first timestamps) and after a
/// pause (with and without a queue reset), the measured frame (the first,
/// or the first after the pause):
/// - the C and the shims agree, call for call and tick for tick;
/// - it shows at its pick the field `Ideal` shows (a new source: its first
///   field), and its second field before the next frame whenever `Ideal`
///   does, never later than `Ideal`; buffered always, unbuffered unless the
///   next frame is there at the next tick (25 into 30 and 50 into 60 fps),
///   where showing it would hold the frame a tick longer and move the
///   unbuffered anchor for good, so it is skipped as before;
/// - from the next pick on, every state is the one of before #1965
///   (`Before`), except where `Before` held a new source's first frame too
///   long, so its next pick came later.
#[test]
fn first_frame_cadences() {
    let mut rows = Vec::new();
    let (mut fixed, mut as_before) = (0usize, 0usize);
    for &frame_rate in &FRAME_RATES {
        for &canvas_rate in &CANVAS_RATES {
            for unbuffered in [false, true] {
                let starts = NEW_BASES
                    .iter()
                    .map(|&base| Start::New { base })
                    .chain([Start::Pause { reset: false }, Start::Pause { reset: true }]);
                for start in starts {
                    let what = format!(
                        "{frame_rate:?} into {canvas_rate:?}, unbuffered {unbuffered}, {start:?}"
                    );
                    let field = u32::try_from(rate_time(1, frame_rate) / 2).expect("a field");
                    let c = cadence(&C, frame_rate, canvas_rate, unbuffered, start);
                    let rust = cadence(&Rust, frame_rate, canvas_rate, unbuffered, start);
                    let before = cadence(&Before, frame_rate, canvas_rate, unbuffered, start);
                    let ideal =
                        cadence(&Ideal { field }, frame_rate, canvas_rate, unbuffered, start);
                    assert_eq!(c.log, rust.log, "{what}");
                    assert_eq!(c.states, rust.states, "{what}");

                    let new = measured(&rust, start, frame_rate);
                    let was = measured(&before, start, frame_rate);
                    let best = measured(&ideal, start, frame_rate);
                    let held_too_long =
                        matches!(start, Start::New { base } if base == HELD_TOO_LONG);
                    let first = rust.shown[new.pick].expect("a frame");
                    assert_eq!(new.pick, best.pick, "{what}");
                    assert_eq!(Some(first), ideal.shown[new.pick], "{what}");
                    if let Start::New { .. } = start {
                        assert_eq!(first.1, 1, "first field at the pick: {what}");
                    }
                    match (new.second, best.second) {
                        (Some(second), Some(ideal_second)) => {
                            assert!(second <= ideal_second, "{what}");
                            assert!(second > new.pick || ideal_second == new.pick, "{what}");
                        }
                        (None, None) => {}
                        (Some(_), None) => panic!("a second field Ideal does not show: {what}"),
                        (None, Some(_)) => {
                            // the next frame is there at the next tick
                            assert!(unbuffered, "{what}");
                            assert_eq!(new.next_pick, new.pick + 1, "{what}");
                            assert!(was.second.is_none() || held_too_long, "{what}");
                        }
                    }
                    if new.second.is_some() && was.second.is_none() {
                        fixed += 1;
                    }
                    if !held_too_long {
                        assert_eq!(was.next_pick, new.next_pick, "{what}");
                        assert_eq!(
                            rust.states[new.next_pick..],
                            before.states[new.next_pick..],
                            "{what}"
                        );
                        as_before += 1;
                    }
                    rows.push(format!(
                        "{what}: second field after {:?} ticks (before #1965 {:?}, ideal {:?}), next frame after {} (before {})",
                        new.second.map(|t| t - new.pick),
                        was.second.map(|t| t - was.pick),
                        best.second.map(|t| t - best.pick),
                        new.next_pick - new.pick,
                        was.next_pick - was.pick
                    ));
                }
            }
        }
    }
    eprintln!("{}", rows.join("\n"));
    eprintln!(
        "{} runs: the second field now shown in {fixed}, every later state as before in {as_before}",
        rows.len()
    );
    assert_eq!(rows.len(), 80);
    assert_eq!(as_before, 64);
}

/// 12.5 frames a second (25 fields), 80 ms apart, into 30 and 60 fps,
/// buffered and unbuffered, from the same starts: a regular stream below
/// 14.3 frames a second keeps its behaviour. The gap is within twice
/// `TS_JUMP_THRESHOLD`, so each frame is timed from the one before as
/// before #1965:
/// - the C and the shims agree;
/// - after a pause, every state from the stream's second frame up to the
///   pause is the one of before;
/// - from the measured frame's next pick on, every state is the one of
///   before (except where before held a new source's first frame too
///   long), and a new source shows its first field at its pick.
#[test]
fn low_rate_cadences() {
    let frame_rate = (25, 2);
    let mut runs = 0usize;
    for &canvas_rate in &CANVAS_RATES {
        for unbuffered in [false, true] {
            let starts = NEW_BASES
                .iter()
                .map(|&base| Start::New { base })
                .chain([Start::Pause { reset: false }, Start::Pause { reset: true }]);
            for start in starts {
                let what =
                    format!("12.5 fps into {canvas_rate:?}, unbuffered {unbuffered}, {start:?}");
                let c = cadence(&C, frame_rate, canvas_rate, unbuffered, start);
                let rust = cadence(&Rust, frame_rate, canvas_rate, unbuffered, start);
                let before = cadence(&Before, frame_rate, canvas_rate, unbuffered, start);
                assert_eq!(c.log, rust.log, "{what}");
                assert_eq!(c.states, rust.states, "{what}");
                let new = measured(&rust, start, frame_rate);
                let was = measured(&before, start, frame_rate);
                if let Start::Pause { .. } = start {
                    // from the stream's second frame to the pause: its
                    // first frame is a new source's
                    let second = rust.picks[1];
                    assert_eq!(second, before.picks[1], "{what}");
                    assert_eq!(
                        rust.states[second..new.pick],
                        before.states[second..new.pick],
                        "{what}"
                    );
                } else {
                    assert_eq!(rust.shown[new.pick].map(|f| f.1), Some(1), "{what}");
                }
                if !matches!(start, Start::New { base } if base == HELD_TOO_LONG) {
                    assert_eq!(was.next_pick, new.next_pick, "{what}");
                    assert_eq!(
                        rust.states[new.next_pick..],
                        before.states[new.next_pick..],
                        "{what}"
                    );
                }
                runs += 1;
            }
        }
    }
    assert_eq!(runs, 20);
}

// --------------------------------------------------------------------------
// The buffered walk's flag on an erased frame (#2009)

/// What a run shows and how it is paced, without the `prev_frame` flags
/// and cache entries: the pacing calls' bytes, and after every tick the
/// frames shown, the queue's timestamps and the pacing fields.
type Visible = (Vec<u8>, Vec<(Option<u64>, Option<u64>, Vec<u64>, [u64; 5])>);

fn visible(log: &[u8], states: &[Source]) -> Visible {
    let states = states
        .iter()
        .map(|s| {
            (
                s.prev_async_frame.map(|f| f.timestamp),
                s.cur_async_frame.map(|f| f.timestamp),
                s.async_frames.iter().map(|f| f.timestamp).collect(),
                [
                    s.last_frame_ts,
                    s.last_sys_timestamp,
                    s.deinterlace_offset,
                    s.deinterlace_frame_ts,
                    u64::from(s.deinterlace_half_duration),
                ],
            )
        })
        .collect();
    (log.to_vec(), states)
}

/// The variants checked against the fix: before #2009, each half of the
/// fix alone, and the front frame flagged instead.
const UPSTREAM: (Walk, bool) = (Walk::Upstream, false);
const CLEAR_ONLY: (Walk, bool) = (Walk::Upstream, true);
const NO_FLAG_ONLY: (Walk, bool) = (Walk::NoFlag, false);
const FLAG_FRONT: (Walk, bool) = (Walk::FlagFront, true);

/// Runs `one` for each variant and counts the runs that differ from the
/// fix. Before #2009 a run differs exactly when a frame reused from the
/// cache with the flag left on was paired; each half of the fix alone
/// gives the fix.
#[derive(Debug, Default)]
struct Changed {
    runs: usize,
    exits: usize,
    upstream: usize,
    stale: usize,
    flag_front: usize,
}

fn compare(changed: &mut Changed, what: &str, one: impl Fn((Walk, bool)) -> (Visible, Reached)) {
    let (fixed, reached) = one(FIXED);
    let (upstream, up_reached) = one(UPSTREAM);
    assert_eq!(reached.stale_pairs, 0, "{what}");
    assert_eq!(one(CLEAR_ONLY).0, fixed, "the cache clear alone: {what}");
    assert_eq!(one(NO_FLAG_ONLY).0, fixed, "the walk alone: {what}");
    changed.runs += 1;
    changed.exits += usize::from(reached.exit_after_erase > 0);
    changed.stale += usize::from(up_reached.stale_pairs > 0);
    if upstream != fixed {
        changed.upstream += 1;
    }
    assert_eq!(upstream != fixed, up_reached.stale_pairs > 0, "{what}");
    if one(FLAG_FRONT).0 != fixed {
        changed.flag_front += 1;
    }
}

/// The cadences of `first_frame_cadences` and `low_rate_cadences`, with
/// pauses of 150 ms (the real-libobs test) and 500 ms, through the walk
/// before #2009 and after: a run changes only where a frame reused from the
/// cache with the flag left on was paired (shown in place of the frame
/// before it). In those runs, before #2009, a frame was shown a frame early
/// after the pause; after it, the frames after the pause are shown in
/// order.
#[test]
fn buffered_walk_cadences() {
    let mut changed = Changed::default();
    let mut early = 0usize;
    let rates = FRAME_RATES.iter().copied().chain([(25, 2)]);
    for frame_rate in rates {
        for &canvas_rate in &CANVAS_RATES {
            for unbuffered in [false, true] {
                let starts = NEW_BASES
                    .iter()
                    .map(|&base| (Start::New { base }, PAUSE))
                    .chain([false, true].into_iter().flat_map(|reset| {
                        [150 * MS, PAUSE].map(|pause| (Start::Pause { reset }, pause))
                    }));
                for (start, pause) in starts {
                    let what = format!(
                        "{frame_rate:?} into {canvas_rate:?}, unbuffered {unbuffered}, {start:?}, pause {pause}"
                    );
                    let one = |variant| {
                        let (c, reached) = cadence_with(
                            &C,
                            frame_rate,
                            canvas_rate,
                            unbuffered,
                            start,
                            pause,
                            variant,
                        );
                        let (r, _) = cadence_with(
                            &Rust,
                            frame_rate,
                            canvas_rate,
                            unbuffered,
                            start,
                            pause,
                            variant,
                        );
                        assert_eq!(c.log, r.log, "{what}");
                        assert_eq!(c.states, r.states, "{what}");
                        (visible(&c.log, &c.states), reached)
                    };
                    compare(&mut changed, &what, one);

                    // the frames shown, in order, every one of them
                    let fixed =
                        cadence_with(&C, frame_rate, canvas_rate, unbuffered, start, pause, FIXED)
                            .0;
                    let before = cadence_with(
                        &C,
                        frame_rate,
                        canvas_rate,
                        unbuffered,
                        start,
                        pause,
                        UPSTREAM,
                    )
                    .0;
                    let order = |run: &Cadence| {
                        let picked: Vec<u64> = run
                            .picks
                            .iter()
                            .map(|&t| run.shown[t].expect("a frame").0)
                            .collect();
                        picked.windows(2).all(|w| w[0] < w[1])
                    };
                    assert!(order(&fixed), "{what}");
                    if fixed.shown != before.shown {
                        early += 1;
                        eprintln!(
                            "{what}: shown before #2009 {:?}\n  after {:?}",
                            before
                                .picks
                                .iter()
                                .map(|&t| before.shown[t])
                                .collect::<Vec<_>>(),
                            fixed
                                .picks
                                .iter()
                                .map(|&t| fixed.shown[t])
                                .collect::<Vec<_>>()
                        );
                    }
                }
            }
        }
    }
    eprintln!("cadences: {changed:?}, shown differently {early}");
}

/// The sequences of `sequences_reach_every_branch`, through the walk
/// before #2009 and after.
#[test]
fn buffered_walk_sequences() {
    let mut runner = proptest::test_runner::TestRunner::deterministic();
    let mut changed = Changed::default();
    for _ in 0..2000 {
        let seq = sequence()
            .new_tree(&mut runner)
            .expect("a sequence")
            .current();
        let what = format!("{seq:?}");
        compare(&mut changed, &what, |variant| {
            let (log, states, reached) = run_with(&C, &seq, variant);
            (visible(&log, &states), reached)
        });
    }
    eprintln!("sequences: {changed:?}");
}

/// The `behind` cases of `test_deinterlace_2x_field_order` through the
/// model: a buffered source that never showed a frame shows frame 0, gets
/// frames 1 to 3 at once right after a tick, stamped 4, 8 and 12 ms after
/// frame 0 (all overdue at the next tick), then right after that tick
/// frames 4 to 9 at once, at its rate after frame 3. Returns the frames
/// picked, in order, and what the run reached.
fn behind(
    frame_rate: (u64, u64),
    canvas_rate: (u64, u64),
    variant: (Walk, bool),
) -> (Vec<u64>, Reached) {
    let mut r = Run::new(&C, false, variant);
    let interval = 1_000_000_000 * canvas_rate.1 / canvas_rate.0;
    let t0 = CANVAS_BASE + MS;
    let stamp = |n: u64| {
        if n < 4 {
            t0 + n * 4 * MS
        } else {
            t0 + 12 * MS + rate_time(n - 3, frame_rate)
        }
    };
    let mut picked = Vec::new();
    for tick in 1u64..=60 {
        let frames = match tick {
            1 => 0..1,
            4 => 1..4,
            5 => 4..10,
            _ => 0..0,
        };
        for n in frames {
            r.output_video(stamp(n));
        }
        let video_time = CANVAS_BASE + tick * interval;
        r.tick(video_time, video_time, interval);
        if let Some(cur) = r.s.cur_async_frame {
            picked.push(
                (0..10)
                    .find(|&n| stamp(n) == cur.timestamp)
                    .expect("a frame"),
            );
        }
        r.render();
    }
    (picked, r.reached)
}

/// Before #2009 the walk's exit flags frame 1 (erased and in the cache),
/// frame 4 reuses its entry and is paired, so frame 5 is shown in its
/// place; after it, frame 1 is dropped (it is overdue) and 2 to 9 are shown
/// in order. Each half of the fix alone does the same. Flagging the front
/// frame at the exit instead would drop frame 2 too.
#[test]
fn behind_cadence() {
    let rates = [
        ((30, 1), (60, 1)),
        ((25, 1), (50, 1)),
        ((25, 1), (60, 1)),
        ((30, 1), (50, 1)),
    ];
    for (frame_rate, canvas_rate) in rates {
        let what = format!("{frame_rate:?} into {canvas_rate:?}");
        let (before, up) = behind(frame_rate, canvas_rate, UPSTREAM);
        let (after, fixed) = behind(frame_rate, canvas_rate, FIXED);
        eprintln!("{what}: picked before #2009 {before:?}, after {after:?}");
        assert_eq!(up.exit_after_erase, 1, "{what}");
        assert_eq!(up.stale_pairs, 1, "{what}");
        assert_eq!(fixed.stale_pairs, 0, "{what}");
        assert_eq!(before, [0, 2, 3, 5, 6, 7, 8, 9], "{what}");
        assert_eq!(after, [0, 2, 3, 4, 5, 6, 7, 8, 9], "{what}");
        assert_eq!(
            behind(frame_rate, canvas_rate, CLEAR_ONLY).0,
            after,
            "{what}"
        );
        assert_eq!(
            behind(frame_rate, canvas_rate, NO_FLAG_ONLY).0,
            after,
            "{what}"
        );
        // flagging the front frame instead would pair 2 with 3 at the
        // exit, dropping frame 2 as well
        let (front, _) = behind(frame_rate, canvas_rate, FLAG_FRONT);
        eprintln!("{what}: front flagged {front:?}");
        assert_eq!(front, [0, 3, 4, 5, 6, 7, 8, 9], "{what}");
    }
}
