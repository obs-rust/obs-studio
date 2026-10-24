//! Tier 1: `obs_source::transition_render`, the render and audio step
//! decisions of the transitions.
//!
//! The `steps_*` cases port `test/cmocka/test_transition_render_steps.c`
//! 1:1, under the same case names, in the same order, with the same inputs
//! and expectations, the "characterized, not endorsed" cases included. The
//! stop steps run as the render functions apply them.
//!
//! The `edge_*` cases are new: the due test and the audio stop step over any
//! `f32` bit pattern; the draw choice, the direct index and the mix against
//! their rules over every flag and any `i32` target; the start frame
//! against an exact 128-bit model wherever `util_mul_div64()` cannot
//! overflow, its order in `ts`, and the wrap; and the offsets against an
//! `f64` model (one rounded multiply, then one rounded subtract) over any
//! `f32` bit patterns.

use obs_source::transition_render::{
    AUDIO_FRAMES, AudioMix, AudioStep, Draw, Offset, Place, TARGET_B, VideoStep, audio_mix,
    audio_place, audio_stop, audio_valid, direct_index, draw, offset, video_due, video_stop,
};
use proptest::prelude::*;

const TARGET_A: i32 = 0;
const SEC: u64 = 1_000_000_000;

const BELOW_ONE: u32 = 0x3f7f_ffff;
const ONE: u32 = 0x3f80_0000;
const ABOVE_ONE: u32 = 0x3f80_0001;
const QNAN: u32 = 0x7fc0_0000;

/// Every t the stop steps are pinned at, and whether `t >= 1` holds.
const T_CASES: [(u32, bool); 14] = [
    (0x0000_0000, false), // 0
    (0x8000_0000, false), // -0
    (0x0000_0001, false), // the smallest subnormal
    (0x3f00_0000, false), // 0.5
    (BELOW_ONE, false),
    (ONE, true),
    (ABOVE_ONE, true),
    (0x4000_0000, true),  // 2
    (0x7f80_0000, true),  // inf
    (0xff80_0000, false), // -inf
    (0xbf80_0000, false), // -1
    (QNAN, false),        // characterized, not endorsed: a NaN t never ends a transition
    (0xffc0_0000, false), // -NaN
    (0x7f80_0001, false), // a signalling NaN
];

#[test]
fn steps_video_due_at_and_around_one() {
    for (bits, at_least_one) in T_CASES {
        let t = f32::from_bits(bits);
        assert_eq!(
            video_due(t, true),
            at_least_one,
            "video_due({bits:#010x}, true)"
        );
        // never due while the video is not transitioning
        assert!(!video_due(t, false), "video_due({bits:#010x}, false)");
    }
}

#[test]
fn steps_video_stop_every_flag() {
    // the audio runs on: only the video ends
    assert_eq!(video_stop(true, true), VideoStep::End);
    // the audio had ended: the transition stops
    assert_eq!(video_stop(false, true), VideoStep::Stop);
    // not active (the direct render only): the audio ends too, and the
    // transition stops, whether or not the audio was still running
    assert_eq!(video_stop(true, false), VideoStep::StopAudio);
    assert_eq!(video_stop(false, false), VideoStep::StopAudio);
}

/// The stop step as the render functions apply it under the lock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VideoOutcome {
    video: bool,
    audio: bool,
    /// `"source_transition_video_stop"`
    video_stopped: bool,
    /// `obs_transition_stop()` and `handle_stop()`
    stopped: bool,
}

fn apply_video_step(t: f32, video: bool, audio: bool, direct: bool, active: bool) -> VideoOutcome {
    let mut out = VideoOutcome {
        video,
        audio,
        video_stopped: false,
        stopped: false,
    };
    if video_due(t, out.video) {
        out.video = false;
        out.video_stopped = true;
        let step = video_stop(out.audio, if direct { active } else { true });
        if step == VideoStep::StopAudio {
            out.audio = false;
        }
        if step != VideoStep::End {
            out.stopped = true;
        }
    }
    out
}

