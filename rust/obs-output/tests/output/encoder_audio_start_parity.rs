//! Tier 2 layout and Tier 3 parity: the shims of
//! `obs_output::ffi::encoder_audio_start` (and the safe core under them)
//! against the original `libobs/obs-encoder-audio-start.c.h`, compiled as
//! `oracle_obs_encoder_audio_start_*` (obs-c-oracle,
//! oracle/encoder_audio_start.c).
//!
//! Every output is compared exactly, on all three sides (oracle, shim,
//! core): the plan field by field, sizes, timestamps and pts as integers,
//! the decisions as bools. The core's `None` from `push_size` is the C's 0,
//! and its `PacketStart` the C's four flags.
//!
//! Runs: the constant and the layout of the plan struct; every two-value
//! decision over every pair of 34 special `u64` values (timestamps at 0, 1,
//! around a packet and a video start, around 2^63 and `u64::MAX`); the
//! packet plan over every triple of them (39,304 calls); the sizes over
//! every pair of 16 special `usize` values and 12 special frame counts;
//! both pts steps over the special values where the C is defined; the setter
//! type over every `int` from -65,536 to 65,536 and the extremes, the
//! divisor over 0 to 65,536 and the edges; the callers (`buffer_audio()`,
//! `start_from_buffer()`, `calc_offset_size()`, `push_back_audio()` and
//! the send loop, as the C test's stand-in encoder) over every combination
//! of eight flags and special timestamps around the video's start, at
//! blocksizes 1, 2, 4 and 8 and rates 1 Hz to 384 kHz, comparing every
//! push, clear, start and pts; and proptests (20,000 cases each) of every
//! decision over full-range values and of the callers over packet
//! sequences.
//!
//! Nothing here divides: both `util_mul_div64()` conversions stay in
//! `obs-encoder.c`, and the stand-in uses this file's own copy of the
//! portable formula with a sample rate of at least 1 (the MSVC x64
//! `_udiv128()` form traps on a zero rate or a quotient past 64 bits). No
//! input is `INT64_MIN / -1`.
//!
//! Intentional difference: the video pts step past `i64::MAX` (or below
//! `i64::MIN`), where the C's `int64_t` sum is undefined, wraps in the port
//! (`wrapping_add`). A pts that large is never reached. Tier 1 covers the
//! wrap; here the C is called only where `cur_pts + timebase_num *
//! frame_rate_divisor` stays within `i64`, which excludes exactly those
//! inputs. Otherwise no input is excluded.
//! `test_encoder_audio_start_steps` runs the C test against the C; the
//! real-libobs output and encoder tests run against the hybrid libobs,
//! where these shims replace the C.

use std::ffi::c_int;
use std::mem::{align_of, offset_of, size_of};

use obs_c_oracle::encoder_audio_start::{
    CONSTANT_NAMES, LAYOUT_NAMES, Plan as OraclePlan, oracle_encoder_audio_start_constant,
    oracle_encoder_audio_start_layout, oracle_obs_encoder_audio_start_audio_pts,
    oracle_obs_encoder_audio_start_buffer_offset, oracle_obs_encoder_audio_start_can_send,
    oracle_obs_encoder_audio_start_divisor_ok, oracle_obs_encoder_audio_start_group_ready,
    oracle_obs_encoder_audio_start_no_video, oracle_obs_encoder_audio_start_not_video,
    oracle_obs_encoder_audio_start_offset_ns, oracle_obs_encoder_audio_start_offset_size,
    oracle_obs_encoder_audio_start_packet, oracle_obs_encoder_audio_start_paired,
    oracle_obs_encoder_audio_start_paired_late, oracle_obs_encoder_audio_start_push_size,
    oracle_obs_encoder_audio_start_size, oracle_obs_encoder_audio_start_unpaired,
    oracle_obs_encoder_audio_start_video_pts,
};
use obs_output::encoder_audio_start::{self as core, OBS_ENCODER_VIDEO, PacketStart};
use obs_output::ffi::encoder_audio_start::{
    obs_encoder_audio_start_audio_pts, obs_encoder_audio_start_buffer_offset,
    obs_encoder_audio_start_can_send, obs_encoder_audio_start_divisor_ok,
    obs_encoder_audio_start_group_ready, obs_encoder_audio_start_no_video,
    obs_encoder_audio_start_not_video, obs_encoder_audio_start_offset_ns,
    obs_encoder_audio_start_offset_size, obs_encoder_audio_start_packet,
    obs_encoder_audio_start_paired, obs_encoder_audio_start_paired_late,
    obs_encoder_audio_start_plan, obs_encoder_audio_start_push_size, obs_encoder_audio_start_size,
    obs_encoder_audio_start_unpaired, obs_encoder_audio_start_video_pts,
};
use proptest::prelude::*;

