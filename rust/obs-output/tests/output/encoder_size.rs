//! Tier 1: `obs_output::encoder_size`, the encoder size and info choices.
//!
//! The `steps_*` cases port `test/cmocka/test_encoder_size_steps.c` 1:1:
//! its 28 cases under the same names, in the same order, with the same
//! values and expectations, the "characterized, not endorsed" ones
//! included. The C calls the hidden `obs_encoder_size_*` functions; here
//! the safe core is called directly ([`audio_defaults`], [`cap_override`],
//! [`scaled_gate`], [`scaled_match`], [`scaling_enabled`],
//! [`video_size_route`], [`sample_rate_route`]).
//!
//! The `edge_*` cases are new: every 4-bit pattern of the audio fields
//! against several mixes, every cap word over the low 8 bits and each high
//! bit, every type from -8 to 8 and the `i32` extremes against every flag
//! pair, sizes and rates at the `u32` edges, and the constants. Each checks
//! the core against a model of the original C.

use obs_output::encoder_size::{
    AUDIO_FORMAT_UNKNOWN, AudioFill, OBS_ENCODER_AUDIO, OBS_ENCODER_CAP_SCALING, OBS_ENCODER_VIDEO,
    SPEAKERS_UNKNOWN, ScaledGate, ScaledMatch, Size, SizeRoute, audio_defaults, cap_override,
    sample_rate_route, scaled_gate, scaled_match, scaling_enabled, video_size_route,
};

const TYPE_AUDIO: i32 = OBS_ENCODER_AUDIO;
const TYPE_VIDEO: i32 = OBS_ENCODER_VIDEO;
const CAP_SCALING: u32 = OBS_ENCODER_CAP_SCALING;

/// Types that are neither `OBS_ENCODER_AUDIO` nor `OBS_ENCODER_VIDEO`.
const OTHER_TYPES: [i32; 5] = [2, 3, -1, i32::MAX, i32::MIN];

const EDGE_SIZES: [u32; 11] = [
    0,
    1,
    2,
    720,
    1080,
    1280,
    1920,
    0x7fff_ffff,
    0x8000_0000,
    u32::MAX - 1,
    u32::MAX,
];

fn fill(format: i32, samples_per_sec: u32, speakers: i32) -> AudioFill {
    AudioFill {
        format,
        samples_per_sec,
        speakers,
    }
}

fn size(width: u32, height: u32) -> Size {
    Size { width, height }
}

fn expect_fill(info: AudioFill, mix: AudioFill, want: AudioFill) {
    assert_eq!(
        audio_defaults(info, mix),
        want,
        "defaults({info:?}, mix {mix:?})"
    );
}

fn expect_override(caps: u32, w: u32, h: u32, vw: u32, vh: u32, want_w: u32, want_h: u32) {
    assert_eq!(
        cap_override(caps, size(w, h), size(vw, vh)),
        size(want_w, want_h),
        "cap_override({caps:#x}, {w} x {h}, voi {vw} x {vh})"
    );
}

fn expect_gate(encoder_type: i32, active: bool, initialized: bool, want: ScaledGate) {
    assert_eq!(
        scaled_gate(encoder_type, active, initialized),
        want,
        "scaled_gate({encoder_type}, {active}, {initialized})"
    );
}

fn expect_match(has_voi: bool, vw: u32, vh: u32, w: u32, h: u32, want: ScaledMatch) {
    let voi = has_voi.then_some(size(vw, vh));
    assert_eq!(
        scaled_match(voi, size(w, h)),
        want,
        "scaled_match({voi:?}, {w} x {h})"
    );
}

fn expect_video_route(encoder_type: i32, has_media: bool, scaled: u32, want: SizeRoute) {
    assert_eq!(
        video_size_route(encoder_type, has_media, scaled),
        want,
        "video_route({encoder_type}, {has_media}, {scaled})"
    );
}

fn expect_rate_route(encoder_type: i32, has_media: bool, rate: u32, want: SizeRoute) {
    assert_eq!(
        sample_rate_route(encoder_type, has_media, rate),
        want,
        "rate_route({encoder_type}, {has_media}, {rate})"
    );
}