#[test]
fn steps_render2_stop_step_combinations() {
    for bits in [BELOW_ONE, ONE, ABOVE_ONE, QNAN] {
        let due_t = bits == ONE || bits == ABOVE_ONE;
        for flags in 0..4 {
            let video = flags & 1 != 0;
            let audio = flags & 2 != 0;
            let mut want = VideoOutcome {
                video,
                audio,
                video_stopped: false,
                stopped: false,
            };
            if due_t && video {
                want.video = false;
                want.video_stopped = true;
                want.stopped = !audio;
            }
            assert_eq!(
                apply_video_step(f32::from_bits(bits), video, audio, false, false),
                want,
                "render2 t={bits:#010x} flags={flags}"
            );
        }
    }
}

#[test]
fn steps_direct_stop_step_combinations() {
    for bits in [BELOW_ONE, ONE, ABOVE_ONE, QNAN] {
        let due_t = bits == ONE || bits == ABOVE_ONE;
        for flags in 0..8 {
            let video = flags & 1 != 0;
            let audio = flags & 2 != 0;
            let active = flags & 4 != 0;
            let mut want = VideoOutcome {
                video,
                audio,
                video_stopped: false,
                stopped: false,
            };
            if due_t && video {
                want.video = false;
                want.video_stopped = true;
                if !active {
                    want.audio = false;
                }
                want.stopped = !want.audio;
            }
            assert_eq!(
                apply_video_step(f32::from_bits(bits), video, audio, true, active),
                want,
                "direct t={bits:#010x} flags={flags}"
            );
        }
    }
}

#[test]
fn steps_draw_every_flag() {
    for flags in 0..16 {
        let video = flags & 1 != 0;
        let locked = flags & 2 != 0;
        let callback = flags & 4 != 0;
        let audio = flags & 8 != 0;
        let want = if video && locked && callback {
            Draw::Callback
        } else if audio {
            Draw::B
        } else {
            Draw::A
        };
        assert_eq!(
            draw(video, locked, callback, audio),
            want,
            "draw({video}, {locked}, {callback}, {audio})"
        );
    }

    // characterized, not endorsed: a video transition whose textures are
    // busy, or that has no callback, draws B only while its audio also
    // transitions, and source A (the old scene) otherwise
    assert_eq!(draw(true, false, true, false), Draw::A);
    assert_eq!(draw(true, true, false, false), Draw::A);
    assert_eq!(draw(true, false, true, true), Draw::B);
}

#[test]
fn steps_direct_index_every_flag() {
    assert_eq!(direct_index(false, TARGET_A), 0);
    assert_eq!(direct_index(false, TARGET_B), 0);
    assert_eq!(direct_index(true, TARGET_A), 0);
    assert_eq!(direct_index(true, TARGET_B), 1);

    // targets outside the enum draw A
    for other in [2, -1, i32::MAX, i32::MIN] {
        assert_eq!(direct_index(true, other), 0, "target {other}");
        assert_eq!(direct_index(false, other), 0, "target {other}");
    }
}

#[test]
fn steps_audio_stop_with_timestamp() {
    for (bits, at_least_one) in T_CASES {
        let t = f32::from_bits(bits);
        for flags in 0..4 {
            let video = flags & 1 != 0;
            let audio = flags & 2 != 0;
            let want = if at_least_one && audio {
                if video {
                    AudioStep::End
                } else {
                    AudioStep::Stop
                }
            } else {
                AudioStep::Keep
            };
            assert_eq!(
                audio_stop(Some(t), video, audio),
                want,
                "audio_stop(Some({bits:#010x}), {video}, {audio})"
            );
        }
    }
}

#[test]
fn steps_audio_stop_without_timestamp() {
    // No child gave a timestamp: the audio ends only once the video has
    // ended (7af9c2d88), so a child whose audio starts late still joins.
    // The C does not ask the clock without a timestamp, so there is no t
    // here; the cmocka case passes each of its t values, which the C
    // ignores.
    for _ in T_CASES {
        assert_eq!(audio_stop(None, false, false), AudioStep::Keep);
        assert_eq!(audio_stop(None, false, true), AudioStep::Stop);
        assert_eq!(audio_stop(None, true, false), AudioStep::Keep);
        assert_eq!(audio_stop(None, true, true), AudioStep::Keep);
    }
}

