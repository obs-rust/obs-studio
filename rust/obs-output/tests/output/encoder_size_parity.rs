//! Tier 2 layout and Tier 3 parity: the shims of
//! `obs_output::ffi::encoder_size` (and the safe core under them) against
//! the original `libobs/obs-encoder-size.c.h`, compiled as
//! `oracle_obs_encoder_size_*` (obs-c-oracle, oracle/encoder_size.c).
//!
//! Every output is compared exactly: the audio fill and the size field by
//! field, the gate, match and route as their values, the scaling check as a
//! bool.
//!
//! Runs: the constants and the layout of the two structs and the two enums;
//! the audio defaults over every combination of seven formats, five rates
//! and seven speaker values on both the info and the mix (60,025 calls);
//! the cap override over every cap word of the low 16 bits, alone and with
//! all high bits, each high bit alone, against special sizes; the gate
//! exhaustively over both flags and every type from -65,536 to 65,536, the
//! `i32` extremes and every 65,521st `i32`; the match over every
//! combination of six edge sizes on all four sizes, with and without a voi;
//! the scaling check over every pair of edge sizes; both routes over the
//! same types, media and edge values; and proptests over full-range `i32`
//! types, `u32` caps, `u32` sizes and sample rates (biased to 0, to the
//! scaled size equal to the video's on one axis or both, and to the cap
//! bit).
//!
//! No input is excluded. No intentional differences.
//! `test_encoder_size_steps` runs the C test against the C; the real-libobs
//! encoder tests run against the hybrid libobs, where these shims replace
//! the C.

use std::ffi::{c_int, c_void};
use std::mem::{align_of, offset_of, size_of};

use obs_c_oracle::encoder_size::{
    AudioFill as OracleFill, CONSTANT_NAMES, EncoderSize as OracleSize, LAYOUT_NAMES,
    oracle_encoder_size_constant, oracle_encoder_size_layout,
    oracle_obs_encoder_size_audio_defaults, oracle_obs_encoder_size_cap_override,
    oracle_obs_encoder_size_rate_route, oracle_obs_encoder_size_scaled_gate,
    oracle_obs_encoder_size_scaled_match, oracle_obs_encoder_size_scaling,
    oracle_obs_encoder_size_video_route,
};
use obs_output::encoder_size::{
    self as core, AUDIO_FORMAT_UNKNOWN, AudioFill, OBS_ENCODER_AUDIO, OBS_ENCODER_CAP_SCALING,
    OBS_ENCODER_VIDEO, SPEAKERS_UNKNOWN, ScaledGate, ScaledMatch, Size, SizeRoute,
};
use obs_output::ffi::encoder_size::{
    OBS_ENCODER_SCALED_ACTIVE, OBS_ENCODER_SCALED_CHECK, OBS_ENCODER_SCALED_DISABLE,
    OBS_ENCODER_SCALED_INITIALIZED, OBS_ENCODER_SCALED_NOT_VIDEO, OBS_ENCODER_SCALED_SET,
    OBS_ENCODER_SIZE_MEDIA, OBS_ENCODER_SIZE_NO_MEDIA, OBS_ENCODER_SIZE_STORED,
    OBS_ENCODER_SIZE_WRONG_TYPE, obs_encoder_audio_fill, obs_encoder_size,
    obs_encoder_size_audio_defaults, obs_encoder_size_cap_override, obs_encoder_size_rate_route,
    obs_encoder_size_scaled_gate, obs_encoder_size_scaled_match, obs_encoder_size_scaling,
    obs_encoder_size_video_route,
};
use proptest::prelude::*;

const EDGES: [u32; 6] = [0, 1, 2, 0x8000_0000, u32::MAX - 1, u32::MAX];

fn route_value(route: SizeRoute) -> u32 {
    match route {
        SizeRoute::WrongType => OBS_ENCODER_SIZE_WRONG_TYPE,
        SizeRoute::NoMedia => OBS_ENCODER_SIZE_NO_MEDIA,
        SizeRoute::Stored => OBS_ENCODER_SIZE_STORED,
        SizeRoute::Media => OBS_ENCODER_SIZE_MEDIA,
    }
}