// ---------------------------------------------------------------------------
// audio_defaults

/// Both callers pass a zeroed info: every field comes from the mix.
#[test]
fn steps_audio_defaults_all_unknown() {
    expect_fill(fill(0, 0, 0), fill(8, 48000, 2), fill(8, 48000, 2));
    expect_fill(fill(0, 0, 0), fill(1, 44100, 8), fill(1, 44100, 8));
}

#[test]
fn steps_audio_defaults_format_unknown() {
    expect_fill(fill(0, 44100, 1), fill(8, 48000, 2), fill(8, 44100, 1));
}

#[test]
fn steps_audio_defaults_rate_unknown() {
    expect_fill(fill(4, 0, 1), fill(8, 48000, 2), fill(4, 48000, 1));
}

#[test]
fn steps_audio_defaults_speakers_unknown() {
    expect_fill(fill(4, 44100, 0), fill(8, 48000, 2), fill(4, 44100, 2));
}

#[test]
fn steps_audio_defaults_none_unknown() {
    expect_fill(fill(4, 44100, 1), fill(8, 48000, 2), fill(4, 44100, 1));
}

/// An unknown field of the mix is copied as it is: still unknown.
#[test]
fn steps_audio_defaults_mix_unknown() {
    expect_fill(fill(0, 0, 0), fill(0, 0, 0), fill(0, 0, 0));
    expect_fill(fill(4, 0, 0), fill(0, 0, 0), fill(4, 0, 0));
}

#[test]
fn steps_audio_defaults_every_combination() {
    let mix = fill(8, 96000, 7);
    for bits in 0..8 {
        let info = fill(
            if bits & 1 != 0 { 3 } else { 0 },
            if bits & 2 != 0 { 22050 } else { 0 },
            if bits & 4 != 0 { 4 } else { 0 },
        );
        let want = fill(
            if bits & 1 != 0 { 3 } else { 8 },
            if bits & 2 != 0 { 22050 } else { 96000 },
            if bits & 4 != 0 { 4 } else { 7 },
        );
        expect_fill(info, mix, want);
    }
}

/// Any value other than 0 counts as known, values outside the enums
/// included; the rate at the `u32` edges.
#[test]
fn steps_audio_defaults_edges() {
    expect_fill(
        fill(-1, u32::MAX, -1),
        fill(8, 48000, 2),
        fill(-1, u32::MAX, -1),
    );
    expect_fill(
        fill(i32::MAX, 1, i32::MIN),
        fill(8, 48000, 2),
        fill(i32::MAX, 1, i32::MIN),
    );
    expect_fill(
        fill(0, 0, 0),
        fill(i32::MIN, u32::MAX, i32::MAX),
        fill(i32::MIN, u32::MAX, i32::MAX),
    );
}

// ---------------------------------------------------------------------------
// cap_override

#[test]
fn steps_cap_override_without_cap() {
    expect_override(0, 1280, 720, 1920, 1080, 1280, 720);
    expect_override(!CAP_SCALING, 1280, 720, 1920, 1080, 1280, 720);
    expect_override(0, 0, 0, 1920, 1080, 0, 0);
}

#[test]
fn steps_cap_override_with_cap() {
    expect_override(CAP_SCALING, 1280, 720, 1920, 1080, 1920, 1080);
    expect_override(u32::MAX, 1280, 720, 1920, 1080, 1920, 1080);
    expect_override(CAP_SCALING, 1920, 1080, 1920, 1080, 1920, 1080);
    expect_override(CAP_SCALING, 0, 0, 1920, 1080, 1920, 1080);
}

#[test]
fn steps_cap_override_single_bits() {
    assert_eq!(CAP_SCALING, 1 << 5);
    for bit in 0..32 {
        let caps = 1u32 << bit;
        if bit == 5 {
            expect_override(caps, 640, 360, 1920, 1080, 1920, 1080);
        } else {
            expect_override(caps, 640, 360, 1920, 1080, 640, 360);
        }
    }
}

