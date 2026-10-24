//! Tier 2 layout and Tier 3 parity: the shims of
//! `obs_output::ffi::output_video_checks` (and the safe core under them)
//! against the original `libobs/obs-output-video-checks.c.h`, compiled as
//! `oracle_obs_output_*` (obs-c-oracle, oracle/output_video_checks.c).
//!
//! Every output is compared exactly: the mixer index, the size route and
//! the begin action as their values, the capture check as a bool, the
//! conversion choice field by field, and the congestion as `f32` bits, so
//! -0 against +0 or a NaN payload would fail.
//!
//! Runs: the constants and the layout of the three structs; every mixer
//! mask of up to two bits with each low 6-bit pattern; every route over
//! special scaled sizes; the capture check exhaustively over the four
//! flags, every video encoder mask of 10 bits, every audio encoder mask of
//! 6 bits and the video, audio and service bits (8,388,608 calls), and over
//! every flag word of the low 8 bits with the bits past the limits; the
//! four begin gates; every conversion over the sizes 0, 1, 2 and
//! `u32::MAX`, set or not; the congestion over the bit patterns near 0, 1,
//! -1, the infinities and the NaNs, and every 65521st of all 2^32; and
//! proptests over full-range `usize` mixer masks, `u32` flags, masks and
//! sizes (biased to equal scaled and video sizes and to 0), and `f32`
//! congestions drawn as bit patterns.
//!
//! No input is excluded. No intentional differences.
//! `test_output_video_checks_steps` runs the C test against the C; the
//! real-libobs output tests run against the hybrid libobs, where these
//! shims replace the C.

use core::ffi::c_void;
use std::mem::{align_of, offset_of, size_of};

use obs_c_oracle::output_video_checks::{
    CONSTANT_NAMES, ConversionChoice, LAYOUT_NAMES, oracle_obs_output_begin_gate,
    oracle_obs_output_capture_ready, oracle_obs_output_congestion_clamp,
    oracle_obs_output_conversion_plan, oracle_obs_output_first_mixer, oracle_obs_output_size_route,
    oracle_output_video_checks_constant, oracle_output_video_checks_layout,
};
use obs_output::ffi::output_video_checks::{
    OBS_OUTPUT_BEGIN_CHECK, OBS_OUTPUT_BEGIN_READY, OBS_OUTPUT_BEGIN_REFUSE,
    OBS_OUTPUT_CONVERSION_NONE, OBS_OUTPUT_CONVERSION_SCALED, OBS_OUTPUT_CONVERSION_SET,
    OBS_OUTPUT_SIZE_ENCODER, OBS_OUTPUT_SIZE_SCALED, OBS_OUTPUT_SIZE_VIDEO, OBS_OUTPUT_SIZE_ZERO,
    obs_output_begin_gate, obs_output_capture_presence, obs_output_capture_ready,
    obs_output_congestion_clamp, obs_output_conversion_choice, obs_output_conversion_plan,
    obs_output_conversion_sizes, obs_output_first_mixer, obs_output_size_route,
};
use obs_output::video_checks::{
    self as checks, BeginGate, Conversion, ConversionSizes, MAX_AUDIO_MIXES,
    MAX_OUTPUT_AUDIO_ENCODERS, MAX_OUTPUT_VIDEO_ENCODERS, OBS_OUTPUT_AUDIO, OBS_OUTPUT_ENCODED,
    OBS_OUTPUT_SERVICE, OBS_OUTPUT_VIDEO, Presence, SizeRoute,
};
use proptest::prelude::*;

fn route_value(route: SizeRoute) -> u32 {
    match route {
        SizeRoute::Zero => OBS_OUTPUT_SIZE_ZERO,
        SizeRoute::Encoder => OBS_OUTPUT_SIZE_ENCODER,
        SizeRoute::Scaled => OBS_OUTPUT_SIZE_SCALED,
        SizeRoute::Video => OBS_OUTPUT_SIZE_VIDEO,
    }
}

fn gate_value(gate: BeginGate) -> u32 {
    match gate {
        BeginGate::Ready => OBS_OUTPUT_BEGIN_READY,
        BeginGate::Refuse => OBS_OUTPUT_BEGIN_REFUSE,
        BeginGate::Check => OBS_OUTPUT_BEGIN_CHECK,
    }
}

