//! Tier 1: `obs_output::encoder_gpu_rescale`, the encoder GPU rescale mix
//! decisions.
//!
//! The `steps_*` cases port `test/cmocka/test_encoder_gpu_rescale_steps.c`
//! 1:1: its 13 cases under the same names, in the same order, with the same
//! values and expectations, the "characterized, not endorsed" ones included.
//! The C calls the hidden `obs_encoder_gpu_rescale_*` functions; here the
//! safe core is called directly ([`wanted`], [`target`], [`matches`],
//! [`release_step`], [`release_refs`]).
//!
//! The `edge_*` cases are new: requests over a grid of scale types, scaled
//! sizes at the `u32` edges and enum values from -2 to 3 and the `i32`
//! extremes, against two video outputs; every match condition changed on
//! its own and in pairs; every release input; reference counts at the `i64`
//! edges; and the constants. Each checks the core against a model of the
//! original C.

use obs_output::encoder_gpu_rescale::{
    OBS_SCALE_DISABLE, Output, Release, Request, VIDEO_CS_DEFAULT, VIDEO_FORMAT_NONE,
    VIDEO_RANGE_DEFAULT, matches, release_refs, release_step, target, wanted,
};

const DISABLE: i32 = OBS_SCALE_DISABLE;
const FORMAT_NONE: i32 = VIDEO_FORMAT_NONE;
const CS_DEFAULT: i32 = VIDEO_CS_DEFAULT;
const RANGE_DEFAULT: i32 = VIDEO_RANGE_DEFAULT;

/// `OBS_SCALE_BICUBIC`; `VIDEO_FORMAT_NV12`, `VIDEO_FORMAT_I420`;
/// `VIDEO_CS_709`, `VIDEO_CS_601`; `VIDEO_RANGE_PARTIAL`, `VIDEO_RANGE_FULL`.
const BICUBIC: i32 = 2;
const NV12: i32 = 2;
const I420: i32 = 1;
const CS_709: i32 = 2;
const CS_601: i32 = 1;
const PARTIAL: i32 = 1;
const FULL: i32 = 2;

/// Enum values the C does not range-check.
const ODD_ENUMS: [i32; 4] = [-1, 99, i32::MAX, i32::MIN];

const EDGE_SIZES: [u32; 7] = [1, 2, 720, 1080, 0x7fff_ffff, 0x8000_0000, u32::MAX];

fn request(
    scale_type: i32,
    scaled_width: u32,
    scaled_height: u32,
    format: i32,
    colorspace: i32,
    range: i32,
) -> Request {
    Request {
        scale_type,
        scaled_width,
        scaled_height,
        format,
        colorspace,
        range,
    }
}

fn output(width: u32, height: u32, format: i32, colorspace: i32, range: i32) -> Output {
    Output {
        width,
        height,
        format,
        colorspace,
        range,
    }
}

fn expect_wanted(has_media: bool, r: Request, want: bool) {
    assert_eq!(
        wanted(has_media, &r),
        want,
        "wanted(media {has_media}, {r:?})"
    );
}

fn expect_target(r: Request, video: Output, want: Output) {
    assert_eq!(target(&r, &video), want, "target({r:?}, video {video:?})");
}

fn expect_match(
    same_view: bool,
    mix_scale_type: i32,
    mix: Output,
    scale_type: i32,
    t: Output,
    want: bool,
) {
    assert_eq!(
        matches(same_view, mix_scale_type, &mix, scale_type, &t),
        want,
        "matches(view {same_view}, scale {mix_scale_type}, mix {mix:?}, scale {scale_type}, target {t:?})"
    );
}

fn expect_refs(refs: i64, want: i64, want_last: bool) {
    assert_eq!(
        release_refs(refs),
        (want, want_last),
        "release_refs({refs})"
    );
}

// -------------------------------------------------------------------------
// test_encoder_gpu_rescale_steps.c, 1:1

#[test]
fn steps_wanted_no_media() {
    expect_wanted(
        false,
        request(BICUBIC, 1280, 720, NV12, CS_709, PARTIAL),
        false,
    );
    expect_wanted(
        false,
        request(BICUBIC, 0, 0, FORMAT_NONE, CS_DEFAULT, RANGE_DEFAULT),
        false,
    );
}

#[test]
fn steps_wanted_scale_disabled() {
    expect_wanted(
        true,
        request(DISABLE, 1280, 720, NV12, CS_709, PARTIAL),
        false,
    );
    expect_wanted(
        true,
        request(DISABLE, 0, 0, FORMAT_NONE, CS_DEFAULT, RANGE_DEFAULT),
        false,
    );
}

