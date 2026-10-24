//! Tier 1: `obs_output::video_checks`, the output video size, conversion
//! and capture checks.
//!
//! The `steps_*` cases port `test/cmocka/test_output_video_checks_steps.c`
//! 1:1: its 27 cases under the same names, in the same order, with the same
//! values and expectations, the "characterized, not endorsed" ones
//! included. The C calls the hidden `obs_output_*` functions; here the safe
//! core is called directly ([`first_mixer`], [`size_route`],
//! [`capture_ready`], [`begin_gate`], [`video_conversion`], [`congestion`]).
//!
//! The `edge_*` cases are new: every mixer mask of up to two bits, every
//! flag word over the low 8 bits against every presence, sizes at the `u32`
//! edges, the congestion's bit patterns around 0 and 1, and the
//! constants.

use obs_output::video_checks::{
    BeginGate, Conversion, ConversionSizes, MAX_AUDIO_MIXES, MAX_OUTPUT_AUDIO_ENCODERS,
    MAX_OUTPUT_VIDEO_ENCODERS, OBS_OUTPUT_AUDIO, OBS_OUTPUT_ENCODED, OBS_OUTPUT_SERVICE,
    OBS_OUTPUT_VIDEO, Presence, SizeRoute, begin_gate, capture_ready, congestion, first_mixer,
    size_route, video_conversion,
};

const FLAG_VIDEO: u32 = OBS_OUTPUT_VIDEO;
const FLAG_AUDIO: u32 = OBS_OUTPUT_AUDIO;
const FLAG_ENCODED: u32 = OBS_OUTPUT_ENCODED;
const FLAG_SERVICE: u32 = OBS_OUTPUT_SERVICE;
const SIZE_BITS: u32 = usize::BITS;

// ---------------------------------------------------------------------------
// first_mixer

#[test]
fn steps_first_mixer_zero() {
    assert_eq!(first_mixer(0), 0);
}

#[test]
fn steps_first_mixer_single_bits() {
    assert_eq!(MAX_AUDIO_MIXES, 6);
    for i in 0..SIZE_BITS {
        let want = if (i as usize) < MAX_AUDIO_MIXES {
            i as usize
        } else {
            0
        };
        assert_eq!(first_mixer(1usize << i), want, "bit {i}");
    }
}

#[test]
fn steps_first_mixer_several_bits() {
    assert_eq!(first_mixer(0x3), 0);
    assert_eq!(first_mixer(0x6), 1);
    assert_eq!(first_mixer(0x28), 3);
    assert_eq!(first_mixer(0x30), 4);
    assert_eq!(first_mixer(0x3f), 0);
    assert_eq!(first_mixer(0x3e), 1);
    assert_eq!(first_mixer(0x20), 5);
    assert_eq!(first_mixer(0x60), 5);
    assert_eq!(first_mixer(0x44), 2);
    assert_eq!(first_mixer((1usize << (SIZE_BITS - 1)) | 0x10), 4);
    assert_eq!(first_mixer(!0usize), 0);
    assert_eq!(first_mixer(!0x1fusize), 5);
    assert_eq!(first_mixer(!0x7usize), 3);
}

/// Characterized, not endorsed: bits only at or above `MAX_AUDIO_MIXES`
/// report mixer 0.
#[test]
fn steps_first_mixer_high_bits_only() {
    assert_eq!(first_mixer(0x40), 0);
    assert_eq!(first_mixer(0x80), 0);
    assert_eq!(first_mixer(0xffc0), 0);
    assert_eq!(first_mixer(!0x3fusize), 0);
    assert_eq!(first_mixer(1usize << 31), 0);
    assert_eq!(first_mixer(1usize << (SIZE_BITS - 1)), 0);
}

// ---------------------------------------------------------------------------
// size_route

#[test]
fn steps_size_route_encoded() {
    for scaled in [0, 1, 1280, u32::MAX] {
        assert_eq!(size_route(true, true, scaled), SizeRoute::Encoder);
        assert_eq!(size_route(true, false, scaled), SizeRoute::Zero);
    }
}