fn conversion_value(c: Conversion) -> ConversionChoice {
    match c {
        Conversion::None => ConversionChoice {
            kind: OBS_OUTPUT_CONVERSION_NONE,
            width: 0,
            height: 0,
        },
        Conversion::UseSet { width, height } => ConversionChoice {
            kind: OBS_OUTPUT_CONVERSION_SET,
            width,
            height,
        },
        Conversion::Scaled { width, height } => ConversionChoice {
            kind: OBS_OUTPUT_CONVERSION_SCALED,
            width,
            height,
        },
    }
}

fn check_first_mixer(mask: usize) -> usize {
    let rust = obs_output_first_mixer(mask);
    // SAFETY: a plain value function.
    let c = unsafe { oracle_obs_output_first_mixer(mask) };
    assert_eq!(rust, c, "first_mixer({mask:#x})");
    assert_eq!(checks::first_mixer(mask), c, "core first_mixer({mask:#x})");
    c
}

fn check_size_route(encoded: bool, has_encoder: bool, scaled: u32) -> u32 {
    let rust = obs_output_size_route(encoded, has_encoder, scaled);
    // SAFETY: a plain value function.
    let c = unsafe { oracle_obs_output_size_route(encoded, has_encoder, scaled) };
    let what = format!("size_route({encoded}, {has_encoder}, {scaled})");
    assert_eq!(rust, c, "{what}");
    assert_eq!(
        route_value(checks::size_route(encoded, has_encoder, scaled)),
        c,
        "core {what}"
    );
    c
}

fn check_capture(p: obs_output_capture_presence) -> bool {
    // SAFETY: a live presence, read during the call.
    let rust = unsafe { obs_output_capture_ready(&raw const p) };
    // SAFETY: as above; the oracle reads it as struct
    // obs_output_capture_presence, whose layout layout_matches_c checks.
    let c = unsafe { oracle_obs_output_capture_ready((&raw const p).cast::<c_void>()) };
    assert_eq!(rust, c, "capture_ready({p:?})");
    let core = checks::capture_ready(&Presence {
        flags: p.flags,
        video_encoders: p.video_encoders,
        audio_encoders: p.audio_encoders,
        video: p.video,
        audio: p.audio,
        service: p.service,
    });
    assert_eq!(core, c, "core capture_ready({p:?})");
    c
}

fn check_gate(delay_active: bool, active: bool) -> u32 {
    let rust = obs_output_begin_gate(delay_active, active);
    // SAFETY: a plain value function.
    let c = unsafe { oracle_obs_output_begin_gate(delay_active, active) };
    assert_eq!(rust, c, "begin_gate({delay_active}, {active})");
    assert_eq!(
        gate_value(checks::begin_gate(delay_active, active)),
        c,
        "core"
    );
    c
}

fn check_conversion(s: obs_output_conversion_sizes) -> ConversionChoice {
    // SAFETY: live sizes, read during the call.
    let rust: obs_output_conversion_choice = unsafe { obs_output_conversion_plan(&raw const s) };
    // SAFETY: as above; the oracle reads them as struct
    // obs_output_conversion_sizes.
    let c = unsafe { oracle_obs_output_conversion_plan((&raw const s).cast::<c_void>()) };
    let rust = ConversionChoice {
        kind: rust.kind,
        width: rust.width,
        height: rust.height,
    };
    assert_eq!(rust, c, "conversion_plan({s:?})");
    let core = conversion_value(checks::video_conversion(&ConversionSizes {
        set: s.set,
        set_width: s.set_width,
        set_height: s.set_height,
        scaled_width: s.scaled_width,
        scaled_height: s.scaled_height,
        video_width: s.video_width,
        video_height: s.video_height,
    }));
    assert_eq!(core, c, "core conversion_plan({s:?})");
    c
}

fn check_congestion(v: f32) -> u32 {
    let rust = obs_output_congestion_clamp(v);
    // SAFETY: a plain value function.
    let c = unsafe { oracle_obs_output_congestion_clamp(v) };
    let what = || format!("congestion_clamp({v} [{:#010x}])", v.to_bits());
    assert_eq!(rust.to_bits(), c.to_bits(), "{}", what());
    assert_eq!(
        checks::congestion(v).to_bits(),
        c.to_bits(),
        "core {}",
        what()
    );
    c.to_bits()
}

fn presence(
    flags: u32,
    video_encoders: u32,
    audio_encoders: u32,
    bits: u32,
) -> obs_output_capture_presence {
    obs_output_capture_presence {
        flags,
        video_encoders,
        audio_encoders,
        video: bits & 1 != 0,
        audio: bits & 2 != 0,
        service: bits & 4 != 0,
    }
}

