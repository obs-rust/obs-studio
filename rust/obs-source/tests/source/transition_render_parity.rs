//! Tier 3: the `obs_source_transition_render_*` shims against the original
//! C (`obs-c-oracle`, `oracle/source_transition_render.c`: the unmodified
//! `libobs/obs-source-transition-render.c.h` with its functions renamed).
//!
//! Every function takes plain values; the place writes its frame through a
//! `size_t *`, which both sides get pre-filled with the same guard value, so
//! a run compares the return value and the frame, written or not. Floats
//! are compared by their bits.
//!
//! Runs:
//! - the constants, the enum values and sizes and the offset struct's
//!   layout against the C header;
//! - exhaustive over the flags: the video stop step, the draw choice, the
//!   mix and the validity check over every combination; the direct index
//!   over both states and every target from -70000 to 70000, every 65521st
//!   `i32` and the ends; the due test and the audio stop step over every
//!   flag at 40 special values of `t`, and the three stop steps as the
//!   render functions apply them;
//! - the due test and the audio stop step over every 4099th `f32` bit
//!   pattern and every pattern within 2^16 of 1.0 (every `f32` in an
//!   ignored release run);
//! - the place over every combination of 26 special timestamps for `ts`
//!   and `min_ts` and 14 sample rates (up to `usize::MAX`, where
//!   `util_mul_div64()` wraps), the first ns of every frame up to 1100 and
//!   the ns before it at eight rates;
//! - the offsets over every combination of 14 special values for a size, a
//!   child size and a scale on each side;
//! - proptests (20,000 cases each) over full-range `u64` timestamps and
//!   `usize` rates (half near min_ts), `f32` bit patterns for `t`, sizes and
//!   scales (never a wide `Range<f32>`, #1951), and any `i32` target.
//!
//! One exception, in the offsets only: where one float operation has two
//! NaN operands (a NaN child size and a NaN scale, or a NaN transition size
//! and a NaN product), which NaN comes out depends on the operand order the
//! compiler picks, as in #2021, so there both sides must give a NaN. A NaN
//! from one NaN operand, and every other result, is compared bit for bit.
//!
//! Intentional difference, checked on the shim only: a null `pos` is not
//! written (the C writes through it).
//!
//! Not compared on MSVC x64: a start frame that does not fit in 64 bits.
//! There `util_mul_div64()` is `_umul128()` and `_udiv128()`, whose
//! division faults when the quotient does not fit (the process dies with
//! `STATUS_INTEGER_OVERFLOW`), and the shim saturates instead (`LATE` at
//! `usize::MAX`), as `obs-scene`'s and `obs-core`'s ports do. It takes a
//! sample rate past 1e9, which libobs never sets, so it is unreachable. For
//! those inputs the run checks the shim's saturation and does not call the
//! C; everywhere else, and for every input off MSVC x64 (where the C's
//! `uint64_t` formula wraps, as the shim's does), the two are compared.

use std::ffi::c_int;

use obs_c_oracle::source_transition_render::{self as oracle, LAYOUT_NAMES, OracleOffset};
use obs_source::ffi::source_transition_render::{
    self as shim, OBS_SOURCE_TRANSITION_RENDER_AUDIO_END, OBS_SOURCE_TRANSITION_RENDER_AUDIO_KEEP,
    OBS_SOURCE_TRANSITION_RENDER_AUDIO_STOP, OBS_SOURCE_TRANSITION_RENDER_COPY_A,
    OBS_SOURCE_TRANSITION_RENDER_DRAW_A, OBS_SOURCE_TRANSITION_RENDER_DRAW_B,
    OBS_SOURCE_TRANSITION_RENDER_DRAW_CALLBACK, OBS_SOURCE_TRANSITION_RENDER_MIX_A,
    OBS_SOURCE_TRANSITION_RENDER_MIX_B, OBS_SOURCE_TRANSITION_RENDER_PLACE_LATE,
    OBS_SOURCE_TRANSITION_RENDER_PLACE_MIX, OBS_SOURCE_TRANSITION_RENDER_PLACE_NONE,
    OBS_SOURCE_TRANSITION_RENDER_VIDEO_END, OBS_SOURCE_TRANSITION_RENDER_VIDEO_STOP,
    OBS_SOURCE_TRANSITION_RENDER_VIDEO_STOP_AUDIO, obs_source_transition_render_audio_place,
    obs_source_transition_render_audio_step, obs_source_transition_render_draw_step,
    obs_source_transition_render_offset, obs_source_transition_render_video_step,
};
use obs_source::transition_render::{AUDIO_FRAMES, TARGET_B};
use proptest::prelude::*;