#[test]
fn steps_size_route_raw() {
    for scaled in [1, 2, 720, 1280, 0x8000_0000, u32::MAX] {
        assert_eq!(size_route(false, false, scaled), SizeRoute::Scaled);
        assert_eq!(size_route(false, true, scaled), SizeRoute::Scaled);
    }
    assert_eq!(size_route(false, false, 0), SizeRoute::Video);
    assert_eq!(size_route(false, true, 0), SizeRoute::Video);
}

// ---------------------------------------------------------------------------
// capture_ready

fn presence(
    flags: u32,
    video_encoders: u32,
    audio_encoders: u32,
    video: bool,
    audio: bool,
    service: bool,
) -> Presence {
    Presence {
        flags,
        video_encoders,
        audio_encoders,
        video,
        audio,
        service,
    }
}

fn ready(
    flags: u32,
    video_encoders: u32,
    audio_encoders: u32,
    video: bool,
    audio: bool,
    service: bool,
) -> bool {
    capture_ready(&presence(
        flags,
        video_encoders,
        audio_encoders,
        video,
        audio,
        service,
    ))
}

#[test]
fn steps_capture_no_flags() {
    assert!(ready(0, 0, 0, false, false, false));
    assert!(ready(0, 0x3ff, 0x3f, true, true, true));
    assert!(ready(FLAG_ENCODED, 0, 0, false, false, false));
}

#[test]
fn steps_capture_raw_video() {
    assert!(ready(FLAG_VIDEO, 0, 0, true, false, false));
    assert!(!ready(FLAG_VIDEO, 0, 0, false, false, false));
    assert!(!ready(FLAG_VIDEO, 0x3ff, 0x3f, false, true, true));
    assert!(ready(FLAG_VIDEO, 0, 0, true, false, true));
}

#[test]
fn steps_capture_raw_audio() {
    assert!(ready(FLAG_AUDIO, 0, 0, false, true, false));
    assert!(!ready(FLAG_AUDIO, 0, 0, false, false, false));
    assert!(!ready(FLAG_AUDIO, 0x3ff, 0x3f, true, false, true));
}

#[test]
fn steps_capture_raw_av() {
    let av = FLAG_VIDEO | FLAG_AUDIO;
    assert!(ready(av, 0, 0, true, true, false));
    assert!(!ready(av, 0, 0, true, false, false));
    assert!(!ready(av, 0, 0, false, true, false));
    assert!(!ready(av, 0, 0, false, false, false));
}

#[test]
fn steps_capture_encoded_video() {
    let video = FLAG_VIDEO | FLAG_ENCODED;
    assert_eq!(MAX_OUTPUT_VIDEO_ENCODERS, 10);
    for i in 0..32 {
        let want = i < MAX_OUTPUT_VIDEO_ENCODERS;
        assert_eq!(
            ready(video, 1 << i, 0, false, false, false),
            want,
            "video encoder bit {i}"
        );
    }
    assert!(!ready(video, 0, 0, false, false, false));
    assert!(ready(video, 0x3ff, 0, false, false, false));
    assert!(ready(video, 0x200, 0, false, false, false));
    assert!(!ready(video, 0, 0x3f, true, true, true));
}

#[test]
fn steps_capture_encoded_audio() {
    let audio = FLAG_AUDIO | FLAG_ENCODED;
    assert_eq!(MAX_OUTPUT_AUDIO_ENCODERS, 6);
    for i in 0..32 {
        let want = i < MAX_OUTPUT_AUDIO_ENCODERS;
        assert_eq!(
            ready(audio, 0, 1 << i, false, false, false),
            want,
            "audio encoder bit {i}"
        );
    }
    assert!(!ready(audio, 0, 0, false, false, false));
    assert!(ready(audio, 0, 0x3f, false, false, false));
    assert!(ready(audio, 0, 0x20, false, false, false));
    assert!(!ready(audio, 0x3ff, 0, true, true, true));
}

/// Characterized: an encoder bit at or above the limit is not an encoder.
#[test]
fn steps_capture_encoder_bits_past_the_limit() {
    assert!(!ready(
        FLAG_VIDEO | FLAG_ENCODED,
        !0x3ff,
        0,
        true,
        true,
        true
    ));
    assert!(!ready(
        FLAG_AUDIO | FLAG_ENCODED,
        0,
        !0x3f,
        true,
        true,
        true
    ));
    assert!(ready(
        FLAG_VIDEO | FLAG_ENCODED,
        !0x1ff,
        0,
        false,
        false,
        false
    ));
    assert!(ready(
        FLAG_AUDIO | FLAG_ENCODED,
        0,
        !0x1f,
        false,
        false,
        false
    ));
}

