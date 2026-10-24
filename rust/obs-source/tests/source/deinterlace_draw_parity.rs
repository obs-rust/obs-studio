//! Tier 3: the `obs_source_deinterlace_draw_*` shims against the original C
//! (`obs-c-oracle`, `oracle/source_deinterlace_draw.c`: the unmodified
//! `libobs/obs-source-deinterlace-draw.c.h` with its functions renamed).
//!
//! Every function takes and returns plain values, so a run compares the
//! bytes of the two results: the C's `bool` as the byte it returns, the
//! enums as their value, the `float` multiplier as its bits.
//!
//! Runs:
//! - the enum values, `TWOX_TOLERANCE` and the type sizes against the C
//!   headers;
//! - modes, exhaustively around the enum (-70000 to 70000), every
//!   65521st `i32` over the whole range and the ends of `i32`: the effect,
//!   the linear rule and every mode change between 41 of them;
//! - space pairs and flags: the technique, the white level read, the
//!   multiplier (at 26 special white levels) for every pair of 649 spaces
//!   around the enum and near the ends of `i32`, and the linear flag for
//!   every space, both graphics flags and every mode;
//! - the early return for both flags each and 18 widths and heights;
//! - frame2 for every combination of 18 special `u64` values and 10 half
//!   durations, and at the threshold -2 to +2;
//! - `deinterlace_render()`'s decisions in its order (the early return, the
//!   linear sRGB read and flag, the technique, the white level read, the
//!   multiplier and frame2), one copy on each side;
//! - proptests (20,000 cases each): frame2 over full-range `u64` times and
//!   `u32` half durations (half of them at the threshold), the multiplier
//!   over any space and any white level bit pattern, the mode functions over
//!   any `i32`, and the render decisions over all of it at once.
//!
//! No intentional differences: there is no pointer to check.

use std::os::raw::c_int;

use obs_c_oracle::source_deinterlace_draw as oracle;
use obs_source::deinterlace_draw::{
    GS_CS_709_EXTENDED, GS_CS_709_SCRGB, GS_CS_SRGB, GS_CS_SRGB_16F, MODE_BLEND, MODE_BLEND_2X,
    MODE_DISABLE, MODE_DISCARD, MODE_LINEAR, MODE_LINEAR_2X, MODE_RETRO, MODE_YADIF, MODE_YADIF_2X,
};
use obs_source::deinterlace_pacing::TWOX_TOLERANCE;
use obs_source::ffi::source_deinterlace_draw as shim;
use proptest::prelude::*;

const TWOX: u64 = TWOX_TOLERANCE;

#[test]
fn values_match_the_c_headers() {
    let rust: [i64; 39] = [
        MODE_DISABLE.into(),
        MODE_DISCARD.into(),
        MODE_RETRO.into(),
        MODE_BLEND.into(),
        MODE_BLEND_2X.into(),
        MODE_LINEAR.into(),
        MODE_LINEAR_2X.into(),
        MODE_YADIF.into(),
        MODE_YADIF_2X.into(),
        GS_CS_SRGB.into(),
        GS_CS_SRGB_16F.into(),
        GS_CS_709_EXTENDED.into(),
        GS_CS_709_SCRGB.into(),
        shim::OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_NONE.into(),
        0,
        1,
        2,
        3,
        4,
        5,
        6,
        7,
        0,
        1,
        2,
        3,
        0,
        1,
        2,
        3,
        i64::try_from(TWOX).unwrap(),
        4,
        4,
        4,
        4,
        4,
        4,
        1,
        4,
    ];
    for (which, (name, value)) in oracle::VALUE_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: a pure lookup.
        let c = unsafe { oracle::oracle_source_deinterlace_draw_value(which) };
        assert_eq!(value, c, "{name}");
    }
    // SAFETY: a pure lookup, one past the end.
    let past = unsafe { oracle::oracle_source_deinterlace_draw_value(oracle::VALUE_NAMES.len()) };
    assert_eq!(past, i64::MIN, "the list has as many values as names");
    // The effect, technique and mode change numbers of the shim are the
    // core's.
    for mode in -2..=10 {
        assert_eq!(
            shim::obs_source_deinterlace_draw_effect(mode),
            obs_source::deinterlace_draw::effect(mode).map_or(-1, |e| e as c_int)
        );
    }
}

