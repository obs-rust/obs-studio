//! Tier 3 parity: the shims of `obs_output::ffi::output_raw_audio` (and the
//! safe core under them) against the original
//! `libobs/obs-output-raw-audio.c.h`, compiled as
//! `oracle_obs_output_raw_audio_*` (obs-c-oracle, oracle/output_raw_audio.c).
//!
//! Every output is compared exactly, on all three sides (oracle, shim,
//! core): timestamps, sizes and frame counts as integers, the decisions as
//! bools.
//!
//! Runs: the two constants; every two-value decision over every pair of 30
//! special `u64` values (0, 1, 2, around 2^32, 2^63 and `u64::MAX`, and
//! around a video start, one 48 kHz packet and one sample) and the drop
//! over every triple (27,000 calls); the sizes over every pair of 13
//! special `usize` values with 12 special frame counts and the 30 special
//! cut lengths; the passthrough over every low 20-bit flag word and a
//! stride through all `u32`; the mix rule over every low 16-bit mask under
//! four high words at every index below the width of `usize`; the delay
//! over 0 to 100,000 and a stride through all `u32`; the interleave rule
//! over all four inputs; the callers (`prepare_audio()`, the block loop of
//! `default_raw_audio_callback()`, `start_raw_audio()` and
//! `hook_data_capture()`, as the C test's stand-in output) on the oracle
//! and on the shims, over flags, video starts, pause data, rates, sizes,
//! plane counts and two-packet sequences around the video start (186,624
//! sequences), comparing every admission, cut, push, block and timestamp,
//! every mixer mask with and without `raw_audio2` and every flag and delay
//! combination of the hookup; and proptests (20,000 cases each) of every
//! decision over full-range values and of the callers over packet
//! sequences.
//!
//! Nothing in the oracle divides. The stand-in's conversions between
//! nanoseconds and frames are this file's own copy of `util_mul_div64()`'s
//! portable formula, shared by both sides, with a sample rate of at least 1;
//! on MSVC x64 the C's `_udiv128()` form traps on a zero rate, which stays
//! in `obs-output.c`. [`mul_div`] asserts the rate when the oracle side runs
//! on that target.
//!
//! Intentional difference: `mix_on` with an index at or past the width of
//! `usize`, where the C's shift is undefined, is not set in the port. The C
//! asks only 0 to 5. Tier 1 pins it; here the oracle is called only below
//! the width, which excludes exactly those inputs. Otherwise no input is
//! excluded. `test_output_raw_audio_steps` runs the C test against the C;
//! the real-libobs output tests run against the hybrid libobs, where these
//! shims replace the C.

use obs_c_oracle::output_raw_audio::{
    CONSTANT_NAMES, oracle_obs_output_raw_audio_before_video,
    oracle_obs_output_raw_audio_block_bytes, oracle_obs_output_raw_audio_block_ts,
    oracle_obs_output_raw_audio_can_pop, oracle_obs_output_raw_audio_cut_bytes,
    oracle_obs_output_raw_audio_cut_frames, oracle_obs_output_raw_audio_cut_ns,
    oracle_obs_output_raw_audio_cut_ts, oracle_obs_output_raw_audio_delay_ns,
    oracle_obs_output_raw_audio_delay_on, oracle_obs_output_raw_audio_ends_before,
    oracle_obs_output_raw_audio_interleave, oracle_obs_output_raw_audio_mix_on,
    oracle_obs_output_raw_audio_next_frames, oracle_obs_output_raw_audio_passthrough,
    oracle_obs_output_raw_audio_paused_ts, oracle_obs_output_raw_audio_push_bytes,
    oracle_obs_output_raw_audio_start_unset, oracle_obs_output_raw_audio_video_unset,
    oracle_output_raw_audio_constant,
};
use obs_output::ffi::output_raw_audio::{
    obs_output_raw_audio_before_video, obs_output_raw_audio_block_bytes,
    obs_output_raw_audio_block_ts, obs_output_raw_audio_can_pop, obs_output_raw_audio_cut_bytes,
    obs_output_raw_audio_cut_frames, obs_output_raw_audio_cut_ns, obs_output_raw_audio_cut_ts,
    obs_output_raw_audio_delay_ns, obs_output_raw_audio_delay_on, obs_output_raw_audio_ends_before,
    obs_output_raw_audio_interleave, obs_output_raw_audio_mix_on, obs_output_raw_audio_next_frames,
    obs_output_raw_audio_passthrough, obs_output_raw_audio_paused_ts,
    obs_output_raw_audio_push_bytes, obs_output_raw_audio_start_unset,
    obs_output_raw_audio_video_unset,
};
use obs_output::raw_audio::{self as core, AUDIO_OUTPUT_FRAMES, OBS_OUTPUT_VIDEO};
use obs_output::video_checks::first_mixer;
use proptest::prelude::*;