#[test]
fn steps_capture_service() {
    assert!(ready(FLAG_SERVICE, 0, 0, false, false, true));
    assert!(!ready(FLAG_SERVICE, 0, 0, false, false, false));
    assert!(!ready(
        FLAG_SERVICE | FLAG_ENCODED,
        0x3ff,
        0x3f,
        true,
        true,
        false
    ));
    let stream = FLAG_VIDEO | FLAG_AUDIO | FLAG_ENCODED | FLAG_SERVICE;
    assert!(ready(stream, 0x1, 0x1, false, false, true));
    assert!(!ready(stream, 0x1, 0x1, true, true, false));
    assert!(!ready(stream, 0x0, 0x1, true, true, true));
    assert!(!ready(stream, 0x1, 0x0, true, true, true));
}

#[test]
fn steps_capture_other_flags_ignored() {
    let others = !(FLAG_VIDEO | FLAG_AUDIO | FLAG_ENCODED | FLAG_SERVICE);
    assert!(ready(others, 0, 0, false, false, false));
    assert!(ready(others | FLAG_VIDEO, 0, 0, true, false, false));
    assert!(!ready(others | FLAG_VIDEO, 0, 0, false, false, false));
    assert!(ready(
        others | FLAG_VIDEO | FLAG_ENCODED,
        0x1,
        0,
        false,
        false,
        false
    ));
}

/// The test's model: each wanted thing must be there.
fn model_ready(p: &Presence) -> bool {
    let encoded = p.flags & FLAG_ENCODED != 0;
    let has_video = if encoded {
        p.video_encoders & 0x3ff != 0
    } else {
        p.video
    };
    let has_audio = if encoded {
        p.audio_encoders & 0x3f != 0
    } else {
        p.audio
    };
    !((p.flags & FLAG_VIDEO != 0 && !has_video)
        || (p.flags & FLAG_AUDIO != 0 && !has_audio)
        || (p.flags & FLAG_SERVICE != 0 && !p.service))
}

fn video_masks() -> Vec<u32> {
    let mut masks = vec![0];
    masks.extend((0..MAX_OUTPUT_VIDEO_ENCODERS).map(|i| 1u32 << i));
    masks.push(1 << MAX_OUTPUT_VIDEO_ENCODERS);
    masks.push(1 << 31);
    masks
}

fn audio_masks() -> Vec<u32> {
    let mut masks = vec![0];
    masks.extend((0..MAX_OUTPUT_AUDIO_ENCODERS).map(|i| 1u32 << i));
    masks.push(1 << MAX_OUTPUT_AUDIO_ENCODERS);
    masks.push(1 << 31);
    masks
}

#[test]
fn steps_capture_every_flag_against_every_presence() {
    let (vm, am) = (video_masks(), audio_masks());
    let (mut checked, mut true_count) = (0usize, 0usize);
    for flags in 0..16u32 {
        for &v in &vm {
            for &a in &am {
                for bits in 0..8u32 {
                    let p = presence(flags, v, a, bits & 1 != 0, bits & 2 != 0, bits & 4 != 0);
                    let got = capture_ready(&p);
                    assert_eq!(got, model_ready(&p), "{p:?}");
                    checked += 1;
                    true_count += usize::from(got);
                }
            }
        }
    }
    assert_eq!(checked, 16 * 13 * 9 * 8);
    assert!(true_count > 1000);
    assert!(checked - true_count > 1000);
}

// ---------------------------------------------------------------------------
// begin_gate

#[test]
fn steps_begin_gate() {
    assert_eq!(begin_gate(true, false), BeginGate::Ready);
    assert_eq!(begin_gate(true, true), BeginGate::Ready);
    assert_eq!(begin_gate(false, true), BeginGate::Refuse);
    assert_eq!(begin_gate(false, false), BeginGate::Check);
}

// ---------------------------------------------------------------------------
// video_conversion