fn gate_value(gate: ScaledGate) -> u32 {
    match gate {
        ScaledGate::NotVideo => OBS_ENCODER_SCALED_NOT_VIDEO,
        ScaledGate::Active => OBS_ENCODER_SCALED_ACTIVE,
        ScaledGate::Initialized => OBS_ENCODER_SCALED_INITIALIZED,
        ScaledGate::Check => OBS_ENCODER_SCALED_CHECK,
    }
}

fn match_value(m: ScaledMatch) -> u32 {
    match m {
        ScaledMatch::Disable => OBS_ENCODER_SCALED_DISABLE,
        ScaledMatch::Set => OBS_ENCODER_SCALED_SET,
    }
}

fn check_defaults(info: obs_encoder_audio_fill, mix: obs_encoder_audio_fill) -> OracleFill {
    // SAFETY: live locals, read during the call.
    let rust = unsafe { obs_encoder_size_audio_defaults(&raw const info, &raw const mix) };
    // SAFETY: as above; the oracle reads them as struct
    // obs_encoder_audio_fill, whose layout layout_matches_c checks.
    let c = unsafe {
        oracle_obs_encoder_size_audio_defaults(
            (&raw const info).cast::<c_void>(),
            (&raw const mix).cast::<c_void>(),
        )
    };
    let shim = OracleFill {
        format: rust.format,
        samples_per_sec: rust.samples_per_sec,
        speakers: rust.speakers,
    };
    assert_eq!(shim, c, "audio_defaults({info:?}, {mix:?})");
    let core = core::audio_defaults(
        AudioFill {
            format: info.format,
            samples_per_sec: info.samples_per_sec,
            speakers: info.speakers,
        },
        AudioFill {
            format: mix.format,
            samples_per_sec: mix.samples_per_sec,
            speakers: mix.speakers,
        },
    );
    let core = OracleFill {
        format: core.format,
        samples_per_sec: core.samples_per_sec,
        speakers: core.speakers,
    };
    assert_eq!(core, c, "core audio_defaults({info:?}, {mix:?})");
    c
}

fn check_override(caps: u32, w: u32, h: u32, vw: u32, vh: u32) -> OracleSize {
    let rust: obs_encoder_size = obs_encoder_size_cap_override(caps, w, h, vw, vh);
    // SAFETY: a plain value function.
    let c = unsafe { oracle_obs_encoder_size_cap_override(caps, w, h, vw, vh) };
    let what = format!("cap_override({caps:#x}, {w} x {h}, voi {vw} x {vh})");
    let shim = OracleSize {
        width: rust.width,
        height: rust.height,
    };
    assert_eq!(shim, c, "{what}");
    let core = core::cap_override(
        caps,
        Size {
            width: w,
            height: h,
        },
        Size {
            width: vw,
            height: vh,
        },
    );
    let core = OracleSize {
        width: core.width,
        height: core.height,
    };
    assert_eq!(core, c, "core {what}");
    c
}

fn check_gate(encoder_type: c_int, active: bool, initialized: bool) -> u32 {
    let rust = obs_encoder_size_scaled_gate(encoder_type, active, initialized);
    // SAFETY: a plain value function.
    let c = unsafe { oracle_obs_encoder_size_scaled_gate(encoder_type, active, initialized) };
    let what = format!("scaled_gate({encoder_type}, {active}, {initialized})");
    assert_eq!(rust, c, "{what}");
    assert_eq!(
        gate_value(core::scaled_gate(encoder_type, active, initialized)),
        c,
        "core {what}"
    );
    c
}