const NS: u64 = 1_000_000_000;
/// 1024 frames at 48 kHz.
const PKT: u64 = 21_333_333;
const V: u64 = 10_000_000_000;

/// Calls one decision on the oracle, the shim and the core, and checks all
/// three agree.
macro_rules! same {
    ($name:ident ($($arg:expr),*)) => {{
        let core_side = core::$name($($arg),*);
        paste_sides!($name, core_side, $($arg),*)
    }};
}

macro_rules! paste_sides {
    (passthrough, $c:expr, $($a:expr),*) => { three($c, obs_output_raw_audio_passthrough($($a),*), unsafe { oracle_obs_output_raw_audio_passthrough($($a),*) }) };
    (video_unset, $c:expr, $($a:expr),*) => { three($c, obs_output_raw_audio_video_unset($($a),*), unsafe { oracle_obs_output_raw_audio_video_unset($($a),*) }) };
    (before_video, $c:expr, $($a:expr),*) => { three($c, obs_output_raw_audio_before_video($($a),*), unsafe { oracle_obs_output_raw_audio_before_video($($a),*) }) };
    (ends_before, $c:expr, $($a:expr),*) => { three($c, obs_output_raw_audio_ends_before($($a),*), unsafe { oracle_obs_output_raw_audio_ends_before($($a),*) }) };
    (cut_ns, $c:expr, $($a:expr),*) => { three($c, obs_output_raw_audio_cut_ns($($a),*), unsafe { oracle_obs_output_raw_audio_cut_ns($($a),*) }) };
    (cut_ts, $c:expr, $($a:expr),*) => { three($c, obs_output_raw_audio_cut_ts($($a),*), unsafe { oracle_obs_output_raw_audio_cut_ts($($a),*) }) };
    (cut_bytes, $c:expr, $($a:expr),*) => { three($c, obs_output_raw_audio_cut_bytes($($a),*), unsafe { oracle_obs_output_raw_audio_cut_bytes($($a),*) }) };
    (cut_frames, $c:expr, $($a:expr),*) => { three($c, obs_output_raw_audio_cut_frames($($a),*), unsafe { oracle_obs_output_raw_audio_cut_frames($($a),*) }) };
    (start_unset, $c:expr, $($a:expr),*) => { three($c, obs_output_raw_audio_start_unset($($a),*), unsafe { oracle_obs_output_raw_audio_start_unset($($a),*) }) };
    (block_bytes, $c:expr, $($a:expr),*) => { three($c, obs_output_raw_audio_block_bytes($($a),*), unsafe { oracle_obs_output_raw_audio_block_bytes($($a),*) }) };
    (push_bytes, $c:expr, $($a:expr),*) => { three($c, obs_output_raw_audio_push_bytes($($a),*), unsafe { oracle_obs_output_raw_audio_push_bytes($($a),*) }) };
    (can_pop, $c:expr, $($a:expr),*) => { three($c, obs_output_raw_audio_can_pop($($a),*), unsafe { oracle_obs_output_raw_audio_can_pop($($a),*) }) };
    (block_ts, $c:expr, $($a:expr),*) => { three($c, obs_output_raw_audio_block_ts($($a),*), unsafe { oracle_obs_output_raw_audio_block_ts($($a),*) }) };
    (paused_ts, $c:expr, $($a:expr),*) => { three($c, obs_output_raw_audio_paused_ts($($a),*), unsafe { oracle_obs_output_raw_audio_paused_ts($($a),*) }) };
    (next_frames, $c:expr, $($a:expr),*) => { three($c, obs_output_raw_audio_next_frames($($a),*), unsafe { oracle_obs_output_raw_audio_next_frames($($a),*) }) };
    (mix_on, $c:expr, $($a:expr),*) => { three($c, obs_output_raw_audio_mix_on($($a),*), unsafe { oracle_obs_output_raw_audio_mix_on($($a),*) }) };
    (interleave, $c:expr, $($a:expr),*) => { three($c, obs_output_raw_audio_interleave($($a),*), unsafe { oracle_obs_output_raw_audio_interleave($($a),*) }) };
    (delay_on, $c:expr, $($a:expr),*) => { three($c, obs_output_raw_audio_delay_on($($a),*), unsafe { oracle_obs_output_raw_audio_delay_on($($a),*) }) };
    (delay_ns, $c:expr, $($a:expr),*) => { three($c, obs_output_raw_audio_delay_ns($($a),*), unsafe { oracle_obs_output_raw_audio_delay_ns($($a),*) }) };
}