const NS: u64 = 1_000_000_000;
/// 1024 frames at 48 kHz.
const PKT: u64 = 21_333_333;
const V: u64 = 10_000_000_000;

/// Special `u64` values: the edges, and timestamps around a packet and a
/// video start.
const U64S: [u64; 34] = [
    0,
    1,
    2,
    3,
    20_833,
    20_834,
    PKT - 1,
    PKT,
    PKT + 1,
    2 * PKT,
    NS,
    V - 2 * PKT,
    V - PKT - 1,
    V - PKT,
    V - PKT + 1,
    V - 20_834,
    V - 1,
    V,
    V + 1,
    V + 20_834,
    V + PKT,
    V + PKT + 1,
    0x7fff_ffff,
    0xffff_ffff,
    0x1_0000_0000,
    0x7fff_ffff_ffff_fffe,
    0x7fff_ffff_ffff_ffff,
    0x8000_0000_0000_0000,
    0x8000_0000_0000_0001,
    u64::MAX - 2 * PKT,
    u64::MAX - PKT,
    u64::MAX - 2,
    u64::MAX - 1,
    u64::MAX,
];

const USIZES: [usize; 16] = [
    0,
    1,
    2,
    3,
    4,
    7,
    8,
    4095,
    4096,
    4097,
    8192,
    0x7fff_ffff,
    0xffff_ffff,
    usize::MAX / 2,
    usize::MAX - 1,
    usize::MAX,
];

const FRAMES: [u32; 12] = [
    0,
    1,
    2,
    480,
    960,
    1023,
    1024,
    2048,
    0x7fff_ffff,
    0x8000_0000,
    u32::MAX - 1,
    u32::MAX,
];

/// The sixteen decisions, one implementation per side.
trait Side {
    fn group_ready(a: u64, b: u64) -> bool;
    fn paired_late(a: u64, b: u64) -> bool;
    fn video_pts(pts: i64, tb: u32, div: u32) -> i64;
    fn size(frames: u32, blocksize: usize) -> usize;
    fn paired(start: u64, has: bool) -> bool;
    fn unpaired(start: u64, has: bool) -> bool;
    fn no_video(v: u64) -> bool;
    /// wait, truncate, clear, from_buffer
    fn packet(ts: u64, dur: u64, v: u64) -> [bool; 4];
    fn offset_ns(v: u64, a: u64) -> u64;
    fn offset_size(frames: u64, blocksize: usize) -> usize;
    fn buffer_offset(a: u64, v: u64) -> bool;
    fn push_size(size: usize, offset: usize) -> usize;
    fn can_send(buffered: usize, frame: usize) -> bool;
    fn audio_pts(pts: i64, framesize: usize) -> i64;
    fn not_video(t: c_int) -> bool;
    fn divisor_ok(d: u32) -> bool;
}

struct Oracle;
struct Shim;
struct Core;