/// What `obs_transition_audio_render()` passes for one child: a child that
/// is not there or removed (`obs_source_removed()`) is dropped before the
/// timestamps are read, and one whose audio is pending passes 0, as one
/// whose timestamp is 0 does (`transition_calc_min_ts()`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChildState {
    Absent,
    Removed,
    Pending,
    NoTs,
    HasTs,
}

const CHILD_STATES: [ChildState; 5] = [
    ChildState::Absent,
    ChildState::Removed,
    ChildState::Pending,
    ChildState::NoTs,
    ChildState::HasTs,
];

#[test]
fn steps_audio_step_children() {
    for a in CHILD_STATES {
        for b in CHILD_STATES {
            let has_ts = a == ChildState::HasTs || b == ChildState::HasTs;
            for flags in 0..4 {
                let video = flags & 1 != 0;
                let audio = flags & 2 != 0;
                let at = |t: f32| if has_ts { Some(t) } else { None };

                // at t = 1
                let want = if audio && (has_ts || !video) {
                    if video {
                        AudioStep::End
                    } else {
                        AudioStep::Stop
                    }
                } else {
                    AudioStep::Keep
                };
                assert_eq!(
                    audio_stop(at(1.0), video, audio),
                    want,
                    "children {a:?} {b:?} flags {flags}"
                );
                // just below 1: only the no-timestamp rule ends the audio
                let want = if !has_ts && audio && !video {
                    AudioStep::Stop
                } else {
                    AudioStep::Keep
                };
                assert_eq!(
                    audio_stop(at(f32::from_bits(BELOW_ONE)), video, audio),
                    want,
                    "children {a:?} {b:?} flags {flags} below 1"
                );
            }
        }
    }

    // the copied children after the re-read: each one there is mixed while
    // the audio transitions; a removed, pending or duplicated one is still
    // passed to transition_process_audio(), which then reads nothing
    // (characterized, not endorsed: the re-read does not drop removed
    // children; transition_process_audio() checks only pending and
    // duplicated)
    assert_eq!(
        audio_mix(true, true, true),
        AudioMix {
            mix_a: true,
            mix_b: true,
            copy_a: false
        }
    );
    assert!(!audio_valid(true, true, false));
}

const fn mix(mix_a: bool, mix_b: bool, copy_a: bool) -> AudioMix {
    AudioMix {
        mix_a,
        mix_b,
        copy_a,
    }
}

#[test]
fn steps_audio_mix_every_flag() {
    assert_eq!(audio_mix(false, false, false), mix(false, false, false));
    assert_eq!(audio_mix(false, true, false), mix(false, false, true));
    assert_eq!(audio_mix(false, false, true), mix(false, false, false));
    assert_eq!(audio_mix(false, true, true), mix(false, false, true));
    assert_eq!(audio_mix(true, false, false), mix(false, false, false));
    assert_eq!(audio_mix(true, true, false), mix(true, false, false));
    assert_eq!(audio_mix(true, false, true), mix(false, true, false));
    assert_eq!(audio_mix(true, true, true), mix(true, true, false));
}

#[test]
fn steps_audio_valid_every_flag() {
    for flags in 0..8 {
        let present = flags & 1 != 0;
        let pending = flags & 2 != 0;
        let duplicated = flags & 4 != 0;
        assert_eq!(
            audio_valid(present, pending, duplicated),
            present && !pending && !duplicated,
            "audio_valid({present}, {pending}, {duplicated})"
        );
    }
}

#[track_caller]
fn check_place(ts: u64, min_ts: u64, rate: usize, want: Place) {
    assert_eq!(
        audio_place(ts, min_ts, rate),
        want,
        "audio_place({ts}, {min_ts}, {rate})"
    );
}

#[test]
fn steps_audio_place_no_timestamp() {
    // a timestamp of 0 (3191f5ddb): nothing more is read, pos untouched
    check_place(0, 0, 48000, Place::None);
    check_place(0, 5 * SEC, 48000, Place::None);
    check_place(0, u64::MAX, 44100, Place::None);
}