fn check_match(has_voi: bool, vw: u32, vh: u32, w: u32, h: u32) -> u32 {
    let rust = obs_encoder_size_scaled_match(has_voi, vw, vh, w, h);
    // SAFETY: a plain value function.
    let c = unsafe { oracle_obs_encoder_size_scaled_match(has_voi, vw, vh, w, h) };
    let what = format!("scaled_match({has_voi}, voi {vw} x {vh}, {w} x {h})");
    assert_eq!(rust, c, "{what}");
    let voi = has_voi.then_some(Size {
        width: vw,
        height: vh,
    });
    let core = core::scaled_match(
        voi,
        Size {
            width: w,
            height: h,
        },
    );
    assert_eq!(match_value(core), c, "core {what}");
    c
}

fn check_scaling(w: u32, h: u32) -> bool {
    let rust = obs_encoder_size_scaling(w, h);
    // SAFETY: a plain value function.
    let c = unsafe { oracle_obs_encoder_size_scaling(w, h) };
    assert_eq!(rust, c, "scaling({w}, {h})");
    assert_eq!(
        core::scaling_enabled(Size {
            width: w,
            height: h
        }),
        c,
        "core scaling({w}, {h})"
    );
    c
}

fn check_routes(encoder_type: c_int, has_media: bool, stored: u32) -> (u32, u32) {
    let what = format!("({encoder_type}, {has_media}, {stored})");
    let rust = obs_encoder_size_video_route(encoder_type, has_media, stored);
    // SAFETY: a plain value function.
    let c = unsafe { oracle_obs_encoder_size_video_route(encoder_type, has_media, stored) };
    assert_eq!(rust, c, "video_route{what}");
    assert_eq!(
        route_value(core::video_size_route(encoder_type, has_media, stored)),
        c,
        "core video_route{what}"
    );
    let rust_rate = obs_encoder_size_rate_route(encoder_type, has_media, stored);
    // SAFETY: a plain value function.
    let c_rate = unsafe { oracle_obs_encoder_size_rate_route(encoder_type, has_media, stored) };
    assert_eq!(rust_rate, c_rate, "rate_route{what}");
    assert_eq!(
        route_value(core::sample_rate_route(encoder_type, has_media, stored)),
        c_rate,
        "core rate_route{what}"
    );
    (c, c_rate)
}

fn fill(format: c_int, samples_per_sec: u32, speakers: c_int) -> obs_encoder_audio_fill {
    obs_encoder_audio_fill {
        format,
        samples_per_sec,
        speakers,
    }
}

/// The types the exhaustive runs check: -65,536 to 65,536, the `i32`
/// extremes, and every 65,521st `i32`.
fn types() -> impl Iterator<Item = c_int> {
    (-65_536..=65_536)
        .chain([c_int::MIN, c_int::MIN + 1, c_int::MAX - 1, c_int::MAX])
        .chain((c_int::MIN..=c_int::MAX).step_by(65_521))
}

#[test]
fn layout_matches_c() {
    let rust = [
        size_of::<obs_encoder_audio_fill>(),
        align_of::<obs_encoder_audio_fill>(),
        offset_of!(obs_encoder_audio_fill, format),
        offset_of!(obs_encoder_audio_fill, samples_per_sec),
        offset_of!(obs_encoder_audio_fill, speakers),
        size_of::<obs_encoder_size>(),
        align_of::<obs_encoder_size>(),
        offset_of!(obs_encoder_size, width),
        offset_of!(obs_encoder_size, height),
        size_of::<u32>(),
        size_of::<u32>(),
    ];
    for (i, (name, rust)) in LAYOUT_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: a plain value lookup.
        let c = unsafe { oracle_encoder_size_layout(i) };
        assert_eq!(rust, c, "{name}");
    }
    // SAFETY: as above.
    assert_eq!(
        unsafe { oracle_encoder_size_layout(LAYOUT_NAMES.len()) },
        usize::MAX
    );
    // The oracle's mirrors of the returned structs have the same layout.
    assert_eq!(size_of::<OracleFill>(), size_of::<obs_encoder_audio_fill>());
    assert_eq!(
        offset_of!(OracleFill, speakers),
        offset_of!(obs_encoder_audio_fill, speakers)
    );
    assert_eq!(size_of::<OracleSize>(), size_of::<obs_encoder_size>());
    assert_eq!(
        offset_of!(OracleSize, height),
        offset_of!(obs_encoder_size, height)
    );
}