#[track_caller]
fn three<T: PartialEq + std::fmt::Debug + Copy>(core_side: T, shim: T, oracle: T) -> T {
    assert_eq!(shim, oracle, "shim against the C");
    assert_eq!(core_side, oracle, "core against the C");
    oracle
}

fn special_u64() -> Vec<u64> {
    let mut v = vec![
        0,
        1,
        2,
        0x7fff_ffff_ffff_ffff,
        0x8000_0000_0000_0000,
        u64::MAX - 1,
        u64::MAX,
        u64::from(u32::MAX),
        u64::from(u32::MAX) + 1,
    ];
    for base in [NS, V, PKT, 20833, 20834] {
        for d in [0u64, 1, 2] {
            v.push(base.wrapping_add(d));
            v.push(base.wrapping_sub(d));
        }
    }
    v.sort_unstable();
    v.dedup();
    v
}

const SPECIAL_USIZE: [usize; 13] = [
    0,
    1,
    2,
    3,
    4,
    8,
    1024,
    4096,
    u32::MAX as usize,
    (u32::MAX as usize).wrapping_add(1),
    usize::MAX / 2,
    usize::MAX - 1,
    usize::MAX,
];

const SPECIAL_FRAMES: [u32; 12] = [
    0,
    1,
    2,
    480,
    1023,
    1024,
    1025,
    4096,
    0x7fff_ffff,
    0x8000_0000,
    u32::MAX - 1,
    u32::MAX,
];

#[test]
fn constants_match_c() {
    let ours = [u64::from(OBS_OUTPUT_VIDEO), u64::from(AUDIO_OUTPUT_FRAMES)];
    for (i, (name, want)) in CONSTANT_NAMES.iter().zip(ours).enumerate() {
        assert_eq!(
            unsafe { oracle_output_raw_audio_constant(i) },
            want,
            "{name}"
        );
    }
    assert_eq!(special_u64().len(), 30);
}

#[test]
fn decisions_over_special_timestamps() {
    let s = special_u64();
    for &a in &s {
        same!(video_unset(a));
        same!(start_unset(a));
        for &b in &s {
            same!(before_video(a, b));
            same!(cut_ns(a, b));
            same!(cut_ts(a, b));
            same!(block_ts(a, b));
            same!(paused_ts(a, b));
            for &c in &s {
                same!(ends_before(a, b, c));
            }
        }
    }
}

#[test]
fn sizes_over_special_values() {
    let s = special_u64();
    for size in SPECIAL_USIZE {
        same!(block_bytes(size));
        same!(next_frames(size));
        for other in SPECIAL_USIZE {
            same!(can_pop(size, other));
        }
        for f in SPECIAL_FRAMES {
            same!(push_bytes(f, size));
        }
        for &c in &s {
            same!(cut_bytes(size, c));
        }
    }
    for f in SPECIAL_FRAMES {
        for &c in &s {
            same!(cut_frames(f, c));
        }
    }
}

#[test]
fn flags_masks_and_delay() {
    for f in 0..(1u32 << 20) {
        same!(passthrough(f));
    }
    for f in (0..=u32::MAX).step_by(65_521).chain([u32::MAX]) {
        same!(passthrough(f));
    }
    for mask in 0..0x1_0000usize {
        for high in [0usize, 1 << 31, usize::MAX << 16, usize::MAX << 6] {
            let m = mask | high;
            for idx in 0..usize::BITS as usize {
                same!(mix_on(m, idx));
            }
        }
    }
    for d in (0..=100_000u32)
        .chain((0..=u32::MAX).step_by(65_521))
        .chain([u32::MAX])
    {
        same!(delay_on(d));
        same!(delay_ns(d));
    }
    for (v, a) in [(false, false), (false, true), (true, false), (true, true)] {
        same!(interleave(v, a));
    }
}

// ------------------------------------------------------------------------
// The callers, on a stand-in output, over each side's functions.