/// Characterized, not endorsed: with the cap, a video of 0 x 0 gives
/// 0 x 0, whatever the plugin set.
#[test]
fn steps_cap_override_u32_edges() {
    expect_override(CAP_SCALING, 1280, 720, 0, 0, 0, 0);
    for a in EDGE_SIZES {
        for b in EDGE_SIZES {
            expect_override(0, a, b, b, a, a, b);
            expect_override(CAP_SCALING, a, b, b, a, b, a);
        }
    }
}

// ---------------------------------------------------------------------------
// scaled_gate

#[test]
fn steps_scaled_gate_not_video() {
    for f in 0..4 {
        let (active, initialized) = (f & 1 != 0, f & 2 != 0);
        expect_gate(TYPE_AUDIO, active, initialized, ScaledGate::NotVideo);
        for t in OTHER_TYPES {
            expect_gate(t, active, initialized, ScaledGate::NotVideo);
        }
    }
}

#[test]
fn steps_scaled_gate_active() {
    expect_gate(TYPE_VIDEO, true, false, ScaledGate::Active);
    expect_gate(TYPE_VIDEO, true, true, ScaledGate::Active);
}

#[test]
fn steps_scaled_gate_initialized() {
    expect_gate(TYPE_VIDEO, false, true, ScaledGate::Initialized);
}

#[test]
fn steps_scaled_gate_check() {
    expect_gate(TYPE_VIDEO, false, false, ScaledGate::Check);
}

// ---------------------------------------------------------------------------
// scaled_match

#[test]
fn steps_scaled_match_equal() {
    expect_match(true, 1920, 1080, 1920, 1080, ScaledMatch::Disable);
    expect_match(true, 1280, 720, 1280, 720, ScaledMatch::Disable);
}

/// Characterized, not endorsed: a size equal to the video's on one axis,
/// or with one axis 0, is set as it is.
#[test]
fn steps_scaled_match_unequal() {
    expect_match(true, 1920, 1080, 1280, 720, ScaledMatch::Set);
    expect_match(true, 1920, 1080, 1920, 720, ScaledMatch::Set);
    expect_match(true, 1920, 1080, 1280, 1080, ScaledMatch::Set);
    expect_match(true, 1920, 1080, 1080, 1920, ScaledMatch::Set);
    expect_match(true, 1920, 1080, 0, 1080, ScaledMatch::Set);
    expect_match(true, 1920, 1080, 1920, 0, ScaledMatch::Set);
}

#[test]
fn steps_scaled_match_no_voi() {
    expect_match(false, 1920, 1080, 1920, 1080, ScaledMatch::Set);
    expect_match(false, 0, 0, 0, 0, ScaledMatch::Set);
    expect_match(false, 0, 0, 1280, 720, ScaledMatch::Set);
}

#[test]
fn steps_scaled_match_u32_edges() {
    expect_match(true, 0, 0, 0, 0, ScaledMatch::Disable);
    expect_match(true, 1920, 1080, 0, 0, ScaledMatch::Set);
    for a in EDGE_SIZES {
        for b in EDGE_SIZES {
            let same = if a == b {
                ScaledMatch::Disable
            } else {
                ScaledMatch::Set
            };
            expect_match(true, a, b, a, b, ScaledMatch::Disable);
            expect_match(true, a, b, b, a, same);
            expect_match(true, a, a, a, b, same);
            expect_match(false, a, b, a, b, ScaledMatch::Set);
        }
    }
}

// ---------------------------------------------------------------------------
// scaling_enabled

#[test]
fn steps_scaling_enabled() {
    assert!(!scaling_enabled(size(0, 0)));
    assert!(scaling_enabled(size(1280, 0)));
    assert!(scaling_enabled(size(0, 720)));
    assert!(scaling_enabled(size(1280, 720)));
    assert!(scaling_enabled(size(1, 0)));
    assert!(scaling_enabled(size(0, 1)));
    assert!(scaling_enabled(size(u32::MAX, 0)));
    assert!(scaling_enabled(size(0, u32::MAX)));
    assert!(scaling_enabled(size(u32::MAX, u32::MAX)));
    assert!(scaling_enabled(size(0x8000_0000, 0)));
}