#[allow(clippy::too_many_arguments)]
fn sizes(
    set: bool,
    set_width: u32,
    set_height: u32,
    scaled_width: u32,
    scaled_height: u32,
    video_width: u32,
    video_height: u32,
) -> obs_output_conversion_sizes {
    obs_output_conversion_sizes {
        set,
        set_width,
        set_height,
        scaled_width,
        scaled_height,
        video_width,
        video_height,
    }
}

#[test]
fn layout_matches_c() {
    let rust = [
        size_of::<obs_output_capture_presence>(),
        align_of::<obs_output_capture_presence>(),
        offset_of!(obs_output_capture_presence, flags),
        offset_of!(obs_output_capture_presence, video_encoders),
        offset_of!(obs_output_capture_presence, audio_encoders),
        offset_of!(obs_output_capture_presence, video),
        offset_of!(obs_output_capture_presence, audio),
        offset_of!(obs_output_capture_presence, service),
        size_of::<obs_output_conversion_sizes>(),
        align_of::<obs_output_conversion_sizes>(),
        offset_of!(obs_output_conversion_sizes, set),
        offset_of!(obs_output_conversion_sizes, set_width),
        offset_of!(obs_output_conversion_sizes, set_height),
        offset_of!(obs_output_conversion_sizes, scaled_width),
        offset_of!(obs_output_conversion_sizes, scaled_height),
        offset_of!(obs_output_conversion_sizes, video_width),
        offset_of!(obs_output_conversion_sizes, video_height),
        size_of::<obs_output_conversion_choice>(),
        align_of::<obs_output_conversion_choice>(),
        offset_of!(obs_output_conversion_choice, kind),
        offset_of!(obs_output_conversion_choice, width),
        offset_of!(obs_output_conversion_choice, height),
        size_of::<u32>(),
    ];
    for (i, (name, rust)) in LAYOUT_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: a plain value lookup.
        let c = unsafe { oracle_output_video_checks_layout(i) };
        assert_eq!(rust, c, "{name}");
    }
    // SAFETY: as above.
    assert_eq!(
        unsafe { oracle_output_video_checks_layout(LAYOUT_NAMES.len()) },
        usize::MAX
    );
    // The oracle's mirror of the returned struct has the same layout.
    assert_eq!(
        size_of::<ConversionChoice>(),
        size_of::<obs_output_conversion_choice>()
    );
    assert_eq!(
        offset_of!(ConversionChoice, height),
        offset_of!(obs_output_conversion_choice, height)
    );
}

#[test]
fn constants_match_c() {
    let rust: [i64; 17] = [
        i64::from(OBS_OUTPUT_VIDEO),
        i64::from(OBS_OUTPUT_AUDIO),
        i64::from(OBS_OUTPUT_ENCODED),
        i64::from(OBS_OUTPUT_SERVICE),
        i64::try_from(MAX_AUDIO_MIXES).expect("small"),
        i64::from(MAX_OUTPUT_VIDEO_ENCODERS),
        i64::from(MAX_OUTPUT_AUDIO_ENCODERS),
        i64::from(OBS_OUTPUT_SIZE_ZERO),
        i64::from(OBS_OUTPUT_SIZE_ENCODER),
        i64::from(OBS_OUTPUT_SIZE_SCALED),
        i64::from(OBS_OUTPUT_SIZE_VIDEO),
        i64::from(OBS_OUTPUT_BEGIN_READY),
        i64::from(OBS_OUTPUT_BEGIN_REFUSE),
        i64::from(OBS_OUTPUT_BEGIN_CHECK),
        i64::from(OBS_OUTPUT_CONVERSION_NONE),
        i64::from(OBS_OUTPUT_CONVERSION_SET),
        i64::from(OBS_OUTPUT_CONVERSION_SCALED),
    ];
    for (i, (name, rust)) in CONSTANT_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: a plain value lookup.
        let c = unsafe { oracle_output_video_checks_constant(i) };
        assert_eq!(rust, c, "{name}");
    }
    // SAFETY: as above.
    assert_eq!(
        unsafe { oracle_output_video_checks_constant(CONSTANT_NAMES.len()) },
        -1
    );
}