#[test]
fn steps_wanted_nothing_requested() {
    expect_wanted(
        true,
        request(BICUBIC, 0, 0, FORMAT_NONE, CS_DEFAULT, RANGE_DEFAULT),
        false,
    );
}

#[test]
fn steps_wanted_each_request_alone() {
    expect_wanted(
        true,
        request(BICUBIC, 1280, 0, FORMAT_NONE, CS_DEFAULT, RANGE_DEFAULT),
        true,
    );
    expect_wanted(
        true,
        request(BICUBIC, 0, 720, FORMAT_NONE, CS_DEFAULT, RANGE_DEFAULT),
        true,
    );
    expect_wanted(
        true,
        request(BICUBIC, 0, 0, I420, CS_DEFAULT, RANGE_DEFAULT),
        true,
    );
    expect_wanted(
        true,
        request(BICUBIC, 0, 0, FORMAT_NONE, CS_601, RANGE_DEFAULT),
        true,
    );
    expect_wanted(
        true,
        request(BICUBIC, 0, 0, FORMAT_NONE, CS_DEFAULT, FULL),
        true,
    );
    for s in EDGE_SIZES {
        expect_wanted(
            true,
            request(BICUBIC, s, 0, FORMAT_NONE, CS_DEFAULT, RANGE_DEFAULT),
            true,
        );
        expect_wanted(
            true,
            request(BICUBIC, 0, s, FORMAT_NONE, CS_DEFAULT, RANGE_DEFAULT),
            true,
        );
    }
}

/// Characterized, not endorsed: out-of-range enum values count as set, and
/// any scale type other than `OBS_SCALE_DISABLE` passes.
#[test]
fn steps_wanted_odd_enums() {
    for v in ODD_ENUMS {
        expect_wanted(
            true,
            request(v, 1280, 720, FORMAT_NONE, CS_DEFAULT, RANGE_DEFAULT),
            true,
        );
        expect_wanted(
            true,
            request(v, 0, 0, FORMAT_NONE, CS_DEFAULT, RANGE_DEFAULT),
            false,
        );
        expect_wanted(
            true,
            request(BICUBIC, 0, 0, v, CS_DEFAULT, RANGE_DEFAULT),
            true,
        );
        expect_wanted(
            true,
            request(BICUBIC, 0, 0, FORMAT_NONE, v, RANGE_DEFAULT),
            true,
        );
        expect_wanted(
            true,
            request(BICUBIC, 0, 0, FORMAT_NONE, CS_DEFAULT, v),
            true,
        );
        expect_wanted(false, request(v, 1280, 720, v, v, v), false);
    }
}

#[test]
fn steps_target_from_video() {
    let video = output(1920, 1080, NV12, CS_709, PARTIAL);
    expect_target(
        request(BICUBIC, 0, 0, FORMAT_NONE, CS_DEFAULT, RANGE_DEFAULT),
        video,
        video,
    );
}

#[test]
fn steps_target_from_request() {
    expect_target(
        request(BICUBIC, 1280, 720, I420, CS_601, FULL),
        output(1920, 1080, NV12, CS_709, PARTIAL),
        output(1280, 720, I420, CS_601, FULL),
    );
}

#[test]
fn steps_target_each_field() {
    let video = output(1920, 1080, NV12, CS_709, PARTIAL);
    expect_target(
        request(BICUBIC, 1280, 0, FORMAT_NONE, CS_DEFAULT, RANGE_DEFAULT),
        video,
        output(1280, 1080, NV12, CS_709, PARTIAL),
    );
    expect_target(
        request(BICUBIC, 0, 720, FORMAT_NONE, CS_DEFAULT, RANGE_DEFAULT),
        video,
        output(1920, 720, NV12, CS_709, PARTIAL),
    );
    expect_target(
        request(BICUBIC, 0, 0, I420, CS_DEFAULT, RANGE_DEFAULT),
        video,
        output(1920, 1080, I420, CS_709, PARTIAL),
    );
    expect_target(
        request(BICUBIC, 0, 0, FORMAT_NONE, CS_601, RANGE_DEFAULT),
        video,
        output(1920, 1080, NV12, CS_601, PARTIAL),
    );
    expect_target(
        request(BICUBIC, 0, 0, FORMAT_NONE, CS_DEFAULT, FULL),
        video,
        output(1920, 1080, NV12, CS_709, FULL),
    );
}