// --------------------------------------------------------------------------
// One call on each side

fn c_effect(mode: c_int) -> c_int {
    // SAFETY: plain values.
    unsafe { oracle::oracle_obs_source_deinterlace_draw_effect(mode) }
}

fn c_linear_required(mode: c_int) -> u8 {
    // SAFETY: plain values.
    unsafe { oracle::oracle_obs_source_deinterlace_draw_linear_required(mode) }
}

fn c_skip(cur: bool, prev: bool, width: u32, height: u32) -> u8 {
    // SAFETY: plain values.
    unsafe { oracle::oracle_obs_source_deinterlace_draw_skip(cur, prev, width, height) }
}

fn c_reads_linear_srgb(space: c_int) -> u8 {
    // SAFETY: plain values.
    unsafe { oracle::oracle_obs_source_deinterlace_draw_reads_linear_srgb(space) }
}

fn c_linear_srgb(space: c_int, gs: bool, mode: c_int) -> u8 {
    // SAFETY: plain values.
    unsafe { oracle::oracle_obs_source_deinterlace_draw_linear_srgb(space, gs, mode) }
}

fn c_technique(source: c_int, current: c_int) -> u32 {
    // SAFETY: plain values.
    unsafe { oracle::oracle_obs_source_deinterlace_draw_technique(source, current) }
}

fn c_reads_white_level(source: c_int, current: c_int) -> u8 {
    // SAFETY: plain values.
    unsafe { oracle::oracle_obs_source_deinterlace_draw_reads_white_level(source, current) }
}

fn c_multiplier(source: c_int, current: c_int, white: f32) -> u32 {
    // SAFETY: plain values.
    unsafe { oracle::oracle_obs_source_deinterlace_draw_multiplier(source, current, white) }
        .to_bits()
}

fn c_frame2(video_time: u64, frame_ts: u64, offset: u64, half: u32) -> u8 {
    // SAFETY: plain values.
    unsafe { oracle::oracle_obs_source_deinterlace_draw_frame2(video_time, frame_ts, offset, half) }
}

fn c_mode_change(current: c_int, mode: c_int) -> u32 {
    // SAFETY: plain values.
    unsafe { oracle::oracle_obs_source_deinterlace_draw_mode_change(current, mode) }
}

fn byte(b: bool) -> u8 {
    u8::from(b)
}

#[track_caller]
fn check_mode(mode: c_int) {
    assert_eq!(
        shim::obs_source_deinterlace_draw_effect(mode),
        c_effect(mode),
        "effect({mode})"
    );
    assert_eq!(
        byte(shim::obs_source_deinterlace_draw_linear_required(mode)),
        c_linear_required(mode),
        "linear_required({mode})"
    );
}

#[track_caller]
fn check_mode_change(current: c_int, mode: c_int) {
    assert_eq!(
        shim::obs_source_deinterlace_draw_mode_change(current, mode),
        c_mode_change(current, mode),
        "mode_change({current}, {mode})"
    );
}

#[track_caller]
fn check_pair(source: c_int, current: c_int, white: f32) {
    assert_eq!(
        shim::obs_source_deinterlace_draw_technique(source, current),
        c_technique(source, current),
        "technique({source}, {current})"
    );
    assert_eq!(
        byte(shim::obs_source_deinterlace_draw_reads_white_level(
            source, current
        )),
        c_reads_white_level(source, current),
        "reads_white_level({source}, {current})"
    );
    assert_eq!(
        shim::obs_source_deinterlace_draw_multiplier(source, current, white).to_bits(),
        c_multiplier(source, current, white),
        "multiplier({source}, {current}, {:#010x})",
        white.to_bits()
    );
}