impl Side for Oracle {
    fn group_ready(a: u64, b: u64) -> bool {
        // SAFETY: plain values in and out.
        unsafe { oracle_obs_encoder_audio_start_group_ready(a, b) }
    }
    fn paired_late(a: u64, b: u64) -> bool {
        // SAFETY: plain values in and out.
        unsafe { oracle_obs_encoder_audio_start_paired_late(a, b) }
    }
    fn video_pts(pts: i64, tb: u32, div: u32) -> i64 {
        assert!(
            pts.checked_add(i64::from(tb.wrapping_mul(div))).is_some(),
            "the C's int64_t sum is undefined here"
        );
        // SAFETY: plain values in and out; the sum stays within i64.
        unsafe { oracle_obs_encoder_audio_start_video_pts(pts, tb, div) }
    }
    fn size(frames: u32, blocksize: usize) -> usize {
        // SAFETY: plain values in and out.
        unsafe { oracle_obs_encoder_audio_start_size(frames, blocksize) }
    }
    fn paired(start: u64, has: bool) -> bool {
        // SAFETY: plain values in and out.
        unsafe { oracle_obs_encoder_audio_start_paired(start, has) }
    }
    fn unpaired(start: u64, has: bool) -> bool {
        // SAFETY: plain values in and out.
        unsafe { oracle_obs_encoder_audio_start_unpaired(start, has) }
    }
    fn no_video(v: u64) -> bool {
        // SAFETY: plain values in and out.
        unsafe { oracle_obs_encoder_audio_start_no_video(v) }
    }
    fn packet(ts: u64, dur: u64, v: u64) -> [bool; 4] {
        // SAFETY: plain values in, a struct by value out.
        let p: OraclePlan = unsafe { oracle_obs_encoder_audio_start_packet(ts, dur, v) };
        [p.wait, p.truncate, p.clear, p.from_buffer]
    }
    fn offset_ns(v: u64, a: u64) -> u64 {
        // SAFETY: plain values in and out.
        unsafe { oracle_obs_encoder_audio_start_offset_ns(v, a) }
    }
    fn offset_size(frames: u64, blocksize: usize) -> usize {
        // SAFETY: plain values in and out.
        unsafe { oracle_obs_encoder_audio_start_offset_size(frames, blocksize) }
    }
    fn buffer_offset(a: u64, v: u64) -> bool {
        // SAFETY: plain values in and out.
        unsafe { oracle_obs_encoder_audio_start_buffer_offset(a, v) }
    }
    fn push_size(size: usize, offset: usize) -> usize {
        // SAFETY: plain values in and out.
        unsafe { oracle_obs_encoder_audio_start_push_size(size, offset) }
    }
    fn can_send(buffered: usize, frame: usize) -> bool {
        // SAFETY: plain values in and out.
        unsafe { oracle_obs_encoder_audio_start_can_send(buffered, frame) }
    }
    fn audio_pts(pts: i64, framesize: usize) -> i64 {
        // SAFETY: plain values in and out (a size_t sum, defined for any
        // input).
        unsafe { oracle_obs_encoder_audio_start_audio_pts(pts, framesize) }
    }
    fn not_video(t: c_int) -> bool {
        // SAFETY: plain values in and out.
        unsafe { oracle_obs_encoder_audio_start_not_video(t) }
    }
    fn divisor_ok(d: u32) -> bool {
        // SAFETY: plain values in and out.
        unsafe { oracle_obs_encoder_audio_start_divisor_ok(d) }
    }
}

impl Side for Shim {
    fn group_ready(a: u64, b: u64) -> bool {
        obs_encoder_audio_start_group_ready(a, b)
    }
    fn paired_late(a: u64, b: u64) -> bool {
        obs_encoder_audio_start_paired_late(a, b)
    }
    fn video_pts(pts: i64, tb: u32, div: u32) -> i64 {
        obs_encoder_audio_start_video_pts(pts, tb, div)
    }
    fn size(frames: u32, blocksize: usize) -> usize {
        obs_encoder_audio_start_size(frames, blocksize)
    }
    fn paired(start: u64, has: bool) -> bool {
        obs_encoder_audio_start_paired(start, has)
    }
    fn unpaired(start: u64, has: bool) -> bool {
        obs_encoder_audio_start_unpaired(start, has)
    }
    fn no_video(v: u64) -> bool {
        obs_encoder_audio_start_no_video(v)
    }
    fn packet(ts: u64, dur: u64, v: u64) -> [bool; 4] {
        let p = obs_encoder_audio_start_packet(ts, dur, v);
        [p.wait, p.truncate, p.clear, p.from_buffer]
    }
    fn offset_ns(v: u64, a: u64) -> u64 {
        obs_encoder_audio_start_offset_ns(v, a)
    }
    fn offset_size(frames: u64, blocksize: usize) -> usize {
        obs_encoder_audio_start_offset_size(frames, blocksize)
    }
    fn buffer_offset(a: u64, v: u64) -> bool {
        obs_encoder_audio_start_buffer_offset(a, v)
    }
    fn push_size(size: usize, offset: usize) -> usize {
        obs_encoder_audio_start_push_size(size, offset)
    }
    fn can_send(buffered: usize, frame: usize) -> bool {
        obs_encoder_audio_start_can_send(buffered, frame)
    }
    fn audio_pts(pts: i64, framesize: usize) -> i64 {
        obs_encoder_audio_start_audio_pts(pts, framesize)
    }
    fn not_video(t: c_int) -> bool {
        obs_encoder_audio_start_not_video(t)
    }
    fn divisor_ok(d: u32) -> bool {
        obs_encoder_audio_start_divisor_ok(d)
    }
}