#[test]
fn constants_match_c() {
    let rust: [i64; 15] = [
        i64::from(OBS_ENCODER_AUDIO),
        i64::from(OBS_ENCODER_VIDEO),
        i64::from(OBS_ENCODER_CAP_SCALING),
        i64::from(AUDIO_FORMAT_UNKNOWN),
        i64::from(SPEAKERS_UNKNOWN),
        i64::from(OBS_ENCODER_SIZE_WRONG_TYPE),
        i64::from(OBS_ENCODER_SIZE_NO_MEDIA),
        i64::from(OBS_ENCODER_SIZE_STORED),
        i64::from(OBS_ENCODER_SIZE_MEDIA),
        i64::from(OBS_ENCODER_SCALED_NOT_VIDEO),
        i64::from(OBS_ENCODER_SCALED_ACTIVE),
        i64::from(OBS_ENCODER_SCALED_INITIALIZED),
        i64::from(OBS_ENCODER_SCALED_CHECK),
        i64::from(OBS_ENCODER_SCALED_DISABLE),
        i64::from(OBS_ENCODER_SCALED_SET),
    ];
    for (i, (name, rust)) in CONSTANT_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: a plain value lookup.
        let c = unsafe { oracle_encoder_size_constant(i) };
        assert_eq!(rust, c, "{name}");
    }
    // SAFETY: as above.
    assert_eq!(
        unsafe { oracle_encoder_size_constant(CONSTANT_NAMES.len()) },
        -1
    );
}

/// Every combination of seven formats, five rates and seven speaker values,
/// on the info and on the mix.
#[test]
fn audio_defaults_exhaustive_match() {
    let enums = [0, 1, 2, 8, -1, c_int::MIN, c_int::MAX];
    let rates = [0, 1, 48000, 0x8000_0000, u32::MAX];
    let mut fills = Vec::new();
    for f in enums {
        for r in rates {
            for s in enums {
                fills.push(fill(f, r, s));
            }
        }
    }
    let mut filled = 0u64;
    for &info in &fills {
        for &mix in &fills {
            let c = check_defaults(info, mix);
            filled += u64::from(c != OracleFill::default() && info == fill(0, 0, 0));
        }
    }
    assert_eq!(fills.len(), 245);
    assert!(filled > 0);
}

/// Every cap word over the low 16 bits, alone and with every high bit set,
/// and each high bit alone, against special sizes.
#[test]
fn cap_override_cap_words_match() {
    let sizes = [
        (0, 0, 0, 0),
        (640, 360, 1920, 1080),
        (u32::MAX, 0, 0, u32::MAX),
    ];
    let mut overridden = 0u64;
    for low in 0..=0xffffu32 {
        for caps in [low, low | 0xffff_0000] {
            for (w, h, vw, vh) in sizes {
                let c = check_override(caps, w, h, vw, vh);
                overridden += u64::from(c.width == vw && c.width != w);
            }
        }
    }
    for bit in 16..32 {
        for (w, h, vw, vh) in sizes {
            check_override(1 << bit, w, h, vw, vh);
        }
    }
    assert_eq!(overridden, 2 * 0x8000 * 2);
}

/// Every size over the edge values on all four, with and without the cap.
#[test]
fn cap_override_sizes_match() {
    for i in 0..EDGES.len().pow(4) {
        let pick = |k: u32| EDGES[(i / EDGES.len().pow(k)) % EDGES.len()];
        for caps in [0, OBS_ENCODER_CAP_SCALING, u32::MAX] {
            check_override(caps, pick(0), pick(1), pick(2), pick(3));
        }
    }
}

/// Both flags against every type of [`types`].
#[test]
fn scaled_gate_exhaustive_match() {
    let mut seen = [0u64; 4];
    for t in types() {
        for active in [false, true] {
            for initialized in [false, true] {
                let g = check_gate(t, active, initialized);
                seen[usize::try_from(g).expect("small")] += 1;
            }
        }
    }
    assert!(seen.iter().all(|&n| n > 0), "{seen:?}");
}