#[test]
fn steps_target_edges() {
    let video = output(1920, 1080, NV12, CS_709, PARTIAL);
    for s in EDGE_SIZES {
        expect_target(
            request(BICUBIC, s, s, FORMAT_NONE, CS_DEFAULT, RANGE_DEFAULT),
            video,
            output(s, s, NV12, CS_709, PARTIAL),
        );
        expect_target(
            request(DISABLE, 0, 0, FORMAT_NONE, CS_DEFAULT, RANGE_DEFAULT),
            output(s, s, NV12, 0, 0),
            output(s, s, NV12, 0, 0),
        );
    }
    for v in ODD_ENUMS {
        expect_target(
            request(v, 0, 0, v, v, v),
            video,
            output(1920, 1080, v, v, v),
        );
    }
    expect_target(
        request(BICUBIC, 0, 0, FORMAT_NONE, CS_DEFAULT, RANGE_DEFAULT),
        output(0, 0, 0, 0, 0),
        output(0, 0, 0, 0, 0),
    );
}

#[test]
fn steps_matches_all_equal() {
    let t = output(1280, 720, NV12, CS_709, PARTIAL);
    expect_match(true, BICUBIC, t, BICUBIC, t, true);
    for s in EDGE_SIZES {
        let e = output(s, s, -1, i32::MAX, i32::MIN);
        expect_match(true, -1, e, -1, e, true);
    }
}

#[test]
fn steps_matches_each_difference() {
    let t = output(1280, 720, NV12, CS_709, PARTIAL);
    expect_match(false, BICUBIC, t, BICUBIC, t, false);
    expect_match(true, BICUBIC + 1, t, BICUBIC, t, false);
    expect_match(true, DISABLE, t, BICUBIC, t, false);
    expect_match(
        true,
        BICUBIC,
        output(1281, 720, NV12, CS_709, PARTIAL),
        BICUBIC,
        t,
        false,
    );
    expect_match(
        true,
        BICUBIC,
        output(1280, 721, NV12, CS_709, PARTIAL),
        BICUBIC,
        t,
        false,
    );
    expect_match(
        true,
        BICUBIC,
        output(720, 1280, NV12, CS_709, PARTIAL),
        BICUBIC,
        t,
        false,
    );
    expect_match(
        true,
        BICUBIC,
        output(1280, 720, I420, CS_709, PARTIAL),
        BICUBIC,
        t,
        false,
    );
    expect_match(
        true,
        BICUBIC,
        output(1280, 720, NV12, CS_601, PARTIAL),
        BICUBIC,
        t,
        false,
    );
    expect_match(
        true,
        BICUBIC,
        output(1280, 720, NV12, CS_709, FULL),
        BICUBIC,
        t,
        false,
    );
    expect_match(true, BICUBIC, output(0, 0, 0, 0, 0), BICUBIC, t, false);
}

#[test]
fn steps_release_step_each_input() {
    assert_eq!(release_step(false, false), Release::Skip);
    assert_eq!(release_step(false, true), Release::Skip);
    assert_eq!(release_step(true, false), Release::Stop);
    assert_eq!(release_step(true, true), Release::Drop);
}

/// Characterized, not endorsed: a count at 0 goes to -1 and is not "last".
#[test]
fn steps_release_refs_counts() {
    expect_refs(1, 0, true);
    expect_refs(2, 1, false);
    expect_refs(3, 2, false);
    expect_refs(0, -1, false);
    expect_refs(-1, -2, false);
    expect_refs(i64::MAX, i64::MAX - 1, false);
}

// -------------------------------------------------------------------------
// New: the core against a model of the original C

const GRID_ENUMS: [i32; 8] = [-2, -1, 0, 1, 2, 3, i32::MAX, i32::MIN];
const GRID_SIZES: [u32; 5] = [0, 1, 1080, 0x8000_0000, u32::MAX];

fn model_wanted(has_media: bool, r: &Request) -> bool {
    if !has_media {
        return false;
    }
    if r.scale_type == 0 {
        return false;
    }
    !(r.scaled_height == 0
        && r.scaled_width == 0
        && r.format == 0
        && r.colorspace == 0
        && r.range == 0)
}