const SEC: u64 = 1_000_000_000;

/// The guard the start frame is pre-filled with on both sides.
const GUARD: usize = 0x5a5a_5a5a;

#[test]
fn layout_matches_the_c_header() {
    let rust = [
        usize::try_from(TARGET_B).unwrap(),
        AUDIO_FRAMES,
        size_of::<obs_source_transition_render_video_step>(),
        OBS_SOURCE_TRANSITION_RENDER_VIDEO_END as usize,
        OBS_SOURCE_TRANSITION_RENDER_VIDEO_STOP as usize,
        OBS_SOURCE_TRANSITION_RENDER_VIDEO_STOP_AUDIO as usize,
        size_of::<obs_source_transition_render_draw_step>(),
        OBS_SOURCE_TRANSITION_RENDER_DRAW_CALLBACK as usize,
        OBS_SOURCE_TRANSITION_RENDER_DRAW_B as usize,
        OBS_SOURCE_TRANSITION_RENDER_DRAW_A as usize,
        size_of::<obs_source_transition_render_audio_step>(),
        OBS_SOURCE_TRANSITION_RENDER_AUDIO_KEEP as usize,
        OBS_SOURCE_TRANSITION_RENDER_AUDIO_END as usize,
        OBS_SOURCE_TRANSITION_RENDER_AUDIO_STOP as usize,
        OBS_SOURCE_TRANSITION_RENDER_MIX_A as usize,
        OBS_SOURCE_TRANSITION_RENDER_MIX_B as usize,
        OBS_SOURCE_TRANSITION_RENDER_COPY_A as usize,
        size_of::<obs_source_transition_render_audio_place>(),
        OBS_SOURCE_TRANSITION_RENDER_PLACE_NONE as usize,
        OBS_SOURCE_TRANSITION_RENDER_PLACE_LATE as usize,
        OBS_SOURCE_TRANSITION_RENDER_PLACE_MIX as usize,
        size_of::<obs_source_transition_render_offset>(),
        std::mem::offset_of!(obs_source_transition_render_offset, y),
        size_of::<c_int>(),
        size_of::<bool>(),
        size_of::<usize>(),
        // The C evaluates float as float (FLT_EVAL_METHOD 0), as Rust does.
        0,
    ];
    assert_eq!(rust.len(), LAYOUT_NAMES.len());
    for (which, (name, value)) in LAYOUT_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: a pure lookup.
        let c = unsafe { oracle::oracle_source_transition_render_layout(which) };
        assert_eq!(value, c, "{name}");
    }
}

// --------------------------------------------------------------------------
// One call on each side

fn video_due(t: f32, video: bool) -> (bool, bool) {
    // SAFETY: plain values.
    let c = unsafe { oracle::oracle_obs_source_transition_render_video_due(t, video) };
    (c, shim::obs_source_transition_render_video_due(t, video))
}

fn video_stop(audio: bool, active: bool) -> (u32, u32) {
    // SAFETY: plain values.
    let c = unsafe { oracle::oracle_obs_source_transition_render_video_stop(audio, active) };
    (
        c,
        shim::obs_source_transition_render_video_stop(audio, active),
    )
}

fn draw(video: bool, locked: bool, callback: bool, audio: bool) -> (u32, u32) {
    // SAFETY: plain values.
    let c =
        unsafe { oracle::oracle_obs_source_transition_render_draw(video, locked, callback, audio) };
    (
        c,
        shim::obs_source_transition_render_draw(video, locked, callback, audio),
    )
}

fn direct_index(transitioning: bool, target: c_int) -> (c_int, c_int) {
    // SAFETY: plain values.
    let c =
        unsafe { oracle::oracle_obs_source_transition_render_direct_index(transitioning, target) };
    (
        c,
        shim::obs_source_transition_render_direct_index(transitioning, target),
    )
}