// ---------------------------------------------------------------------------
// video_size_route, sample_rate_route

#[test]
fn steps_video_route_not_video() {
    for s in EDGE_SIZES {
        for m in [false, true] {
            expect_video_route(TYPE_AUDIO, m, s, SizeRoute::WrongType);
            for t in OTHER_TYPES {
                expect_video_route(t, m, s, SizeRoute::WrongType);
            }
        }
    }
}

#[test]
fn steps_video_route_no_media() {
    for s in EDGE_SIZES {
        expect_video_route(TYPE_VIDEO, false, s, SizeRoute::NoMedia);
    }
}

#[test]
fn steps_video_route_scaled_or_video() {
    expect_video_route(TYPE_VIDEO, true, 0, SizeRoute::Media);
    for s in EDGE_SIZES.into_iter().filter(|&s| s != 0) {
        expect_video_route(TYPE_VIDEO, true, s, SizeRoute::Stored);
    }
}

#[test]
fn steps_rate_route_not_audio() {
    for s in EDGE_SIZES {
        for m in [false, true] {
            expect_rate_route(TYPE_VIDEO, m, s, SizeRoute::WrongType);
            for t in OTHER_TYPES {
                expect_rate_route(t, m, s, SizeRoute::WrongType);
            }
        }
    }
}

#[test]
fn steps_rate_route_no_media() {
    for s in EDGE_SIZES {
        expect_rate_route(TYPE_AUDIO, false, s, SizeRoute::NoMedia);
    }
}

#[test]
fn steps_rate_route_stored_or_mix() {
    expect_rate_route(TYPE_AUDIO, true, 0, SizeRoute::Media);
    for r in [1, 8000, 44100, 48000, 96000, u32::MAX] {
        expect_rate_route(TYPE_AUDIO, true, r, SizeRoute::Stored);
    }
}

fn pick_axis(encoder_type: i32, has_media: bool, scaled: u32, video: u32) -> u32 {
    match video_size_route(encoder_type, has_media, scaled) {
        SizeRoute::Stored => scaled,
        SizeRoute::Media => video,
        SizeRoute::WrongType | SizeRoute::NoMedia => 0,
    }
}

#[test]
fn steps_width_height_end_to_end() {
    assert_eq!(pick_axis(TYPE_VIDEO, true, 0, 1920), 1920);
    assert_eq!(pick_axis(TYPE_VIDEO, true, 0, 1080), 1080);
    assert_eq!(pick_axis(TYPE_VIDEO, true, 1280, 1920), 1280);
    assert_eq!(pick_axis(TYPE_VIDEO, true, 720, 1080), 720);
    assert_eq!(pick_axis(TYPE_VIDEO, true, 1280, 1920), 1280);
    assert_eq!(pick_axis(TYPE_VIDEO, true, 0, 1080), 1080);
    assert_eq!(pick_axis(TYPE_VIDEO, false, 1280, 1920), 0);
    assert_eq!(pick_axis(TYPE_VIDEO, false, 0, 1920), 0);
    assert_eq!(pick_axis(TYPE_AUDIO, true, 1280, 1920), 0);
    assert_eq!(pick_axis(TYPE_VIDEO, true, u32::MAX, 1920), u32::MAX);
    assert_eq!(pick_axis(TYPE_VIDEO, true, 0, u32::MAX), u32::MAX);
}

// ---------------------------------------------------------------------------
// edge cases against models of the original C

/// The types checked below: -8 to 8 and the `i32` extremes.
fn types() -> impl Iterator<Item = i32> {
    (-8..=8).chain([i32::MIN, i32::MIN + 1, i32::MAX - 1, i32::MAX])
}