/// The decisions one side provides.
struct Side {
    passthrough: fn(u32) -> bool,
    video_unset: fn(u64) -> bool,
    before_video: fn(u64, u64) -> bool,
    ends_before: fn(u64, u64, u64) -> bool,
    cut_ns: fn(u64, u64) -> u64,
    cut_ts: fn(u64, u64) -> u64,
    cut_bytes: fn(usize, u64) -> usize,
    cut_frames: fn(u32, u64) -> u32,
    start_unset: fn(u64) -> bool,
    block_bytes: fn(usize) -> usize,
    push_bytes: fn(u32, usize) -> usize,
    can_pop: fn(usize, usize) -> bool,
    block_ts: fn(u64, u64) -> u64,
    paused_ts: fn(u64, u64) -> u64,
    next_frames: fn(usize) -> usize,
    mix_on: fn(usize, usize) -> bool,
    interleave: fn(bool, bool) -> bool,
    delay_on: fn(u32) -> bool,
    delay_ns: fn(u32) -> u64,
    oracle: bool,
}

const ORACLE: Side = Side {
    passthrough: |a| unsafe { oracle_obs_output_raw_audio_passthrough(a) },
    video_unset: |a| unsafe { oracle_obs_output_raw_audio_video_unset(a) },
    before_video: |a, b| unsafe { oracle_obs_output_raw_audio_before_video(a, b) },
    ends_before: |a, b, c| unsafe { oracle_obs_output_raw_audio_ends_before(a, b, c) },
    cut_ns: |a, b| unsafe { oracle_obs_output_raw_audio_cut_ns(a, b) },
    cut_ts: |a, b| unsafe { oracle_obs_output_raw_audio_cut_ts(a, b) },
    cut_bytes: |a, b| unsafe { oracle_obs_output_raw_audio_cut_bytes(a, b) },
    cut_frames: |a, b| unsafe { oracle_obs_output_raw_audio_cut_frames(a, b) },
    start_unset: |a| unsafe { oracle_obs_output_raw_audio_start_unset(a) },
    block_bytes: |a| unsafe { oracle_obs_output_raw_audio_block_bytes(a) },
    push_bytes: |a, b| unsafe { oracle_obs_output_raw_audio_push_bytes(a, b) },
    can_pop: |a, b| unsafe { oracle_obs_output_raw_audio_can_pop(a, b) },
    block_ts: |a, b| unsafe { oracle_obs_output_raw_audio_block_ts(a, b) },
    paused_ts: |a, b| unsafe { oracle_obs_output_raw_audio_paused_ts(a, b) },
    next_frames: |a| unsafe { oracle_obs_output_raw_audio_next_frames(a) },
    mix_on: |a, b| unsafe { oracle_obs_output_raw_audio_mix_on(a, b) },
    interleave: |a, b| unsafe { oracle_obs_output_raw_audio_interleave(a, b) },
    delay_on: |a| unsafe { oracle_obs_output_raw_audio_delay_on(a) },
    delay_ns: |a| unsafe { oracle_obs_output_raw_audio_delay_ns(a) },
    oracle: true,
};

const SHIMS: Side = Side {
    passthrough: |a| obs_output_raw_audio_passthrough(a),
    video_unset: |a| obs_output_raw_audio_video_unset(a),
    before_video: |a, b| obs_output_raw_audio_before_video(a, b),
    ends_before: |a, b, c| obs_output_raw_audio_ends_before(a, b, c),
    cut_ns: |a, b| obs_output_raw_audio_cut_ns(a, b),
    cut_ts: |a, b| obs_output_raw_audio_cut_ts(a, b),
    cut_bytes: |a, b| obs_output_raw_audio_cut_bytes(a, b),
    cut_frames: |a, b| obs_output_raw_audio_cut_frames(a, b),
    start_unset: |a| obs_output_raw_audio_start_unset(a),
    block_bytes: |a| obs_output_raw_audio_block_bytes(a),
    push_bytes: |a, b| obs_output_raw_audio_push_bytes(a, b),
    can_pop: |a, b| obs_output_raw_audio_can_pop(a, b),
    block_ts: |a, b| obs_output_raw_audio_block_ts(a, b),
    paused_ts: |a, b| obs_output_raw_audio_paused_ts(a, b),
    next_frames: |a| obs_output_raw_audio_next_frames(a),
    mix_on: |a, b| obs_output_raw_audio_mix_on(a, b),
    interleave: |a, b| obs_output_raw_audio_interleave(a, b),
    delay_on: |a| obs_output_raw_audio_delay_on(a),
    delay_ns: |a| obs_output_raw_audio_delay_ns(a),
    oracle: false,
};