impl Side for Core {
    fn group_ready(a: u64, b: u64) -> bool {
        core::group_ready(a, b)
    }
    fn paired_late(a: u64, b: u64) -> bool {
        core::paired_late(a, b)
    }
    fn video_pts(pts: i64, tb: u32, div: u32) -> i64 {
        core::video_pts(pts, tb, div)
    }
    fn size(frames: u32, blocksize: usize) -> usize {
        core::packet_size(frames, blocksize)
    }
    fn paired(start: u64, has: bool) -> bool {
        core::paired_start(start, has)
    }
    fn unpaired(start: u64, has: bool) -> bool {
        core::unpaired_start(start, has)
    }
    fn no_video(v: u64) -> bool {
        core::no_video(v)
    }
    fn packet(ts: u64, dur: u64, v: u64) -> [bool; 4] {
        match core::packet_start(ts, dur, v) {
            PacketStart::Wait => [true, false, false, false],
            PacketStart::Start {
                truncate,
                clear,
                from_buffer,
            } => [false, truncate, clear, from_buffer],
        }
    }
    fn offset_ns(v: u64, a: u64) -> u64 {
        core::offset_ns(v, a)
    }
    fn offset_size(frames: u64, blocksize: usize) -> usize {
        core::offset_size(frames, blocksize)
    }
    fn buffer_offset(a: u64, v: u64) -> bool {
        core::buffer_offset(a, v)
    }
    fn push_size(size: usize, offset: usize) -> usize {
        match core::push_size(size, offset) {
            Some(0) => panic!("push_size gave Some(0)"),
            Some(n) => n,
            None => 0,
        }
    }
    fn can_send(buffered: usize, frame: usize) -> bool {
        core::can_send(buffered, frame)
    }
    fn audio_pts(pts: i64, framesize: usize) -> i64 {
        core::audio_pts(pts, framesize)
    }
    fn not_video(t: c_int) -> bool {
        core::not_video(t)
    }
    fn divisor_ok(d: u32) -> bool {
        core::divisor_ok(d)
    }
}

/// Calls `f` on all three sides and checks they agree.
macro_rules! same {
    ($f:ident ( $($a:expr),* )) => {{
        let o = Oracle::$f($($a),*);
        let s = Shim::$f($($a),*);
        let c = Core::$f($($a),*);
        assert_eq!(o, s, "{}({:?}): oracle vs shim", stringify!($f), ($($a),*));
        assert_eq!(o, c, "{}({:?}): oracle vs core", stringify!($f), ($($a),*));
        o
    }};
}

/// Whether the C's `cur_pts + timebase_num * frame_rate_divisor` is
/// defined.
fn video_pts_defined(pts: i64, tb: u32, div: u32) -> bool {
    pts.checked_add(i64::from(tb.wrapping_mul(div))).is_some()
}

const PTS: [i64; 12] = [
    0,
    1,
    -1,
    1001,
    -1024,
    0x7fff_ffff,
    -0x8000_0000,
    i64::MAX - 0xffff_ffff,
    i64::MAX - 1,
    i64::MAX,
    i64::MIN + 1,
    i64::MIN,
];
const U32S: [u32; 9] = [
    0,
    1,
    2,
    1001,
    0xffff,
    0x1_0000,
    0x1_0001,
    u32::MAX - 1,
    u32::MAX,
];