#[test]
fn steps_audio_place_around_min_ts() {
    let min_ts = 5 * SEC;

    // at 48 kHz a frame is 20833.3 ns: frame k starts at ceil(k * 1e9 / 48000)
    check_place(min_ts, min_ts, 48000, Place::Mix(0));
    check_place(min_ts + 1, min_ts, 48000, Place::Mix(0));
    check_place(min_ts + 20833, min_ts, 48000, Place::Mix(0));
    check_place(min_ts + 20834, min_ts, 48000, Place::Mix(1));
    check_place(min_ts + 21_312_499, min_ts, 48000, Place::Mix(1022));
    check_place(min_ts + 21_312_500, min_ts, 48000, Place::Mix(1023));
    check_place(min_ts + 21_333_333, min_ts, 48000, Place::Mix(1023));
    // characterized, not endorsed: a start at AUDIO_OUTPUT_FRAMES is not
    // late (the scene's window uses >=); mix_child() then mixes 0 frames
    check_place(min_ts + 21_333_334, min_ts, 48000, Place::Mix(AUDIO_FRAMES));
    check_place(min_ts + 21_354_166, min_ts, 48000, Place::Mix(AUDIO_FRAMES));
    check_place(
        min_ts + 21_354_167,
        min_ts,
        48000,
        Place::Late(AUDIO_FRAMES + 1),
    );
    check_place(min_ts + SEC, min_ts, 48000, Place::Late(48000));

    // 44.1 kHz: frame 1024 starts at 23219955 ns, frame 1025 at 23242631
    check_place(min_ts + 23_219_954, min_ts, 44100, Place::Mix(1023));
    check_place(min_ts + 23_219_955, min_ts, 44100, Place::Mix(AUDIO_FRAMES));
    check_place(min_ts + 23_242_630, min_ts, 44100, Place::Mix(AUDIO_FRAMES));
    check_place(
        min_ts + 23_242_631,
        min_ts,
        44100,
        Place::Late(AUDIO_FRAMES + 1),
    );

    // a sample rate of 0 places everything at 0; 1e9 places by the ns
    check_place(min_ts + 7 * SEC, min_ts, 0, Place::Mix(0));
    check_place(min_ts + 1024, min_ts, 1_000_000_000, Place::Mix(1024));
    check_place(min_ts + 1025, min_ts, 1_000_000_000, Place::Late(1025));
    check_place(min_ts + SEC, min_ts, 1, Place::Mix(1));
}

/// `(2^64 - 1) * 48000 / 1e9` by the libobs formula.
const WRAPPED_48K: u64 = 885_443_715_538_058;

#[test]
fn steps_audio_place_u64_wrap() {
    // characterized, not endorsed: a timestamp below min_ts wraps in
    // uint64_t; with a 64-bit size_t the frame is far past the tick, so it
    // is late (read, nothing mixed)
    if let Ok(wrapped) = usize::try_from(WRAPPED_48K) {
        check_place(5 * SEC - 1, 5 * SEC, 48000, Place::Late(wrapped));
    }
    // a min_ts past UINT64_MAX's wrap: ts - min_ts lands near 0 again
    check_place(1, u64::MAX, 48000, Place::Mix(0));
    check_place(20834, u64::MAX, 48000, Place::Mix(1));
    check_place(u64::MAX, u64::MAX, 48000, Place::Mix(0));
    if let Ok(wrapped) = usize::try_from(WRAPPED_48K) {
        check_place(u64::MAX, 1, 48000, Place::Late(wrapped));
    }
}

#[track_caller]
#[allow(clippy::too_many_arguments)]
fn check_offset(
    tr_cx: f32,
    tr_cy: f32,
    source_cx: f32,
    source_cy: f32,
    scale_x: f32,
    scale_y: f32,
    want_x: u32,
    want_y: u32,
) {
    let got = offset(tr_cx, tr_cy, source_cx, source_cy, scale_x, scale_y);
    assert_eq!(
        (got.x.to_bits(), got.y.to_bits()),
        (want_x, want_y),
        "offset({tr_cx}, {tr_cy}, {source_cx}, {source_cy}, {:#010x}, {:#010x}) is {got:?}",
        scale_x.to_bits(),
        scale_y.to_bits()
    );
}