fn audio_stop(has_ts: bool, t: f32, video: bool, audio: bool) -> (u32, u32) {
    // SAFETY: plain values.
    let c =
        unsafe { oracle::oracle_obs_source_transition_render_audio_stop(has_ts, t, video, audio) };
    (
        c,
        shim::obs_source_transition_render_audio_stop(has_ts, t, video, audio),
    )
}

fn audio_mix(audio: bool, a: bool, b: bool) -> (u32, u32) {
    // SAFETY: plain values.
    let c = unsafe { oracle::oracle_obs_source_transition_render_audio_mix(audio, a, b) };
    (c, shim::obs_source_transition_render_audio_mix(audio, a, b))
}

fn audio_valid(present: bool, pending: bool, duplicated: bool) -> (bool, bool) {
    // SAFETY: plain values.
    let c = unsafe {
        oracle::oracle_obs_source_transition_render_audio_valid(present, pending, duplicated)
    };
    (
        c,
        shim::obs_source_transition_render_audio_valid(present, pending, duplicated),
    )
}

/// The place and the frame (the guard when not written) on each side.
/// Whether `util_mul_div64()` is the 128-bit math that faults on overflow.
const UDIV128: bool = cfg!(all(target_env = "msvc", target_arch = "x86_64"));

/// Whether the C faults for this place: on MSVC x64 only, a timestamp that
/// is not 0 whose start frame does not fit in 64 bits.
fn c_faults(ts: u64, min_ts: u64, rate: usize) -> bool {
    let rate = u128::from(u64::try_from(rate).unwrap());
    let frames = u128::from(ts.wrapping_sub(min_ts)) * rate / u128::from(SEC);
    UDIV128 && ts != 0 && frames > u128::from(u64::MAX)
}

fn audio_place(ts: u64, min_ts: u64, rate: usize) -> ((u32, usize), (u32, usize)) {
    let mut c_pos = GUARD;
    let mut r_pos = GUARD;
    if c_faults(ts, min_ts, rate) {
        // SAFETY: r_pos is a valid size_t.
        let r =
            unsafe { shim::obs_source_transition_render_audio_place(ts, min_ts, rate, &mut r_pos) };
        let saturated = (OBS_SOURCE_TRANSITION_RENDER_PLACE_LATE, usize::MAX);
        assert_eq!(
            (r, r_pos),
            saturated,
            "audio_place({ts}, {min_ts}, {rate}) saturates"
        );
        return (saturated, saturated);
    }
    // SAFETY: c_pos is a valid size_t.
    let c = unsafe {
        oracle::oracle_obs_source_transition_render_audio_place(ts, min_ts, rate, &mut c_pos)
    };
    // SAFETY: r_pos is a valid size_t.
    let r = unsafe { shim::obs_source_transition_render_audio_place(ts, min_ts, rate, &mut r_pos) };
    ((c, c_pos), (r, r_pos))
}

fn offset(args: [f32; 6]) -> (OracleOffset, obs_source_transition_render_offset) {
    let [a, b, c, d, e, f] = args;
    // SAFETY: plain values.
    let co = unsafe { oracle::oracle_obs_source_transition_render_offset(a, b, c, d, e, f) };
    (
        co,
        shim::obs_source_transition_render_offset(a, b, c, d, e, f),
    )
}

#[track_caller]
fn same<T: PartialEq + std::fmt::Debug>(pair: (T, T), what: &str) {
    assert_eq!(pair.0, pair.1, "{what}: C {:?}, shim {:?}", pair.0, pair.1);
}

/// One side of an offset: `tr - source * scale`. Bit for bit, except that
/// where an operation has two NaN operands both must only be NaN.
#[track_caller]
fn same_offset_side(c: f32, r: f32, tr: f32, source: f32, scale: f32, what: &str) {
    let two_nans_mul = source.is_nan() && scale.is_nan();
    let product = source * scale;
    let two_nans_sub = tr.is_nan() && product.is_nan();
    if (two_nans_mul || two_nans_sub) && c.is_nan() && r.is_nan() {
        return;
    }
    assert_eq!(
        c.to_bits(),
        r.to_bits(),
        "{what}: C {c:e} ({:#010x}), shim {r:e} ({:#010x})",
        c.to_bits(),
        r.to_bits()
    );
}