/// Every mask of up to two set bits, alone, with each low 6-bit pattern
/// or'ed in, and with its low 6 bits replaced by each pattern.
#[test]
fn first_mixer_masks_match() {
    check_first_mixer(0);
    for i in 0..usize::BITS {
        for j in 0..usize::BITS {
            let mask = (1usize << i) | (1usize << j);
            check_first_mixer(mask);
            for low in 0..64usize {
                check_first_mixer(mask | low);
                check_first_mixer((mask & !0x3f) | low);
            }
        }
    }
    check_first_mixer(usize::MAX);
}

#[test]
fn size_routes_match() {
    let mut seen = [false; 4];
    for scaled in [0u32, 1, 2, 720, 1080, 0x7fff_ffff, 0x8000_0000, u32::MAX] {
        for encoded in [false, true] {
            for has_encoder in [false, true] {
                let r = check_size_route(encoded, has_encoder, scaled);
                seen[usize::try_from(r).expect("small")] = true;
            }
        }
    }
    assert_eq!(seen, [true; 4]);
}

/// Every combination of the four checked flags, every video encoder mask
/// below the limit, every audio encoder mask below the limit, and the
/// video, audio and service bits.
#[test]
fn capture_exhaustive_matches() {
    let (mut checked, mut ready) = (0u64, 0u64);
    for flags in 0..16u32 {
        for v in 0..(1u32 << MAX_OUTPUT_VIDEO_ENCODERS) {
            for a in 0..(1u32 << MAX_OUTPUT_AUDIO_ENCODERS) {
                for bits in 0..8u32 {
                    ready += u64::from(check_capture(presence(flags, v, a, bits)));
                    checked += 1;
                }
            }
        }
    }
    assert_eq!(checked, 16 * 1024 * 64 * 8);
    assert!(ready > 0 && ready < checked);
}

/// Every flag word over the low 8 bits, with and without every high bit,
/// against the encoder masks with bits past the limits (alone, below and
/// both), and every presence bit.
#[test]
fn capture_flag_words_and_high_bits_match() {
    let video_masks = [
        0u32,
        1,
        1 << 9,
        1 << 10,
        1 << 31,
        !0x3ff,
        !0x1ff,
        0x3ff,
        u32::MAX,
    ];
    let audio_masks = [
        0u32,
        1,
        1 << 5,
        1 << 6,
        1 << 31,
        !0x3f,
        !0x1f,
        0x3f,
        u32::MAX,
    ];
    for low in 0..256u32 {
        for flags in [low, low | 0xffff_ff00, low | 0x8000_0000] {
            for v in video_masks {
                for a in audio_masks {
                    for bits in 0..8u32 {
                        check_capture(presence(flags, v, a, bits));
                    }
                }
            }
        }
    }
}

#[test]
fn begin_gates_match() {
    assert_eq!(check_gate(true, false), OBS_OUTPUT_BEGIN_READY);
    assert_eq!(check_gate(true, true), OBS_OUTPUT_BEGIN_READY);
    assert_eq!(check_gate(false, true), OBS_OUTPUT_BEGIN_REFUSE);
    assert_eq!(check_gate(false, false), OBS_OUTPUT_BEGIN_CHECK);
}

/// Every conversion over the sizes 0, 1, 2 and `u32::MAX` on all six, set
/// and not set, plus the `test_output_video_checks_steps` values.
#[test]
fn conversions_match() {
    let values = [0u32, 1, 2, u32::MAX];
    let mut kinds = [0usize; 3];
    for set in [false, true] {
        for i in 0..4usize.pow(6) {
            let pick = |k: u32| values[(i / 4usize.pow(k)) % 4];
            let c = check_conversion(sizes(
                set,
                pick(0),
                pick(1),
                pick(2),
                pick(3),
                pick(4),
                pick(5),
            ));
            kinds[usize::try_from(c.kind).expect("small")] += 1;
        }
    }
    assert!(kinds.iter().all(|&n| n > 100), "{kinds:?}");
    for (set, sw, sh, cw, ch, vw, vh) in [
        (false, 0, 0, 0, 0, 1920, 1080),
        (false, 0, 0, 1920, 1080, 1920, 1080),
        (false, 0, 0, 1280, 720, 1920, 1080),
        (false, 0, 0, 1920, 1081, 1920, 1080),
        (false, 0, 0, 1919, 1080, 1920, 1080),
        (false, 640, 360, 1280, 720, 1920, 1080),
        (false, 0, 0, 1280, 0, 1920, 1080),
        (false, 0, 0, 0, 720, 1920, 1080),
        (false, 0, 0, 1280, 720, 0, 0),
        (true, 640, 360, 1280, 720, 1920, 1080),
        (true, 0, 0, 1280, 720, 1920, 1080),
        (true, 0, 360, 0, 0, 1920, 1080),
        (true, 640, 0, 0, 0, 1920, 1080),
        (true, 0, 0, 1280, 0, 1920, 1080),
        (true, 0, 0, 0, 0, 0, 0),
    ] {
        check_conversion(sizes(set, sw, sh, cw, ch, vw, vh));
    }
}