fn plan(
    set: bool,
    set_width: u32,
    set_height: u32,
    scaled_width: u32,
    scaled_height: u32,
    video_width: u32,
    video_height: u32,
) -> Conversion {
    video_conversion(&ConversionSizes {
        set,
        set_width,
        set_height,
        scaled_width,
        scaled_height,
        video_width,
        video_height,
    })
}

const NONE: Conversion = Conversion::None;

fn set(width: u32, height: u32) -> Conversion {
    Conversion::UseSet { width, height }
}

fn scaled(width: u32, height: u32) -> Conversion {
    Conversion::Scaled { width, height }
}

#[test]
fn steps_conversion_not_set() {
    assert_eq!(plan(false, 0, 0, 0, 0, 1920, 1080), NONE);
    assert_eq!(plan(false, 0, 0, 1920, 1080, 1920, 1080), NONE);
    assert_eq!(plan(false, 0, 0, 1280, 720, 1920, 1080), scaled(1280, 720));
    assert_eq!(
        plan(false, 0, 0, 3840, 2160, 1920, 1080),
        scaled(3840, 2160)
    );
    assert_eq!(
        plan(false, 0, 0, 1920, 1081, 1920, 1080),
        scaled(1920, 1081)
    );
    assert_eq!(
        plan(false, 0, 0, 1919, 1080, 1920, 1080),
        scaled(1919, 1080)
    );
    assert_eq!(
        plan(false, 640, 360, 1280, 720, 1920, 1080),
        scaled(1280, 720)
    );
    assert_eq!(plan(false, 640, 360, 0, 0, 1920, 1080), NONE);
}

#[test]
fn steps_conversion_scaled_one_axis_zero() {
    assert_eq!(plan(false, 0, 0, 1280, 0, 1920, 1080), NONE);
    assert_eq!(plan(false, 0, 0, 0, 720, 1920, 1080), NONE);
}

#[test]
fn steps_conversion_no_video() {
    assert_eq!(plan(false, 0, 0, 1280, 720, 0, 0), scaled(1280, 720));
    assert_eq!(plan(false, 0, 0, 0, 0, 0, 0), NONE);
}

#[test]
fn steps_conversion_set() {
    assert_eq!(plan(true, 640, 360, 0, 0, 1920, 1080), set(640, 360));
    assert_eq!(plan(true, 640, 360, 1280, 720, 1920, 1080), set(640, 360));
    assert_eq!(plan(true, 640, 360, 1920, 1080, 1920, 1080), set(640, 360));
    assert_eq!(
        plan(true, 1920, 1080, 1920, 1080, 1920, 1080),
        set(1920, 1080)
    );
    assert_eq!(plan(true, u32::MAX, 1, 0, 0, 0, 0), set(u32::MAX, 1));
}

#[test]
fn steps_conversion_set_zero_filled() {
    assert_eq!(plan(true, 0, 0, 0, 0, 1920, 1080), set(1920, 1080));
    assert_eq!(plan(true, 0, 0, 1280, 720, 1920, 1080), set(1280, 720));
    assert_eq!(plan(true, 0, 360, 1280, 720, 1920, 1080), set(1280, 360));
    assert_eq!(plan(true, 640, 0, 1280, 720, 1920, 1080), set(640, 720));
    assert_eq!(plan(true, 0, 360, 0, 0, 1920, 1080), set(1920, 360));
    assert_eq!(plan(true, 640, 0, 0, 0, 1920, 1080), set(640, 1080));
    assert_eq!(plan(true, 0, 0, 1280, 0, 1920, 1080), set(1280, 1080));
    assert_eq!(plan(true, 0, 0, 0, 720, 1920, 1080), set(1920, 720));
}

/// Characterized, not endorsed: a 0 axis with no scaled size and no video
/// stays 0, and the set conversion is still returned.
#[test]
fn steps_conversion_set_zero_without_video() {
    assert_eq!(plan(true, 0, 0, 0, 0, 0, 0), set(0, 0));
    assert_eq!(plan(true, 640, 0, 0, 0, 0, 0), set(640, 0));
}

// ---------------------------------------------------------------------------
// congestion