#[track_caller]
fn same_offset(args: [f32; 6]) {
    let (c, r) = offset(args);
    let what = format!("offset({:#010x?})", args.map(f32::to_bits));
    same_offset_side(c.x, r.x, args[0], args[2], args[4], &what);
    same_offset_side(c.y, r.y, args[1], args[3], args[5], &what);
}

// --------------------------------------------------------------------------
// Special values

/// 40 values of `t`: around 0 and 1, the ends, infinities and NaNs.
fn special_t() -> Vec<f32> {
    let mut v: Vec<f32> = [
        0x0000_0000_u32,
        0x8000_0000,
        0x0000_0001,
        0x8000_0001,
        0x007f_ffff,
        0x0080_0000,
        0x3f00_0000,
        0x3f7f_fffe,
        0x3f7f_ffff,
        0x3f80_0000,
        0x3f80_0001,
        0x3f80_0002,
        0xbf7f_ffff,
        0xbf80_0000,
        0xbf80_0001,
        0x4000_0000,
        0x4b80_0000,
        0x7f7f_ffff,
        0xff7f_ffff,
        0x7f80_0000,
        0xff80_0000,
        0x7f80_0001,
        0x7fbf_ffff,
        0x7fc0_0000,
        0x7fc0_0001,
        0x7fff_ffff,
        0xff80_0001,
        0xffc0_0000,
        0xffff_ffff,
    ]
    .map(f32::from_bits)
    .to_vec();
    v.extend([
        0.25, 0.75, 0.999, 1.001, 0.5e-30, 3.0, 100.0, -0.5, 1e30, -1e30, 1e-30,
    ]);
    v
}

/// 26 timestamps: 0, small, around a few frame edges, around 2^32, 2^53,
/// 2^63 and the top.
const SPECIAL_TS: [u64; 26] = [
    0,
    1,
    2,
    20_833,
    20_834,
    21_333_333,
    21_333_334,
    21_354_166,
    21_354_167,
    23_219_955,
    23_242_631,
    SEC,
    5 * SEC,
    5 * SEC + 21_333_334,
    (1 << 32) - 1,
    1 << 32,
    (1 << 53) + 1,
    (1 << 63) - 1,
    1 << 63,
    (1 << 63) + 1,
    u64::MAX - 21_354_167,
    u64::MAX - 21_333_334,
    u64::MAX - 2,
    u64::MAX - 1,
    u64::MAX,
    0x0123_4567_89ab_cdef,
];

/// 14 sample rates, up to where util_mul_div64() wraps.
const SPECIAL_RATES: [usize; 14] = [
    0,
    1,
    8000,
    22050,
    44100,
    48000,
    96000,
    192_000,
    1_000_000_000,
    1_000_000_001,
    18_446_744_073,
    1 << 40,
    usize::MAX - 1,
    usize::MAX,
];

/// 14 sizes and scales: zeros, ones, typical, fractional, the ends of
/// `f32`, the largest `u32` size, infinities and NaNs.
fn special_f32() -> [f32; 14] {
    [
        0.0,
        -0.0,
        1.0,
        1.5,
        1080.0,
        1920.0,
        f32::from_bits(0x3eaa_aaab),
        f32::from_bits(0x3f83_4835),
        4_294_967_296.0,
        f32::MAX,
        f32::from_bits(1),
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
    ]
}

// --------------------------------------------------------------------------
// Exhaustive runs

#[test]
fn flags_every_combination() {
    for audio in [false, true] {
        for active in [false, true] {
            same(video_stop(audio, active), "video_stop");
        }
    }
    for flags in 0..16 {
        same(
            draw(
                flags & 1 != 0,
                flags & 2 != 0,
                flags & 4 != 0,
                flags & 8 != 0,
            ),
            "draw",
        );
    }
    for flags in 0..8 {
        same(
            audio_mix(flags & 1 != 0, flags & 2 != 0, flags & 4 != 0),
            "audio_mix",
        );
        same(
            audio_valid(flags & 1 != 0, flags & 2 != 0, flags & 4 != 0),
            "audio_valid",
        );
    }
}