/// Every pattern of 0 and non-0 audio fields (the non-0 ones at small
/// values, outside the enums and at the edges) against several mixes.
#[test]
fn edge_audio_defaults_model() {
    let formats = [0, 1, 8, -1, i32::MIN, i32::MAX];
    let rates = [0, 1, 48000, 0x8000_0000, u32::MAX];
    let mixes = [
        fill(0, 0, 0),
        fill(8, 48000, 2),
        fill(-1, u32::MAX, i32::MIN),
        fill(i32::MAX, 1, 8),
    ];
    for mix in mixes {
        for f in formats {
            for r in rates {
                for s in formats {
                    let want = fill(
                        if f == AUDIO_FORMAT_UNKNOWN {
                            mix.format
                        } else {
                            f
                        },
                        if r == 0 { mix.samples_per_sec } else { r },
                        if s == SPEAKERS_UNKNOWN {
                            mix.speakers
                        } else {
                            s
                        },
                    );
                    expect_fill(fill(f, r, s), mix, want);
                }
            }
        }
    }
}

/// Every cap word over the low 8 bits, alone and with every high bit set,
/// and each high bit alone: only bit 5 matters.
#[test]
fn edge_cap_override_cap_words() {
    let info = size(640, 360);
    let voi = size(1920, 1080);
    for low in 0..256u32 {
        for caps in [low, low | 0xffff_ff00] {
            let want = if caps & (1 << 5) != 0 { voi } else { info };
            assert_eq!(cap_override(caps, info, voi), want, "{caps:#x}");
        }
    }
    for bit in 8..32 {
        assert_eq!(cap_override(1 << bit, info, voi), info, "bit {bit}");
    }
}

/// Every type against every flag pair, against the C's if chain.
#[test]
fn edge_scaled_gate_model() {
    for t in types() {
        for active in [false, true] {
            for initialized in [false, true] {
                let want = if t != 1 {
                    ScaledGate::NotVideo
                } else if active {
                    ScaledGate::Active
                } else if initialized {
                    ScaledGate::Initialized
                } else {
                    ScaledGate::Check
                };
                expect_gate(t, active, initialized, want);
            }
        }
    }
}

/// Every voi and size over the edge sizes on all four values, with and
/// without the voi.
#[test]
fn edge_scaled_match_model() {
    let values = [0u32, 1, 0x8000_0000, u32::MAX];
    for i in 0..4usize.pow(4) {
        let pick = |k: u32| values[(i / 4usize.pow(k)) % 4];
        let (vw, vh, w, h) = (pick(0), pick(1), pick(2), pick(3));
        for has_voi in [false, true] {
            let want = if has_voi && vw == w && vh == h {
                ScaledMatch::Disable
            } else {
                ScaledMatch::Set
            };
            expect_match(has_voi, vw, vh, w, h, want);
        }
    }
}

#[test]
fn edge_scaling_enabled_model() {
    for w in EDGE_SIZES {
        for h in EDGE_SIZES {
            assert_eq!(scaling_enabled(size(w, h)), w != 0 || h != 0, "{w} x {h}");
        }
    }
}

/// Every type against media and the edge sizes, for both routes.
#[test]
fn edge_routes_model() {
    for t in types() {
        for has_media in [false, true] {
            for s in EDGE_SIZES {
                let route = |want_type: i32| {
                    if t != want_type {
                        SizeRoute::WrongType
                    } else if !has_media {
                        SizeRoute::NoMedia
                    } else if s != 0 {
                        SizeRoute::Stored
                    } else {
                        SizeRoute::Media
                    }
                };
                expect_video_route(t, has_media, s, route(1));
                expect_rate_route(t, has_media, s, route(0));
            }
        }
    }
}

#[test]
fn edge_constants() {
    assert_eq!(OBS_ENCODER_AUDIO, 0);
    assert_eq!(OBS_ENCODER_VIDEO, 1);
    assert_eq!(OBS_ENCODER_CAP_SCALING, 0x20);
    assert_eq!(AUDIO_FORMAT_UNKNOWN, 0);
    assert_eq!(SPEAKERS_UNKNOWN, 0);
}