/// Bit for bit: -0 is not +0, and a NaN keeps its payload.
fn expect_clamp(input: f32, want: f32) {
    let got = congestion(input);
    assert!(
        got.to_bits() == want.to_bits(),
        "clamp({input} [{:#010x}]): {got} [{:#010x}], expected {want} [{:#010x}]",
        input.to_bits(),
        got.to_bits(),
        want.to_bits()
    );
}

#[test]
fn steps_congestion_range() {
    expect_clamp(-1.0, 0.0);
    expect_clamp(0.0, 0.0);
    expect_clamp(0.5, 0.5);
    expect_clamp(1.0, 1.0);
    expect_clamp(2.0, 1.0);
    expect_clamp(0.25, 0.25);
    expect_clamp(f32::from_bits(0x3f7f_ffff), f32::from_bits(0x3f7f_ffff));
    expect_clamp(f32::from_bits(0x3f80_0001), 1.0);
    expect_clamp(f32::from_bits(1), f32::from_bits(1));
    expect_clamp(f32::from_bits(0x8000_0001), 0.0);
    expect_clamp(f32::MIN, 0.0);
    expect_clamp(f32::MAX, 1.0);
}

#[test]
fn steps_congestion_infinities() {
    expect_clamp(f32::INFINITY, 1.0);
    expect_clamp(f32::NEG_INFINITY, 0.0);
}

#[test]
fn steps_congestion_negative_zero() {
    expect_clamp(-0.0, -0.0);
    assert_eq!(congestion(-0.0).to_bits(), 0x8000_0000);
}

/// Characterized, not endorsed: a NaN passes through, payload and sign
/// kept.
#[test]
fn steps_congestion_nan() {
    assert!(congestion(f32::NAN).is_nan());
    for bits in [
        0x7fc0_0000u32,
        0xffc0_0000,
        0x7fc0_0001,
        0x7fff_ffff,
        0xffff_ffff,
    ] {
        assert_eq!(congestion(f32::from_bits(bits)).to_bits(), bits);
    }
}

// ---------------------------------------------------------------------------
// edges

/// The C's loop over the mixers, as written.
fn model_first_mixer(mask: usize) -> usize {
    for i in 0..MAX_AUDIO_MIXES {
        if (1usize << i) & mask != 0 {
            return i;
        }
    }
    0
}

/// Every mask of up to two set bits, and each with every low 6-bit pattern
/// or'ed in.
#[test]
fn edge_first_mixer_two_bits_and_low_patterns() {
    for i in 0..SIZE_BITS {
        for j in 0..SIZE_BITS {
            let mask = (1usize << i) | (1usize << j);
            assert_eq!(first_mixer(mask), model_first_mixer(mask), "{mask:#x}");
            for low in 0..64usize {
                let m = mask | low;
                assert_eq!(first_mixer(m), model_first_mixer(m), "{m:#x}");
                let m = (mask & !0x3f) | low;
                assert_eq!(first_mixer(m), model_first_mixer(m), "{m:#x}");
            }
        }
    }
}

/// Every flag word over the low 8 bits (the four checked flags and
/// `MULTI_TRACK`, `CAN_PAUSE`, `MULTI_TRACK_VIDEO` and one unknown bit),
/// with and without the high bits, against every presence.
#[test]
fn edge_capture_every_low_flag_word() {
    let (vm, am) = (video_masks(), audio_masks());
    for low in 0..256u32 {
        for flags in [low, low | 0xffff_ff00] {
            for &v in &vm {
                for &a in &am {
                    for bits in 0..8u32 {
                        let p = presence(flags, v, a, bits & 1 != 0, bits & 2 != 0, bits & 4 != 0);
                        assert_eq!(capture_ready(&p), model_ready(&p), "{p:?}");
                    }
                }
            }
        }
    }
}

/// The encoder masks with many bits: every 10-bit video and 6-bit audio
/// pattern, alone and with the bits past the limits set.
#[test]
fn edge_capture_every_encoder_pattern() {
    let flags = FLAG_VIDEO | FLAG_AUDIO | FLAG_ENCODED;
    for v in 0..1024u32 {
        for a in 0..64u32 {
            for (vh, ah) in [(0, 0), (!0x3ff, !0x3f)] {
                let p = presence(flags, v | vh, a | ah, false, false, false);
                assert_eq!(capture_ready(&p), v != 0 && a != 0, "{p:?}");
            }
        }
    }
}