fn bits(value: f32) -> u32 {
    value.to_bits()
}

#[test]
fn steps_offset_each_scale_type() {
    // 1280x720 in 1920x1080: max-only keeps 1, aspect and stretch scale 1.5
    check_offset(
        1920.0,
        1080.0,
        1280.0,
        720.0,
        1.0,
        1.0,
        bits(640.0),
        bits(360.0),
    );
    check_offset(
        1920.0,
        1080.0,
        1280.0,
        720.0,
        1.5,
        1.5,
        bits(0.0),
        bits(0.0),
    );

    // 1000x1000 in 1920x1080: aspect fits the height, stretch both sides
    check_offset(
        1920.0,
        1080.0,
        1000.0,
        1000.0,
        1.08,
        1.08,
        bits(840.0),
        bits(0.0),
    );
    check_offset(
        1920.0,
        1080.0,
        1000.0,
        1000.0,
        1.92,
        1.08,
        bits(0.0),
        bits(0.0),
    );

    // 3840x2160 in 1920x1080 (max-only fits a larger child as aspect)
    check_offset(
        1920.0,
        1080.0,
        3840.0,
        2160.0,
        0.5,
        0.5,
        bits(0.0),
        bits(0.0),
    );
    // a larger child at a scale of 1, the scale's initial value
    check_offset(
        1920.0,
        1080.0,
        3840.0,
        2160.0,
        1.0,
        1.0,
        bits(-1920.0),
        bits(-1080.0),
    );

    // 32x16 in 64x64 (test_transition_scale_type's case)
    check_offset(64.0, 64.0, 32.0, 16.0, 1.0, 1.0, bits(32.0), bits(48.0));
    check_offset(64.0, 64.0, 32.0, 16.0, 2.0, 2.0, bits(0.0), bits(32.0));
    check_offset(64.0, 64.0, 32.0, 16.0, 2.0, 4.0, bits(0.0), bits(0.0));
}

#[test]
fn steps_offset_rounding_and_specials() {
    // the product is rounded to float before the subtract: 3 * 0x3eaaaaab
    // is 1.00000002, which rounds to 1, so 1 - 1 is exactly 0 (a fused
    // multiply-subtract would give -2^-25)
    let third = f32::from_bits(0x3eaa_aaab);
    check_offset(1.0, 1.0, 3.0, 3.0, third, third, 0x0000_0000, 0x0000_0000);
    // 1053 * (1080 / 1053 as float) rounds to 1080.0001: the offset is
    // -2^-13, just below 0 (the (int) cast in the C makes it 0)
    let q = f32::from_bits(0x3f83_4835);
    check_offset(
        1080.0,
        1080.0,
        1053.0,
        1053.0,
        q,
        q,
        0xb900_0000,
        0xb900_0000,
    );

    // a transition of size 0 (scale 0 from the fit)
    check_offset(0.0, 0.0, 100.0, 100.0, 0.0, 0.0, bits(0.0), bits(0.0));
    // characterized, not endorsed: an infinite scale (from a transition of
    // size 0 on the other side) gives -inf, and inf - inf is NaN
    check_offset(
        1920.0,
        1080.0,
        100.0,
        100.0,
        f32::INFINITY,
        f32::INFINITY,
        0xff80_0000,
        0xff80_0000,
    );
    let got = offset(f32::INFINITY, 1.0, 100.0, 1.0, f32::INFINITY, 1.0);
    assert!(got.x.is_nan());
    assert_eq!(got.y.to_bits(), bits(0.0));
    // a NaN scale is a NaN offset
    let got = offset(1920.0, 1080.0, 100.0, 100.0, f32::NAN, 1.0);
    assert!(got.x.is_nan());
    assert_eq!(got.y.to_bits(), bits(980.0));
    // the largest sizes: a u32 size is 4294967296.0f as float
    check_offset(
        4_294_967_296.0,
        1.0,
        1.0,
        1.0,
        1.0,
        1.0,
        bits(4_294_967_296.0),
        bits(0.0),
    );
    check_offset(
        1.0,
        1.0,
        4_294_967_296.0,
        1.0,
        1.0,
        1.0,
        bits(-4_294_967_296.0),
        bits(0.0),
    );
}