/// `util_mul_div64()`'s portable formula. The rate is never 0 here; on
/// MSVC x64 the C's `_udiv128()` form would trap on it, so the oracle side
/// checks that before it runs on that target.
fn mul_div(side: &Side, num: u64, mul: u64, div: u64) -> u64 {
    if side.oracle && cfg!(all(target_env = "msvc", target_arch = "x86_64")) {
        assert_ne!(div, 0, "a zero rate traps on MSVC x64");
    }
    let rem = num % div;
    (num / div)
        .wrapping_mul(mul)
        .wrapping_add(rem.wrapping_mul(mul) / div)
}

const PLANES: usize = 8;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct StandIn {
    flags: u32,
    video_start_ts: u64,
    last_video_ts: u64,
    ts_offset: u64,
    audio_start_ts: u64,
    sample_rate: u64,
    audio_size: usize,
    planes: usize,
    total_audio_frames: usize,
    buffered: [usize; PLANES],
    /// Every admission, cut, push and block, in order.
    trace: Vec<(u8, u64, u64)>,
}

impl StandIn {
    /// `default_raw_audio_callback()` with `prepare_audio()`, with
    /// `data_active()` true and no pause.
    fn callback(&mut self, s: &Side, timestamp: u64, frames: u32) {
        let mut out_ts = timestamp;
        let mut out_frames = frames;
        if !(s.passthrough)(self.flags) {
            if (s.video_unset)(self.video_start_ts) {
                self.video_start_ts = self.last_video_ts;
            }
            if (s.video_unset)(self.video_start_ts) {
                self.trace.push((0, timestamp, 0));
                return;
            }
            if (s.before_video)(timestamp, self.video_start_ts) {
                let duration = mul_div(s, u64::from(frames), NS, self.sample_rate);
                if (s.ends_before)(timestamp, duration, self.video_start_ts) {
                    self.trace.push((1, timestamp, duration));
                    return;
                }
                let cutoff = (s.cut_ns)(self.video_start_ts, timestamp);
                out_ts = (s.cut_ts)(out_ts, cutoff);
                let cutoff = mul_div(s, cutoff, self.sample_rate, NS);
                for _ in 0..self.planes {
                    let b = (s.cut_bytes)(self.audio_size, cutoff);
                    self.trace.push((2, b as u64, cutoff));
                }
                out_frames = (s.cut_frames)(out_frames, cutoff);
            }
        }
        self.trace.push((3, out_ts, u64::from(out_frames)));
        if (s.start_unset)(self.audio_start_ts) {
            self.audio_start_ts = out_ts;
        }
        let frame_size_bytes = (s.block_bytes)(self.audio_size);
        for i in 0..self.planes {
            let b = (s.push_bytes)(out_frames, self.audio_size);
            self.buffered[i] = self.buffered[i].wrapping_add(b);
            self.trace.push((4, i as u64, b as u64));
        }
        while (s.can_pop)(self.buffered[0], frame_size_bytes) {
            for i in 0..self.planes {
                self.buffered[i] = self.buffered[i].wrapping_sub(frame_size_bytes);
            }
            let ns = mul_div(s, self.total_audio_frames as u64, NS, self.sample_rate);
            let ts = (s.block_ts)(self.audio_start_ts, ns);
            let ts = (s.paused_ts)(ts, self.ts_offset);
            self.total_audio_frames = (s.next_frames)(self.total_audio_frames);
            self.trace.push((5, ts, self.total_audio_frames as u64));
            assert!(self.trace.len() < 4096, "runaway block loop");
        }
    }
}

/// `start_raw_audio()`: the mixes connected.
fn mixers(s: &Side, mixer_mask: usize, raw_audio2: bool) -> Vec<usize> {
    if raw_audio2 {
        (0..6).filter(|&idx| (s.mix_on)(mixer_mask, idx)).collect()
    } else {
        vec![first_mixer(mixer_mask)]
    }
}

/// `hook_data_capture()`: (interleave, delay, delay_ns, audio, video).
fn hook(s: &Side, flags: u32, delay_sec: u32) -> (bool, bool, u64, bool, bool) {
    let has_video = flags & 1 != 0;
    let has_audio = flags & 2 != 0;
    let mut r = (false, false, 0, has_audio, has_video);
    if flags & 4 != 0 {
        r.0 = (s.interleave)(has_video, has_audio);
        if (s.delay_on)(delay_sec) {
            r.1 = true;
            r.2 = (s.delay_ns)(delay_sec);
        }
    }
    r
}