#[track_caller]
fn check_frame2(video_time: u64, frame_ts: u64, offset: u64, half: u32) {
    assert_eq!(
        byte(shim::obs_source_deinterlace_draw_frame2(
            video_time, frame_ts, offset, half
        )),
        c_frame2(video_time, frame_ts, offset, half),
        "frame2({video_time}, {frame_ts}, {offset}, {half})"
    );
}

// --------------------------------------------------------------------------
// Inputs

/// Modes: -70000 to 70000, every 65521st `i32` from `i32::MIN`, and the
/// ends.
fn sweep_modes() -> impl Iterator<Item = c_int> {
    let stride = (0_i64..=65_545).map(|k| i32::try_from(i64::from(i32::MIN) + k * 65_521).unwrap());
    (-70_000..=70_000)
        .chain(stride)
        .chain([i32::MIN, i32::MIN + 1, i32::MAX - 1, i32::MAX])
}

/// 41 modes for every pair of mode changes.
const PAIR_MODES: [c_int; 41] = [
    -3,
    -2,
    -1,
    0,
    1,
    2,
    3,
    4,
    5,
    6,
    7,
    8,
    9,
    10,
    11,
    12,
    15,
    16,
    31,
    32,
    63,
    64,
    99,
    127,
    128,
    255,
    256,
    257,
    258,
    264,
    0xffff,
    0x1_0000,
    0x1_0008,
    -256,
    -255,
    i32::MIN,
    i32::MIN + 1,
    i32::MIN + 8,
    i32::MAX - 8,
    i32::MAX - 1,
    i32::MAX,
];

/// Spaces: -320 to 320 and 4 near each end of `i32`, 649 values.
fn spaces() -> Vec<c_int> {
    let mut spaces: Vec<c_int> = (-320..=320).collect();
    spaces.extend([
        i32::MIN,
        i32::MIN + 1,
        i32::MIN + 2,
        i32::MIN + 3,
        i32::MAX - 3,
        i32::MAX - 2,
        i32::MAX - 1,
        i32::MAX,
    ]);
    spaces
}

/// Special white levels as bits: zeros, subnormals, the normal ends, OBS's
/// levels (80 to 480 nits), infinities, quiet and signalling NaNs of both
/// signs with payloads.
const WHITES: [u32; 26] = [
    0x0000_0000,
    0x8000_0000,
    0x0000_0001,
    0x8000_0001,
    0x007f_ffff,
    0x0080_0000,
    0x7f7f_ffff,
    0xff7f_ffff,
    0x3f80_0000,
    0x42a0_0000,
    0x42a0_0001,
    0x429f_ffff,
    0x4396_0000,
    0x43f0_0000,
    0x4348_0000,
    0xc2a0_0000,
    0x7f80_0000,
    0xff80_0000,
    0x7fc0_0000,
    0xffc0_0000,
    0x7fc1_2345,
    0xffff_ffff,
    0x7f80_0001,
    0xff80_0001,
    0x7fbf_ffff,
    0x3c23_d70a,
];

const SIZES: [u32; 18] = [
    0,
    1,
    2,
    3,
    7,
    8,
    720,
    1080,
    1920,
    0xff,
    0x100,
    0xffff,
    0x1_0000,
    0x7fff_ffff,
    0x8000_0000,
    0xffff_fffe,
    u32::MAX,
    0x0101_0101,
];

const U64S: [u64; 18] = [
    0,
    1,
    999_999,
    1_000_000,
    1_000_001,
    2_000_000,
    16_666_667,
    20_000_000,
    1_000_000_000_000,
    0xffff_ffff,
    0x1_0000_0000,
    0x7fff_ffff_ffff_ffff,
    0x8000_0000_0000_0000,
    u64::MAX - 1_000_000,
    u64::MAX - 999_999,
    u64::MAX - 20_000_000,
    u64::MAX - 1,
    u64::MAX,
];