/// Every combination of the edge values on the voi and the size, with and
/// without the voi.
#[test]
fn scaled_match_exhaustive_match() {
    let mut disabled = 0u64;
    for i in 0..EDGES.len().pow(4) {
        let pick = |k: u32| EDGES[(i / EDGES.len().pow(k)) % EDGES.len()];
        for has_voi in [false, true] {
            disabled += u64::from(
                check_match(has_voi, pick(0), pick(1), pick(2), pick(3))
                    == OBS_ENCODER_SCALED_DISABLE,
            );
        }
    }
    assert_eq!(disabled, 36);
}

#[test]
fn scaling_exhaustive_match() {
    for w in EDGES {
        for h in EDGES {
            check_scaling(w, h);
        }
    }
}

/// Both routes over every type of [`types`], media or not, and the edge
/// values.
#[test]
fn routes_exhaustive_match() {
    let mut seen = [[0u64; 4]; 2];
    for t in types().step_by(7).chain(-8..=8) {
        for has_media in [false, true] {
            for s in EDGES {
                let (v, r) = check_routes(t, has_media, s);
                seen[0][usize::try_from(v).expect("small")] += 1;
                seen[1][usize::try_from(r).expect("small")] += 1;
            }
        }
    }
    assert!(seen.iter().flatten().all(|&n| n > 0), "{seen:?}");
}

fn encoder_type() -> impl Strategy<Value = c_int> {
    prop_oneof![2 => any::<c_int>(), 2 => -2..=3, 1 => Just(0), 1 => Just(1)]
}

fn dimension() -> impl Strategy<Value = u32> {
    prop_oneof![3 => any::<u32>(), 1 => Just(0u32), 1 => 0u32..=4, 1 => (u32::MAX - 4)..=u32::MAX]
}

fn caps() -> impl Strategy<Value = u32> {
    prop_oneof![
        2 => any::<u32>(),
        1 => any::<u32>().prop_map(|c| c | OBS_ENCODER_CAP_SCALING),
        1 => any::<u32>().prop_map(|c| c & !OBS_ENCODER_CAP_SCALING),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(8192))]

    #[test]
    fn random_audio_defaults_match_oracle(
        f in prop_oneof![any::<c_int>(), Just(0)],
        r in prop_oneof![any::<u32>(), Just(0u32)],
        s in prop_oneof![any::<c_int>(), Just(0)],
        mf in any::<c_int>(),
        mr in any::<u32>(),
        ms in any::<c_int>(),
    ) {
        check_defaults(fill(f, r, s), fill(mf, mr, ms));
    }

    #[test]
    fn random_cap_override_matches_oracle(
        caps in caps(),
        w in dimension(),
        h in dimension(),
        vw in dimension(),
        vh in dimension(),
    ) {
        check_override(caps, w, h, vw, vh);
    }

    #[test]
    fn random_scaled_gate_matches_oracle(
        t in encoder_type(),
        active in any::<bool>(),
        initialized in any::<bool>(),
    ) {
        check_gate(t, active, initialized);
    }

    /// Full-range sizes, the asked size biased to equal the voi's on one
    /// axis or both.
    #[test]
    fn random_scaled_match_matches_oracle(
        has_voi in any::<bool>(),
        vw in dimension(),
        vh in dimension(),
        w in dimension(),
        h in dimension(),
        same in 0u32..4,
    ) {
        let w = if same & 1 != 0 { vw } else { w };
        let h = if same & 2 != 0 { vh } else { h };
        check_match(has_voi, vw, vh, w, h);
    }

    #[test]
    fn random_scaling_matches_oracle(w in dimension(), h in dimension()) {
        check_scaling(w, h);
    }

    /// Full-range scaled sizes and sample rates.
    #[test]
    fn random_routes_match_oracle(
        t in encoder_type(),
        has_media in any::<bool>(),
        stored in dimension(),
    ) {
        check_routes(t, has_media, stored);
    }
}