#[test]
fn direct_index_every_target() {
    let mut targets: Vec<c_int> = (-70_000..=70_000).collect();
    targets.extend((i32::MIN..=i32::MAX).step_by(65_521));
    targets.extend([i32::MIN, i32::MIN + 1, i32::MAX - 1, i32::MAX]);
    for target in targets {
        for transitioning in [false, true] {
            same(direct_index(transitioning, target), "direct_index");
        }
    }
}

#[test]
fn due_and_audio_stop_special_t() {
    for t in special_t() {
        for flags in 0..8 {
            let (a, b, c) = (flags & 1 != 0, flags & 2 != 0, flags & 4 != 0);
            same(video_due(t, a), "video_due");
            same(audio_stop(a, t, b, c), "audio_stop");
        }
    }
}

/// The three stop steps as the render functions apply them, from the C's
/// and the shim's answers each: the flags afterwards, the video stop signal
/// and obs_transition_stop().
#[test]
fn stop_steps_as_applied() {
    fn video_apply(
        due: bool,
        step: Option<u32>,
        video: bool,
        audio: bool,
    ) -> (bool, bool, bool, bool) {
        let (mut v, mut a, mut signal, mut stop) = (video, audio, false, false);
        if due {
            v = false;
            signal = true;
            let step = step.unwrap();
            if step == OBS_SOURCE_TRANSITION_RENDER_VIDEO_STOP_AUDIO {
                a = false;
            }
            stop = step != OBS_SOURCE_TRANSITION_RENDER_VIDEO_END;
        }
        (v, a, signal, stop)
    }
    fn audio_apply(step: u32, audio: bool) -> (bool, bool) {
        let a = audio && step == OBS_SOURCE_TRANSITION_RENDER_AUDIO_KEEP;
        (a, step == OBS_SOURCE_TRANSITION_RENDER_AUDIO_STOP)
    }
    for t in special_t() {
        for flags in 0..16 {
            let video = flags & 1 != 0;
            let audio = flags & 2 != 0;
            let active = flags & 4 != 0;
            let direct = flags & 8 != 0;
            let active = if direct { active } else { true };
            let (c_due, r_due) = video_due(t, video);
            let (c_step, r_step) = video_stop(audio, active);
            assert_eq!(
                video_apply(c_due, Some(c_step), video, audio),
                video_apply(r_due, Some(r_step), video, audio),
                "video step t={:#010x} flags={flags}",
                t.to_bits()
            );
            for has_ts in [false, true] {
                let (c, r) = audio_stop(has_ts, t, video, audio);
                assert_eq!(audio_apply(c, audio), audio_apply(r, audio));
            }
        }
    }
}

fn f32_patterns(stride: usize) -> impl Iterator<Item = f32> {
    let near_one = (0x3f80_0000_u32 - 65_536)..=(0x3f80_0000 + 65_536);
    (0..=u32::MAX)
        .step_by(stride)
        .chain(near_one)
        .chain([u32::MAX])
        .map(f32::from_bits)
}

#[test]
fn due_and_audio_stop_f32_patterns() {
    for t in f32_patterns(4099) {
        same(video_due(t, true), "video_due");
        same(audio_stop(true, t, false, true), "audio_stop");
        same(audio_stop(true, t, true, true), "audio_stop");
    }
}

/// Every `f32`: run with `--release --ignored`.
#[test]
#[ignore = "every f32 bit pattern: run once with --release --ignored"]
fn due_and_audio_stop_every_f32() {
    for t in f32_patterns(1) {
        same(video_due(t, true), "video_due");
        same(audio_stop(true, t, false, true), "audio_stop");
        same(audio_stop(true, t, true, true), "audio_stop");
    }
}

#[test]
fn place_special_values() {
    for ts in SPECIAL_TS {
        for min_ts in SPECIAL_TS {
            for rate in SPECIAL_RATES {
                same(audio_place(ts, min_ts, rate), "audio_place");
            }
        }
    }
}

#[test]
fn place_every_frame_edge() {
    for rate in [8000_u64, 22050, 32000, 44100, 48000, 88200, 96000, 192_000] {
        let rate_usize = usize::try_from(rate).unwrap();
        for k in 0_u64..1100 {
            let first = (k * SEC).div_ceil(rate);
            for min_ts in [1, 7 * SEC, u64::MAX - 5 * SEC] {
                for d in [first.saturating_sub(1), first, first + 1] {
                    same(
                        audio_place(min_ts.wrapping_add(d), min_ts, rate_usize),
                        "audio_place",
                    );
                    same(
                        audio_place(min_ts.wrapping_sub(d), min_ts, rate_usize),
                        "audio_place wrapped",
                    );
                }
            }
        }
    }
}