const HALVES: [u32; 10] = [
    0,
    1,
    500_000,
    999_999,
    1_000_000,
    1_000_001,
    8_333_333,
    20_000_000,
    0x7fff_ffff,
    u32::MAX,
];

// --------------------------------------------------------------------------
// Exhaustive runs

#[test]
fn modes_sweep() {
    let mut n = 0_usize;
    for mode in sweep_modes() {
        check_mode(mode);
        n += 1;
    }
    assert!(n > 200_000, "{n} modes");
}

#[test]
fn mode_change_every_pair() {
    for current in PAIR_MODES {
        for mode in PAIR_MODES {
            check_mode_change(current, mode);
        }
    }
}

#[test]
fn space_pairs_at_special_levels() {
    let spaces = spaces();
    for &source in &spaces {
        for &current in &spaces {
            check_pair(source, current, 300.0);
        }
    }
    // Every white level for the pairs near the enum.
    for source in -3..=6 {
        for current in -3..=6 {
            for bits in WHITES {
                check_pair(source, current, f32::from_bits(bits));
            }
        }
    }
}

#[test]
fn linear_srgb_every_space_flag_and_mode() {
    for space in spaces() {
        assert_eq!(
            byte(shim::obs_source_deinterlace_draw_reads_linear_srgb(space)),
            c_reads_linear_srgb(space),
            "reads_linear_srgb({space})"
        );
        for gs in [false, true] {
            for mode in PAIR_MODES {
                assert_eq!(
                    byte(shim::obs_source_deinterlace_draw_linear_srgb(
                        space, gs, mode
                    )),
                    c_linear_srgb(space, gs, mode),
                    "linear_srgb({space}, {gs}, {mode})"
                );
            }
        }
    }
}

#[test]
fn skip_every_flag_and_size() {
    for cur in [false, true] {
        for prev in [false, true] {
            for width in SIZES {
                for height in SIZES {
                    assert_eq!(
                        byte(shim::obs_source_deinterlace_draw_skip(
                            cur, prev, width, height
                        )),
                        c_skip(cur, prev, width, height),
                        "skip({cur}, {prev}, {width}, {height})"
                    );
                }
            }
        }
    }
}

#[test]
fn frame2_special_values() {
    for video_time in U64S {
        for frame_ts in U64S {
            for offset in U64S {
                for half in HALVES {
                    check_frame2(video_time, frame_ts, offset, half);
                }
            }
        }
    }
    for frame_ts in U64S {
        for offset in U64S {
            for half in HALVES {
                let at = frame_ts
                    .wrapping_add(offset)
                    .wrapping_add(u64::from(half))
                    .wrapping_sub(TWOX);
                for delta in -2_i64..=2 {
                    check_frame2(at.wrapping_add_signed(delta), frame_ts, offset, half);
                }
            }
        }
    }
}

// --------------------------------------------------------------------------
// deinterlace_render()'s decisions, in its order

/// What `deinterlace_render()` decides for one call, each value as the C
/// returns it. `None` past the early return.
#[derive(Debug, PartialEq, Eq)]
struct Render {
    skip: u8,
    rest: Option<RenderRest>,
}

#[derive(Debug, PartialEq, Eq)]
struct RenderRest {
    reads_linear_srgb: u8,
    linear_srgb: u8,
    technique: u32,
    reads_white_level: u8,
    multiplier: u32,
    frame2: u8,
}

#[derive(Debug, Clone, Copy)]
struct RenderIn {
    cur: bool,
    prev: bool,
    width: u32,
    height: u32,
    source_space: c_int,
    gs_linear_srgb: bool,
    mode: c_int,
    current_space: c_int,
    white_bits: u32,
    video_time: u64,
    frame_ts: u64,
    offset: u64,
    half: u32,
}