#[test]
fn layout_matches_c() {
    let ours = [
        size_of::<obs_encoder_audio_start_plan>(),
        align_of::<obs_encoder_audio_start_plan>(),
        offset_of!(obs_encoder_audio_start_plan, wait),
        offset_of!(obs_encoder_audio_start_plan, truncate),
        offset_of!(obs_encoder_audio_start_plan, clear),
        offset_of!(obs_encoder_audio_start_plan, from_buffer),
    ];
    let oracle_side = [
        size_of::<OraclePlan>(),
        align_of::<OraclePlan>(),
        offset_of!(OraclePlan, wait),
        offset_of!(OraclePlan, truncate),
        offset_of!(OraclePlan, clear),
        offset_of!(OraclePlan, from_buffer),
    ];
    for (i, name) in LAYOUT_NAMES.iter().enumerate() {
        // SAFETY: an index into a constant table.
        let c = unsafe { oracle_encoder_audio_start_layout(i) };
        assert_eq!(ours[i], c, "{name}: shim struct");
        assert_eq!(oracle_side[i], c, "{name}: oracle binding");
    }
    // SAFETY: an index past the table.
    assert_eq!(
        unsafe { oracle_encoder_audio_start_layout(LAYOUT_NAMES.len()) },
        usize::MAX
    );
}

#[test]
fn constants_match_c() {
    let ours = [i64::from(OBS_ENCODER_VIDEO)];
    for (i, name) in CONSTANT_NAMES.iter().enumerate() {
        // SAFETY: an index into a constant table.
        assert_eq!(
            unsafe { oracle_encoder_audio_start_constant(i) },
            ours[i],
            "{name}"
        );
    }
    // SAFETY: an index past the table.
    assert_eq!(
        unsafe { oracle_encoder_audio_start_constant(CONSTANT_NAMES.len()) },
        -1
    );
}

#[test]
fn two_value_decisions_exhaustive_match() {
    for &a in &U64S {
        for &b in &U64S {
            same!(group_ready(a, b));
            same!(paired_late(a, b));
            same!(offset_ns(a, b));
            same!(buffer_offset(a, b));
        }
        for has in [false, true] {
            same!(paired(a, has));
            same!(unpaired(a, has));
        }
        same!(no_video(a));
    }
}

#[test]
fn packet_exhaustive_match() {
    for &ts in &U64S {
        for &dur in &U64S {
            for &v in &U64S {
                same!(packet(ts, dur, v));
            }
        }
    }
}

#[test]
fn sizes_exhaustive_match() {
    for &a in &USIZES {
        for &b in &USIZES {
            same!(push_size(a, b));
            same!(can_send(a, b));
        }
        for &f in &FRAMES {
            same!(size(f, a));
        }
        for &o in &U64S {
            same!(offset_size(o, a));
        }
    }
}

#[test]
fn pts_exhaustive_match() {
    let mut called = 0;
    for &pts in &PTS {
        for &tb in &U32S {
            for &div in &U32S {
                if video_pts_defined(pts, tb, div) {
                    same!(video_pts(pts, tb, div));
                    called += 1;
                }
            }
        }
        for &f in &USIZES {
            same!(audio_pts(pts, f));
        }
    }
    assert!(called > 800, "only {called} defined video pts inputs");
}

#[test]
fn setters_exhaustive_match() {
    for t in (-65_536..=65_536).chain([c_int::MIN, c_int::MIN + 1, c_int::MAX - 1, c_int::MAX]) {
        same!(not_video(t));
    }
    for d in (0..=65_536).chain([0x7fff_ffff, 0x8000_0000, u32::MAX - 1, u32::MAX]) {
        same!(divisor_ok(d));
    }
}

// ---------------------------------------------------------------------------
// The callers, on a stand-in encoder (as test_encoder_audio_start_steps.c's)