fn model_target(r: &Request, v: &Output) -> Output {
    Output {
        width: if r.scaled_width != 0 {
            r.scaled_width
        } else {
            v.width
        },
        height: if r.scaled_height != 0 {
            r.scaled_height
        } else {
            v.height
        },
        format: if r.format != 0 { r.format } else { v.format },
        colorspace: if r.colorspace != 0 {
            r.colorspace
        } else {
            v.colorspace
        },
        range: if r.range != 0 { r.range } else { v.range },
    }
}

fn grid_requests() -> impl Iterator<Item = Request> {
    GRID_ENUMS.into_iter().flat_map(|scale_type| {
        GRID_SIZES.into_iter().flat_map(move |scaled_width| {
            GRID_SIZES.into_iter().flat_map(move |scaled_height| {
                GRID_ENUMS.into_iter().flat_map(move |format| {
                    [0, 1, -1, i32::MIN]
                        .into_iter()
                        .flat_map(move |colorspace| {
                            [0, 2, -1, i32::MAX].into_iter().map(move |range| Request {
                                scale_type,
                                scaled_width,
                                scaled_height,
                                format,
                                colorspace,
                                range,
                            })
                        })
                })
            })
        })
    })
}

#[test]
fn edge_wanted_and_target_model() {
    let videos = [
        output(1920, 1080, NV12, CS_709, PARTIAL),
        output(0, 0, 0, 0, 0),
        output(u32::MAX, 1, -1, i32::MAX, i32::MIN),
    ];
    let mut cases = 0_u32;
    for r in grid_requests() {
        for has_media in [false, true] {
            assert_eq!(
                wanted(has_media, &r),
                model_wanted(has_media, &r),
                "wanted(media {has_media}, {r:?})"
            );
        }
        for v in &videos {
            assert_eq!(target(&r, v), model_target(&r, v), "target({r:?}, {v:?})");
        }
        cases = cases.saturating_add(1);
    }
    assert_eq!(cases, 8 * 5 * 5 * 8 * 4 * 4);
}

/// Each of the eight conditions changed alone refuses, and every pair of
/// changes refuses too; nothing changed matches.
#[test]
fn edge_matches_model() {
    let targets = [
        output(1280, 720, NV12, CS_709, PARTIAL),
        output(0, 0, 0, 0, 0),
        output(u32::MAX, u32::MAX, i32::MAX, i32::MIN, -1),
    ];
    let change = |o: Output, k: u32| -> (bool, i32, Output) {
        let mut m = o;
        let mut view = true;
        let mut scale = BICUBIC;
        match k {
            0 => view = false,
            1 => scale = BICUBIC.wrapping_add(1),
            2 => m.width = m.width.wrapping_add(1),
            3 => m.height = m.height.wrapping_add(1),
            4 => m.format = m.format.wrapping_add(1),
            5 => m.colorspace = m.colorspace.wrapping_add(1),
            6 => m.range = m.range.wrapping_add(1),
            7 => scale = DISABLE,
            _ => {}
        }
        (view, scale, m)
    };
    for t in targets {
        expect_match(true, BICUBIC, t, BICUBIC, t, true);
        for a in 0..8 {
            let (view, scale, m) = change(t, a);
            expect_match(view, scale, m, BICUBIC, t, false);
            for b in 0..8 {
                let (view_b, scale_b, mb) = change(m, b);
                let view_ab = view && view_b;
                let scale_ab = if b == 1 || b == 7 { scale_b } else { scale };
                expect_match(view_ab, scale_ab, mb, BICUBIC, t, false);
            }
        }
    }
}

#[test]
fn edge_release_model() {
    for is_media in [false, true] {
        for encoder_only in [false, true] {
            let want = if !is_media {
                Release::Skip
            } else if !encoder_only {
                Release::Stop
            } else {
                Release::Drop
            };
            assert_eq!(
                release_step(is_media, encoder_only),
                want,
                "release_step({is_media}, {encoder_only})"
            );
        }
    }
    for refs in [
        i64::MIN.wrapping_add(1),
        -2,
        -1,
        0,
        1,
        2,
        1000,
        i64::from(i32::MAX),
        i64::MAX,
    ] {
        let after = refs.wrapping_sub(1);
        assert_eq!(
            release_refs(refs),
            (after, after == 0),
            "release_refs({refs})"
        );
    }
}

#[test]
fn edge_constants() {
    assert_eq!(OBS_SCALE_DISABLE, 0);
    assert_eq!(VIDEO_FORMAT_NONE, 0);
    assert_eq!(VIDEO_CS_DEFAULT, 0);
    assert_eq!(VIDEO_RANGE_DEFAULT, 0);
}