/// The routes are not changed by the scaled size's value, only by whether
/// it is 0.
#[test]
fn edge_size_route_every_shape() {
    for scaled in [0u32, 1, 0x7fff_ffff, 0x8000_0000, u32::MAX - 1, u32::MAX] {
        for encoded in [false, true] {
            for has_encoder in [false, true] {
                let want = match (encoded, has_encoder, scaled != 0) {
                    (true, true, _) => SizeRoute::Encoder,
                    (true, false, _) => SizeRoute::Zero,
                    (false, _, true) => SizeRoute::Scaled,
                    (false, _, false) => SizeRoute::Video,
                };
                assert_eq!(size_route(encoded, has_encoder, scaled), want);
            }
        }
    }
}

/// The C's video conversion choice, as written.
fn model_conversion(s: &ConversionSizes) -> Conversion {
    if s.set {
        let w = if s.set_width == 0 {
            if s.scaled_width != 0 {
                s.scaled_width
            } else {
                s.video_width
            }
        } else {
            s.set_width
        };
        let h = if s.set_height == 0 {
            if s.scaled_height != 0 {
                s.scaled_height
            } else {
                s.video_height
            }
        } else {
            s.set_height
        };
        Conversion::UseSet {
            width: w,
            height: h,
        }
    } else if s.scaled_width != 0
        && s.scaled_height != 0
        && (s.video_width != s.scaled_width || s.video_height != s.scaled_height)
    {
        Conversion::Scaled {
            width: s.scaled_width,
            height: s.scaled_height,
        }
    } else {
        Conversion::None
    }
}

/// Every combination of 0, 1, 2 and `u32::MAX` on all six sizes, set and
/// not set.
#[test]
fn edge_conversion_every_small_shape() {
    let values = [0u32, 1, 2, u32::MAX];
    let mut count = 0usize;
    for set in [false, true] {
        for i in 0..4usize.pow(6) {
            let pick = |k: u32| values[(i / 4usize.pow(k)) % 4];
            let s = ConversionSizes {
                set,
                set_width: pick(0),
                set_height: pick(1),
                scaled_width: pick(2),
                scaled_height: pick(3),
                video_width: pick(4),
                video_height: pick(5),
            };
            assert_eq!(video_conversion(&s), model_conversion(&s), "{s:?}");
            count += 1;
        }
    }
    assert_eq!(count, 2 * 4096);
}

/// The floats within 256 bit patterns of 0, -0, 1 and -1, and the
/// infinities and NaNs.
#[test]
// The model is the C's two comparisons, written out as the C has them.
#[allow(clippy::manual_clamp)]
fn edge_congestion_bits_near_the_edges() {
    let model = |v: f32| {
        if v < 0.0 {
            0.0
        } else if v > 1.0 {
            1.0
        } else {
            v
        }
    };
    for center in [
        0x0000_0000u32,
        0x8000_0000,
        0x3f80_0000,
        0xbf80_0000,
        0x7f80_0000,
        0xff80_0000,
        0x7fc0_0000,
        0xffc0_0000,
    ] {
        for d in -256i32..=256 {
            let Some(b) = center.checked_add_signed(d) else {
                continue;
            };
            let v = f32::from_bits(b);
            let want = model(v);
            assert_eq!(congestion(v).to_bits(), want.to_bits(), "{v} [{b:#010x}]");
            // Never outside [0, 1] except NaN and -0.
            let got = congestion(v);
            assert!(got.is_nan() || (0.0..=1.0).contains(&got), "{got}");
        }
    }
}

#[test]
fn edge_constants() {
    assert_eq!(OBS_OUTPUT_VIDEO, 1);
    assert_eq!(OBS_OUTPUT_AUDIO, 2);
    assert_eq!(OBS_OUTPUT_ENCODED, 4);
    assert_eq!(OBS_OUTPUT_SERVICE, 8);
    assert_eq!(MAX_AUDIO_MIXES, 6);
    assert_eq!(MAX_OUTPUT_VIDEO_ENCODERS, 10);
    assert_eq!(MAX_OUTPUT_AUDIO_ENCODERS, 6);
}