fn run_both(start: &StandIn, packets: &[(u64, u32)]) {
    let mut a = start.clone();
    let mut b = start.clone();
    for &(ts, frames) in packets {
        a.callback(&ORACLE, ts, frames);
        b.callback(&SHIMS, ts, frames);
    }
    assert_eq!(a, b, "{start:?} {packets:?}");
}

#[test]
fn callers_on_the_stand_in() {
    let offsets: [i64; 9] = [
        -2 * PKT as i64,
        -(PKT as i64),
        -(PKT as i64) + 1,
        -20834,
        -20833,
        -1,
        0,
        1,
        PKT as i64,
    ];
    let frames = [0u32, 1, 1023, 1024, 1025, 4096];
    let mut sequences = 0u64;
    for flags in [2u32, 1, 3, 7] {
        for video_start_ts in [0, V] {
            for last_video_ts in [0, V, V + 1] {
                for rate in [1u64, 44100, 48000, 384_000] {
                    for size in [1usize, 2, 4, 8] {
                        for planes in [1usize, 2, 8] {
                            let start = StandIn {
                                flags,
                                video_start_ts,
                                last_video_ts,
                                ts_offset: 7,
                                sample_rate: rate,
                                audio_size: size,
                                planes,
                                ..StandIn::default()
                            };
                            for off in offsets {
                                for f in frames {
                                    let ts = V.wrapping_add_signed(off);
                                    for next in [0u64, PKT, 3 * PKT] {
                                        run_both(&start, &[(ts, f), (ts.wrapping_add(next), 1024)]);
                                        sequences += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(sequences, 4 * 2 * 3 * 4 * 4 * 3 * 9 * 6 * 3);

    for mask in 0..0x1000usize {
        for raw2 in [false, true] {
            assert_eq!(mixers(&ORACLE, mask, raw2), mixers(&SHIMS, mask, raw2));
        }
    }
    for flags in 0..64u32 {
        for d in [0u32, 1, 20, 0x8000_0000, u32::MAX] {
            assert_eq!(hook(&ORACLE, flags, d), hook(&SHIMS, flags, d));
        }
    }
}

fn arb_start() -> impl Strategy<Value = StandIn> {
    (
        any::<u32>(),
        prop_oneof![Just(0u64), V - 3 * PKT..V + 3 * PKT, any::<u64>()],
        prop_oneof![Just(0u64), V - 3 * PKT..V + 3 * PKT, any::<u64>()],
        prop_oneof![Just(0u64), any::<u64>()],
        prop_oneof![Just(0u64), V - 3 * PKT..V + 3 * PKT, any::<u64>()],
        prop_oneof![Just(48000u64), 1..1_000_000u64, 1..=u64::MAX],
        prop_oneof![Just(4usize), 1..9usize, 1..4096usize],
        1..=PLANES,
        any::<usize>(),
    )
        .prop_map(
            |(flags, vs, lv, off, a0, rate, size, planes, total)| StandIn {
                flags,
                video_start_ts: vs,
                last_video_ts: lv,
                ts_offset: off,
                audio_start_ts: a0,
                sample_rate: rate,
                audio_size: size,
                planes,
                total_audio_frames: total,
                ..StandIn::default()
            },
        )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(20_000)))]

    #[test]
    fn proptest_decisions(
        a in any::<u64>(),
        b in any::<u64>(),
        c in any::<u64>(),
        f in any::<u32>(),
        s in any::<usize>(),
        t in any::<usize>(),
        i in 0..usize::BITS as usize,
        v in any::<bool>(),
        w in any::<bool>(),
    ) {
        same!(passthrough(f));
        same!(video_unset(a));
        same!(start_unset(a));
        same!(before_video(a, b));
        same!(ends_before(a, b, c));
        same!(cut_ns(a, b));
        same!(cut_ts(a, b));
        same!(cut_bytes(s, a));
        same!(cut_frames(f, a));
        same!(block_bytes(s));
        same!(push_bytes(f, s));
        same!(can_pop(s, t));
        same!(block_ts(a, b));
        same!(paused_ts(a, b));
        same!(next_frames(s));
        same!(mix_on(s, i));
        same!(interleave(v, w));
        same!(delay_on(f));
        same!(delay_ns(f));
    }

    #[test]
    fn proptest_callers(
        start in arb_start(),
        packets in prop::collection::vec(
            (
                prop_oneof![V - 3 * PKT..V + 3 * PKT, Just(V - PKT), any::<u64>()],
                prop_oneof![Just(1024u32), 0..4097u32],
            ),
            0..6,
        ),
    ) {
        run_both(&start, &packets);
    }
}