/// The decisions as `deinterlace_render()` takes them: the graphics flag
/// and the white level are only read (passed on) when the decision before
/// says so.
fn render_c(i: RenderIn) -> Render {
    let skip = c_skip(i.cur, i.prev, i.width, i.height);
    if skip != 0 {
        return Render { skip, rest: None };
    }
    let reads_linear_srgb = c_reads_linear_srgb(i.source_space);
    let gs = reads_linear_srgb != 0 && i.gs_linear_srgb;
    let reads_white_level = c_reads_white_level(i.source_space, i.current_space);
    Render {
        skip,
        rest: Some(RenderRest {
            reads_linear_srgb,
            linear_srgb: c_linear_srgb(i.source_space, gs, i.mode),
            technique: c_technique(i.source_space, i.current_space),
            reads_white_level,
            multiplier: if reads_white_level != 0 {
                c_multiplier(
                    i.source_space,
                    i.current_space,
                    f32::from_bits(i.white_bits),
                )
            } else {
                1.0_f32.to_bits()
            },
            frame2: c_frame2(i.video_time, i.frame_ts, i.offset, i.half),
        }),
    }
}

fn render_rust(i: RenderIn) -> Render {
    let skip = byte(shim::obs_source_deinterlace_draw_skip(
        i.cur, i.prev, i.width, i.height,
    ));
    if skip != 0 {
        return Render { skip, rest: None };
    }
    let reads_linear_srgb = byte(shim::obs_source_deinterlace_draw_reads_linear_srgb(
        i.source_space,
    ));
    let gs = reads_linear_srgb != 0 && i.gs_linear_srgb;
    let reads_white_level = byte(shim::obs_source_deinterlace_draw_reads_white_level(
        i.source_space,
        i.current_space,
    ));
    Render {
        skip,
        rest: Some(RenderRest {
            reads_linear_srgb,
            linear_srgb: byte(shim::obs_source_deinterlace_draw_linear_srgb(
                i.source_space,
                gs,
                i.mode,
            )),
            technique: shim::obs_source_deinterlace_draw_technique(i.source_space, i.current_space),
            reads_white_level,
            multiplier: if reads_white_level != 0 {
                shim::obs_source_deinterlace_draw_multiplier(
                    i.source_space,
                    i.current_space,
                    f32::from_bits(i.white_bits),
                )
                .to_bits()
            } else {
                1.0_f32.to_bits()
            },
            frame2: byte(shim::obs_source_deinterlace_draw_frame2(
                i.video_time,
                i.frame_ts,
                i.offset,
                i.half,
            )),
        }),
    }
}

#[test]
fn render_decisions_every_space_mode_and_level() {
    let base = RenderIn {
        cur: true,
        prev: true,
        width: 1920,
        height: 1080,
        source_space: GS_CS_SRGB,
        gs_linear_srgb: false,
        mode: MODE_YADIF_2X,
        current_space: GS_CS_SRGB,
        white_bits: 0x4396_0000,
        video_time: 1_000_000_000_000,
        frame_ts: 5_000_000_000,
        offset: 995_000_000_000,
        half: 20_000_000,
    };
    let mut n = 0_usize;
    for source_space in -2..=6 {
        for current_space in -2..=6 {
            for gs_linear_srgb in [false, true] {
                for mode in -1..=10 {
                    for white_bits in WHITES {
                        let i = RenderIn {
                            source_space,
                            current_space,
                            gs_linear_srgb,
                            mode,
                            white_bits,
                            ..base
                        };
                        assert_eq!(render_rust(i), render_c(i), "{i:?}");
                        n += 1;
                    }
                }
            }
        }
    }
    for cur in [false, true] {
        for prev in [false, true] {
            for width in SIZES {
                for height in SIZES {
                    let i = RenderIn {
                        cur,
                        prev,
                        width,
                        height,
                        ..base
                    };
                    assert_eq!(render_rust(i), render_c(i), "{i:?}");
                    n += 1;
                }
            }
        }
    }
    assert!(n > 50_000, "{n} renders");
}