/// The congestion's bit patterns within 256 of 0, -0, 1, -1, the largest
/// finite floats, the infinities and the NaNs (quiet, signalling, either
/// sign), and every 65521st bit pattern of all 2^32.
#[test]
fn congestion_bits_match() {
    for center in [
        0x0000_0000u32,
        0x8000_0000,
        0x3f80_0000,
        0xbf80_0000,
        0x3f00_0000,
        0x7f7f_ffff,
        0xff7f_ffff,
        0x7f80_0000,
        0xff80_0000,
        0x7fc0_0000,
        0xffc0_0000,
        0x7fff_ffff,
        0xffff_ffff,
    ] {
        for d in -256i32..=256 {
            if let Some(b) = center.checked_add_signed(d) {
                check_congestion(f32::from_bits(b));
            }
        }
    }
    let mut b = 0u32;
    loop {
        check_congestion(f32::from_bits(b));
        match b.checked_add(65_521) {
            Some(next) => b = next,
            None => break,
        }
    }
    // The values the steps test pins.
    for v in [
        -1.0f32,
        0.0,
        0.5,
        1.0,
        2.0,
        f32::INFINITY,
        f32::NEG_INFINITY,
    ] {
        check_congestion(v);
    }
    assert!(f32::from_bits(check_congestion(f32::NAN)).is_nan());
}

/// A mixer mask: any `usize`, or a few low bits over any high bits.
fn mixer_mask() -> impl Strategy<Value = usize> {
    prop_oneof![
        any::<usize>(),
        (any::<usize>(), 0usize..64).prop_map(|(high, low)| (high & !0x3f) | low),
        (0u32..usize::BITS).prop_map(|i| 1usize << i),
    ]
}

/// A size: any `u32`, 0, or small.
fn size() -> impl Strategy<Value = u32> {
    prop_oneof![3 => any::<u32>(), 1 => Just(0u32), 1 => 0u32..=4]
}

/// The conversion sizes, with the scaled size often equal to the video's
/// on one axis or both.
fn conversion_sizes() -> impl Strategy<Value = obs_output_conversion_sizes> {
    (
        any::<bool>(),
        size(),
        size(),
        size(),
        size(),
        size(),
        size(),
        0u8..4,
    )
        .prop_map(|(set, sw, sh, cw, ch, vw, vh, same)| {
            let vw = if same & 1 != 0 { cw } else { vw };
            let vh = if same & 2 != 0 { ch } else { vh };
            sizes(set, sw, sh, cw, ch, vw, vh)
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(8192))]

    #[test]
    fn random_first_mixer_matches_oracle(mask in mixer_mask()) {
        check_first_mixer(mask);
    }

    #[test]
    fn random_size_route_matches_oracle(
        encoded in any::<bool>(),
        has_encoder in any::<bool>(),
        scaled in size(),
    ) {
        check_size_route(encoded, has_encoder, scaled);
    }

    #[test]
    fn random_capture_matches_oracle(
        flags in prop_oneof![any::<u32>(), 0u32..16],
        v in prop_oneof![any::<u32>(), 0u32..1024],
        a in prop_oneof![any::<u32>(), 0u32..64],
        bits in 0u32..8,
    ) {
        check_capture(presence(flags, v, a, bits));
    }

    #[test]
    fn random_conversion_matches_oracle(s in conversion_sizes()) {
        check_conversion(s);
    }

    #[test]
    fn random_congestion_matches_oracle(bits in any::<u32>()) {
        check_congestion(f32::from_bits(bits));
    }

    /// Bit patterns within 64 of 0, 1, -1 and the NaNs, either sign.
    #[test]
    fn random_congestion_near_edges_matches_oracle(
        center in prop::sample::select(vec![
            0x0000_0000u32, 0x8000_0000, 0x3f80_0000, 0xbf80_0000, 0x7fc0_0000, 0xffc0_0000,
        ]),
        d in -64i32..=64,
    ) {
        check_congestion(f32::from_bits(center.wrapping_add_signed(d)));
    }
}