// ---- edge cases (new) ----

/// Any `f32`, from its bit pattern (never a wide float range, #1951), half
/// of them near 1.
fn any_t() -> impl Strategy<Value = f32> {
    prop_oneof![
        any::<u32>().prop_map(f32::from_bits),
        (0x3f7f_fff0_u32..=0x3f80_0010).prop_map(f32::from_bits),
    ]
}

/// Sizes and scales as the matrix has them: any bit pattern, or a whole
/// number as a `u32` size converts.
fn any_size() -> impl Strategy<Value = f32> {
    prop_oneof![
        any::<u32>().prop_map(f32::from_bits),
        any::<u32>().prop_map(|v| v as f32),
        (0_u32..5000).prop_map(|v| v as f32),
    ]
}

fn any_scale() -> impl Strategy<Value = f32> {
    prop_oneof![
        any::<u32>().prop_map(f32::from_bits),
        (1_u32..5000, 1_u32..5000).prop_map(|(a, b)| a as f32 / b as f32),
    ]
}

/// The C's `util_mul_div64(num, rate, 1e9)` where it cannot overflow (a rate
/// up to 1e9): the exact quotient, floored.
fn exact_frames(num: u64, rate: u64) -> u64 {
    u64::try_from(u128::from(num) * u128::from(rate) / u128::from(SEC)).unwrap()
}

/// The `f32` result of `a - b * c` with the product rounded first, from
/// `f64` (each product of two `f32` is exact in `f64`, and one `f64`
/// operation on `f32` values rounds to `f32` as the `f32` operation does).
fn model_offset(a: f32, b: f32, c: f32) -> f32 {
    let product = (f64::from(b) * f64::from(c)) as f32;
    (f64::from(a) - f64::from(product)) as f32
}

fn same_f32(a: f32, b: f32) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

#[test]
fn edge_draw_and_index_every_flag() {
    for flags in 0..16 {
        let video = flags & 1 != 0;
        let locked = flags & 2 != 0;
        let callback = flags & 4 != 0;
        let audio = flags & 8 != 0;
        // only the callback draws both, and only with all three
        assert_eq!(
            draw(video, locked, callback, audio) == Draw::Callback,
            video && locked && callback
        );
        // never A while the audio transitions
        if audio {
            assert_ne!(draw(video, locked, callback, audio), Draw::A);
        }
    }
    for target in [TARGET_A, TARGET_B, 2, 3, -1, -2, i32::MAX, i32::MIN] {
        assert_eq!(direct_index(true, target), usize::from(target == TARGET_B));
        assert_eq!(direct_index(false, target), 0);
    }
}

#[test]
fn edge_audio_mix_never_both() {
    for flags in 0..8 {
        let audio = flags & 1 != 0;
        let a = flags & 2 != 0;
        let b = flags & 4 != 0;
        let got = audio_mix(audio, a, b);
        // a copy only without the audio transition, and never with a mix
        assert!(!(got.copy_a && (got.mix_a || got.mix_b)));
        assert_eq!(got.copy_a, !audio && a);
        assert_eq!(got.mix_a, audio && a);
        assert_eq!(got.mix_b, audio && b);
    }
}