// --------------------------------------------------------------------------
// Proptests

fn any_space() -> impl Strategy<Value = c_int> {
    prop_oneof![
        0_i32..4,
        -2_i32..8,
        any::<i32>(),
        Just(i32::MIN),
        Just(i32::MAX)
    ]
}

fn any_mode() -> impl Strategy<Value = c_int> {
    prop_oneof![
        0_i32..9,
        -2_i32..12,
        any::<i32>(),
        Just(i32::MIN),
        Just(i32::MAX)
    ]
}

/// White levels as bit patterns: any `u32`, or one of the special ones.
fn any_white_bits() -> impl Strategy<Value = u32> {
    prop_oneof![any::<u32>(), proptest::sample::select(WHITES.to_vec())]
}

fn any_u64() -> impl Strategy<Value = u64> {
    prop_oneof![
        any::<u64>(),
        proptest::sample::select(U64S.to_vec()),
        0_u64..4_000_000,
        (u64::MAX - 4_000_000)..=u64::MAX,
    ]
}

fn any_half() -> impl Strategy<Value = u32> {
    prop_oneof![
        any::<u32>(),
        proptest::sample::select(HALVES.to_vec()),
        0_u32..2_000_000
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20_000))]

    #[test]
    fn prop_frame2(
        video_time in any_u64(),
        frame_ts in any_u64(),
        offset in any_u64(),
        half in any_half(),
        near in any::<bool>(),
        delta in -3_i64..=3,
    ) {
        let video_time = if near {
            frame_ts
                .wrapping_add(offset)
                .wrapping_add(u64::from(half))
                .wrapping_sub(TWOX)
                .wrapping_add_signed(delta)
        } else {
            video_time
        };
        prop_assert_eq!(
            byte(shim::obs_source_deinterlace_draw_frame2(video_time, frame_ts, offset, half)),
            c_frame2(video_time, frame_ts, offset, half)
        );
    }

    #[test]
    fn prop_multiplier(source in any_space(), current in any_space(), bits in any_white_bits()) {
        let white = f32::from_bits(bits);
        prop_assert_eq!(
            shim::obs_source_deinterlace_draw_multiplier(source, current, white).to_bits(),
            c_multiplier(source, current, white)
        );
        prop_assert_eq!(
            shim::obs_source_deinterlace_draw_technique(source, current),
            c_technique(source, current)
        );
    }

    #[test]
    fn prop_modes(current in any_mode(), mode in any_mode()) {
        prop_assert_eq!(shim::obs_source_deinterlace_draw_effect(mode), c_effect(mode));
        prop_assert_eq!(
            byte(shim::obs_source_deinterlace_draw_linear_required(mode)),
            c_linear_required(mode)
        );
        prop_assert_eq!(
            shim::obs_source_deinterlace_draw_mode_change(current, mode),
            c_mode_change(current, mode)
        );
    }

    #[test]
    fn prop_render_decisions(
        cur in prop::bool::weighted(0.9),
        prev in prop::bool::weighted(0.9),
        width in prop_oneof![Just(0_u32), any::<u32>(), 1_u32..4096],
        height in prop_oneof![Just(0_u32), any::<u32>(), 1_u32..4096],
        source_space in any_space(),
        gs_linear_srgb in any::<bool>(),
        mode in any_mode(),
        current_space in any_space(),
        white_bits in any_white_bits(),
        video_time in any_u64(),
        frame_ts in any_u64(),
        offset in any_u64(),
        half in any_half(),
    ) {
        let i = RenderIn {
            cur, prev, width, height, source_space, gs_linear_srgb, mode, current_space,
            white_bits, video_time, frame_ts, offset, half,
        };
        prop_assert_eq!(render_rust(i), render_c(i));
    }
}