/// `util_mul_div64()`'s portable formula (Linux, macOS, MSVC arm64). The
/// rate is at least 1 here, so it never divides by 0.
fn util_mul_div64(num: u64, mul: u64, div: u64) -> u64 {
    let rem = num % div;
    (num / div)
        .wrapping_mul(mul)
        .wrapping_add(rem.wrapping_mul(mul) / div)
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct StandIn {
    samplerate: u32,
    blocksize: usize,
    framesize: usize,
    start_ts: u64,
    first_raw_ts: u64,
    has_paired: bool,
    v_start_ts: u64,
    buffered: usize,
    cur_pts: i64,
    /// Every push (size, offset), clear, start from the buffer and send.
    trace: Vec<(char, u64, u64)>,
}

impl StandIn {
    fn push_back<S: Side>(&mut self, size: usize, offset: usize) {
        let size = S::push_size(size, offset);
        if size == 0 {
            return;
        }
        self.buffered = self.buffered.wrapping_add(size);
        self.trace.push(('p', size as u64, offset as u64));
    }

    fn calc_offset_size<S: Side>(&self, v: u64, a: u64) -> usize {
        let offset = S::offset_ns(v, a);
        let offset = util_mul_div64(offset, u64::from(self.samplerate), NS);
        S::offset_size(offset, self.blocksize)
    }

    fn start_from_buffer<S: Side>(&mut self, v: u64) {
        let size = self.buffered;
        let mut offset = 0;
        self.buffered = 0;
        self.trace.push(('b', size as u64, v));
        if S::buffer_offset(self.first_raw_ts, v) {
            offset = self.calc_offset_size::<S>(v, self.first_raw_ts);
        }
        self.push_back::<S>(size, offset);
    }

    fn buffer_audio<S: Side>(&mut self, ts: u64, frames: u32) -> bool {
        let size = S::size(frames, self.blocksize);
        let mut offset = 0;
        let mut success = true;
        if S::paired(self.start_ts, self.has_paired) {
            let v = self.v_start_ts;
            if S::no_video(v) {
                success = false;
            } else {
                let dur = util_mul_div64(u64::from(frames), NS, u64::from(self.samplerate));
                let [wait, truncate, clear, from_buffer] = S::packet(ts, dur, v);
                if wait {
                    success = false;
                } else {
                    if truncate {
                        offset = self.calc_offset_size::<S>(v, ts);
                    }
                    if clear {
                        self.buffered = 0;
                        self.trace.push(('c', 0, 0));
                    }
                    self.start_ts = v;
                    if from_buffer {
                        self.start_from_buffer::<S>(v);
                    }
                }
            }
        } else if S::unpaired(self.start_ts, self.has_paired) {
            self.start_ts = ts;
        }
        self.push_back::<S>(size, offset);
        success
    }

    /// `receive_audio()` after the first-packet reset: buffer, then send
    /// whole frames.
    fn receive_audio<S: Side>(&mut self, ts: u64, frames: u32) {
        if !self.buffer_audio::<S>(ts, frames) {
            return;
        }
        let frame_bytes = self.framesize.wrapping_mul(self.blocksize);
        let mut sends = 0;
        while S::can_send(self.buffered, frame_bytes) && sends < 64 {
            self.buffered = self.buffered.saturating_sub(frame_bytes);
            self.cur_pts = S::audio_pts(self.cur_pts, self.framesize);
            self.trace.push(('s', self.cur_pts as u64, 0));
            sends += 1;
        }
    }
}

fn run_sequence<S: Side>(start: &StandIn, packets: &[(u64, u32)]) -> StandIn {
    let mut e = start.clone();
    for &(ts, frames) in packets {
        e.receive_audio::<S>(ts, frames);
    }
    e
}

fn check_sequence(start: &StandIn, packets: &[(u64, u32)]) {
    let o = run_sequence::<Oracle>(start, packets);
    let s = run_sequence::<Shim>(start, packets);
    let c = run_sequence::<Core>(start, packets);
    assert_eq!(o, s, "{start:?} {packets:?}: oracle vs shim");
    assert_eq!(o, c, "{start:?} {packets:?}: oracle vs core");
}

const RATES: [u32; 6] = [48_000, 44_100, 8_000, 384_000, 1, 7];
const BLOCKSIZES: [usize; 4] = [1, 2, 4, 8];

#[test]
fn callers_exhaustive_match() {
    let offsets: [u64; 7] = [0, 1, 20_834, PKT - 1, PKT, PKT + 1, 3 * PKT];
    let mut runs = 0u64;
    for flags in 0u32..256 {
        let paired = flags & 1 != 0;
        let started = flags & 2 != 0;
        let video = flags & 4 != 0;
        let before = flags & 8 != 0;
        let frames = if flags & 16 != 0 { 1024 } else { 960 };
        let short_frame = flags & 32 != 0;
        let preloaded = flags & 64 != 0;
        let second_after = flags & 128 != 0;
        for &rate in &RATES {
            for &bs in &BLOCKSIZES {
                for &off in &offsets {
                    let first = if before { V - off } else { V + off };
                    let e = StandIn {
                        samplerate: rate,
                        blocksize: bs,
                        framesize: if short_frame {
                            0
                        } else {
                            usize::try_from(frames).unwrap()
                        },
                        start_ts: if started { V - 7 } else { 0 },
                        first_raw_ts: first,
                        has_paired: paired,
                        v_start_ts: if video { V } else { 0 },
                        buffered: if preloaded { 3 * bs } else { 0 },
                        cur_pts: 0,
                        trace: Vec::new(),
                    };
                    let next = if second_after { first + PKT } else { first + 1 };
                    check_sequence(&e, &[(first, frames), (next, frames), (V + PKT, 1)]);
                    runs += 1;
                }
            }
        }
    }
    assert_eq!(runs, 256 * 6 * 4 * 7);
}

// ---------------------------------------------------------------------------
// Proptests

fn timestamp() -> impl Strategy<Value = u64> {
    prop_oneof![
        any::<u64>(),
        (V - 4 * PKT..V + 4 * PKT),
        Just(0u64),
        Just(u64::MAX),
        (u64::MAX - 4 * PKT..=u64::MAX),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20_000))]

    #[test]
    fn proptest_decisions_match(
        a in timestamp(),
        b in timestamp(),
        d in timestamp(),
        frames in any::<u32>(),
        blocksize in any::<usize>(),
        size in any::<usize>(),
        offset in any::<usize>(),
        pts in any::<i64>(),
        tb in any::<u32>(),
        div in any::<u32>(),
        t in any::<c_int>(),
        has in any::<bool>(),
    ) {
        same!(group_ready(a, b));
        same!(paired_late(a, b));
        same!(size(frames, blocksize));
        same!(paired(a, has));
        same!(unpaired(a, has));
        same!(no_video(a));
        same!(packet(a, d, b));
        same!(offset_ns(a, b));
        same!(offset_size(d, blocksize));
        same!(buffer_offset(a, b));
        same!(push_size(size, offset));
        same!(can_send(size, offset));
        same!(audio_pts(pts, size));
        same!(not_video(t));
        same!(divisor_ok(div));
        let pts = if video_pts_defined(pts, tb, div) { pts } else { pts / 2 };
        if video_pts_defined(pts, tb, div) {
            same!(video_pts(pts, tb, div));
        }
    }

    #[test]
    fn proptest_callers_match(
        rate in prop_oneof![Just(48_000u32), Just(44_100), 1u32..400_000],
        blocksize in prop_oneof![Just(4usize), Just(8), 0usize..64],
        framesize in prop_oneof![Just(1024usize), Just(960), 0usize..5000],
        started in any::<bool>(),
        paired in any::<bool>(),
        v_start in prop_oneof![Just(0u64), V - 3 * PKT..V + 3 * PKT],
        first in V - 4 * PKT..V + 2 * PKT,
        preload in 0usize..20_000,
        packets in proptest::collection::vec((0u32..4096, 0u64..2 * PKT), 1..8),
    ) {
        let e = StandIn {
            samplerate: rate,
            blocksize,
            framesize,
            start_ts: if started { first } else { 0 },
            first_raw_ts: first,
            has_paired: paired,
            v_start_ts: v_start,
            buffered: preload,
            cur_pts: 0,
            trace: Vec::new(),
        };
        let mut ts = first;
        let seq: Vec<(u64, u32)> = packets
            .iter()
            .map(|&(frames, gap)| {
                let p = (ts, frames);
                ts = ts.wrapping_add(gap);
                p
            })
            .collect();
        check_sequence(&e, &seq);
    }
}