#[test]
fn place_null_pos_is_not_written_by_the_shim() {
    // Intentional difference: the C would write through the null pointer.
    for ts in [0, 1, 5 * SEC, u64::MAX] {
        // SAFETY: a null pos is never written by the shim.
        let r = unsafe {
            shim::obs_source_transition_render_audio_place(ts, 1, 48000, std::ptr::null_mut())
        };
        let ((c, _), _) = audio_place(ts, 1, 48000);
        assert_eq!(r, c);
    }
}

#[test]
fn offset_special_values() {
    let v = special_f32();
    for tr in v {
        for source in v {
            for scale in v {
                // each side on its own, the other side typical
                same_offset([tr, 1080.0, source, 720.0, scale, 1.5]);
                same_offset([1920.0, tr, 1280.0, source, 1.5, scale]);
            }
        }
    }
    // both sides at once
    for (i, a) in v.iter().enumerate() {
        for b in &v[i..] {
            same_offset([*a, *b, *b, *a, *a, *b]);
            same_offset([*b, *a, *a, *b, *b, *a]);
        }
    }
}

// --------------------------------------------------------------------------
// Proptests

fn any_f32() -> impl Strategy<Value = f32> {
    prop_oneof![
        any::<u32>().prop_map(f32::from_bits),
        (0x3f7f_ff00_u32..=0x3f80_0100).prop_map(f32::from_bits),
    ]
}

fn any_size() -> impl Strategy<Value = f32> {
    prop_oneof![
        any::<u32>().prop_map(f32::from_bits),
        any::<u32>().prop_map(|v| v as f32),
        (0_u32..8192).prop_map(|v| v as f32),
    ]
}

fn any_scale() -> impl Strategy<Value = f32> {
    prop_oneof![
        any::<u32>().prop_map(f32::from_bits),
        (1_u32..8192, 1_u32..8192).prop_map(|(a, b)| a as f32 / b as f32),
    ]
}

fn any_rate() -> impl Strategy<Value = usize> {
    prop_oneof![
        any::<usize>(),
        0_usize..=1_000_000_000,
        prop::sample::select(SPECIAL_RATES.to_vec()),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20_000))]

    #[test]
    fn prop_steps(t in any_f32(), flags in 0_u8..32, target: i32) {
        let f = |bit: u8| flags & (1 << bit) != 0;
        prop_assert_eq!(video_due(t, f(0)).0, video_due(t, f(0)).1);
        let s = video_stop(f(1), f(2));
        prop_assert_eq!(s.0, s.1);
        let s = draw(f(0), f(1), f(2), f(3));
        prop_assert_eq!(s.0, s.1);
        let s = direct_index(f(4), target);
        prop_assert_eq!(s.0, s.1);
        let s = audio_stop(f(4), t, f(0), f(1));
        prop_assert_eq!(s.0, s.1);
        let s = audio_mix(f(1), f(2), f(3));
        prop_assert_eq!(s.0, s.1);
        let s = audio_valid(f(2), f(3), f(4));
        prop_assert_eq!(s.0, s.1);
    }

    #[test]
    fn prop_place(
        min_ts: u64,
        diff in prop_oneof![any::<u64>(), 0_u64..60_000_000],
        below: bool,
        zero_ts: bool,
        rate in any_rate(),
    ) {
        let ts = if zero_ts {
            0
        } else if below {
            min_ts.wrapping_sub(diff)
        } else {
            min_ts.wrapping_add(diff)
        };
        let (c, r) = audio_place(ts, min_ts, rate);
        prop_assert_eq!(c, r);
    }

    #[test]
    fn prop_offset(
        tr_cx in any_size(),
        tr_cy in any_size(),
        source_cx in any_size(),
        source_cy in any_size(),
        scale_x in any_scale(),
        scale_y in any_scale(),
    ) {
        same_offset([tr_cx, tr_cy, source_cx, source_cy, scale_x, scale_y]);
    }
}