#[test]
fn edge_audio_place_at_every_frame_edge() {
    // every frame k from 0 to 1100 at the common rates: the first ns of
    // frame k is frame k, the ns before it frame k - 1
    for rate in [8000_u64, 22050, 32000, 44100, 48000, 88200, 96000, 192_000] {
        for k in 0_u64..1100 {
            let first = (k * SEC).div_ceil(rate);
            let place = audio_place(7 * SEC + first, 7 * SEC, usize::try_from(rate).unwrap());
            let k_usize = usize::try_from(k).unwrap();
            let want = if k_usize > AUDIO_FRAMES {
                Place::Late(k_usize)
            } else {
                Place::Mix(k_usize)
            };
            assert_eq!(place, want, "rate {rate} frame {k}");
            if first > 0 {
                let before =
                    audio_place(7 * SEC + first - 1, 7 * SEC, usize::try_from(rate).unwrap());
                let k1 = k_usize - 1;
                let want = if k1 > AUDIO_FRAMES {
                    Place::Late(k1)
                } else {
                    Place::Mix(k1)
                };
                assert_eq!(before, want, "rate {rate} before frame {k}");
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20_000))]

    #[test]
    fn edge_video_due_any_t(t in any_t(), video: bool) {
        prop_assert_eq!(video_due(t, video), video && t >= 1.0);
        // NaN never, inf always
        if t.is_nan() {
            prop_assert!(!video_due(t, true));
        }
    }

    #[test]
    fn edge_video_stop_any_flags(audio: bool, active: bool) {
        let step = video_stop(audio, active);
        prop_assert_eq!(step == VideoStep::StopAudio, !active);
        prop_assert_eq!(step == VideoStep::End, active && audio);
    }

    #[test]
    fn edge_audio_stop_any_t(t in any_t(), has_ts: bool, video: bool, audio: bool) {
        let got = audio_stop(has_ts.then_some(t), video, audio);
        let ends = if has_ts { t >= 1.0 && audio } else { !video && audio };
        let want = if !ends {
            AudioStep::Keep
        } else if video {
            AudioStep::End
        } else {
            AudioStep::Stop
        };
        prop_assert_eq!(got, want);
        // the audio never ends while it is not transitioning
        if !audio {
            prop_assert_eq!(got, AudioStep::Keep);
        }
    }

    #[test]
    fn edge_audio_place_matches_exact_frames(
        min_ts: u64,
        diff in prop_oneof![any::<u64>(), 0_u64..50_000_000],
        rate in prop_oneof![0_u64..=1_000_000_000, Just(48_000), Just(44_100)],
        zero_ts: bool,
    ) {
        let ts = if zero_ts { 0 } else { min_ts.wrapping_add(diff) };
        let got = audio_place(ts, min_ts, usize::try_from(rate).unwrap());
        if ts == 0 {
            prop_assert_eq!(got, Place::None);
        } else {
            let frames = exact_frames(ts.wrapping_sub(min_ts), rate);
            // a usize as the C's (size_t) cast: the low bits
            let pos = usize::try_from(frames & u64::try_from(usize::MAX).unwrap()).unwrap();
            if pos > AUDIO_FRAMES {
                prop_assert_eq!(got, Place::Late(pos));
            } else {
                prop_assert_eq!(got, Place::Mix(pos));
            }
        }
    }

    #[test]
    fn edge_audio_place_grows_with_ts(
        min_ts in 1_u64..u64::MAX / 2,
        a in 0_u64..100_000_000,
        b in 0_u64..100_000_000,
        rate in 1_u64..=384_000,
    ) {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        let frame = |d: u64| match audio_place(min_ts + d, min_ts, usize::try_from(rate).unwrap()) {
            Place::Mix(p) | Place::Late(p) => p,
            Place::None => usize::MAX,
        };
        prop_assert!(frame(lo) <= frame(hi));
    }

    #[test]
    fn edge_offset_any_bits(
        tr_cx in any_size(),
        tr_cy in any_size(),
        source_cx in any_size(),
        source_cy in any_size(),
        scale_x in any_scale(),
        scale_y in any_scale(),
    ) {
        let got: Offset = offset(tr_cx, tr_cy, source_cx, source_cy, scale_x, scale_y);
        let x = model_offset(tr_cx, source_cx, scale_x);
        let y = model_offset(tr_cy, source_cy, scale_y);
        prop_assert!(same_f32(got.x, x), "x {:#010x} vs {:#010x}", got.x.to_bits(), x.to_bits());
        prop_assert!(same_f32(got.y, y), "y {:#010x} vs {:#010x}", got.y.to_bits(), y.to_bits());
    }
}
