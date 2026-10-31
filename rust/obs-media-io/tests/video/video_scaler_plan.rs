//! Tier 1: the safe core of `libobs/media-io/video-scaler-plan.c.h`.
//!
//! The `steps_*` tests port `test/cmocka/test_video_scaler_plan_steps.c` one
//! to one: the same cases under the same names, in the same order, with the
//! same values, the characterized-not-endorsed ones included.
//!
//! The `edge_*` tests and the proptests check the core over the edges of each
//! input: every value of the three enums and the values around them, heights
//! at the ends of `uint32_t`, every chroma shift FFmpeg has and the ones it
//! doesn't, component planes outside 0 to 3, how often the chroma shift is
//! read, and line sizes and heights at the ends of `size_t`.
//!
//! Intentional difference from the C (`plane_heights`): a shifted chroma
//! plane's height rounds up, as `av_image_alloc()` and `sws_scale()` size it
//! and `video_frame_init()` allocates it, where the C rounded down and left
//! the output's last chroma row unwritten at an odd height
//! (`edge_odd_height_chroma_rounds_up`).

use core::ffi::c_int;

use obs_media_io::video_scaler_plan::{
    PlaneCopy, plane_copy, plane_heights, sws, sws_colorspace, sws_flags, sws_range,
};
use proptest::prelude::*;

/// `UNSET` of the C test: a height a plane the format does not have keeps.
const UNSET: c_int = -7;

struct Layout {
    name: &'static str,
    comp_planes: [c_int; 4],
    log2_chroma_h: u8,
}

const YUV420P: Layout = Layout {
    name: "yuv420p",
    comp_planes: [0, 1, 2, 0],
    log2_chroma_h: 1,
};
const YUV422P: Layout = Layout {
    name: "yuv422p",
    comp_planes: [0, 1, 2, 0],
    log2_chroma_h: 0,
};
const YUV444P: Layout = Layout {
    name: "yuv444p",
    comp_planes: [0, 1, 2, 0],
    log2_chroma_h: 0,
};
const NV12: Layout = Layout {
    name: "nv12",
    comp_planes: [0, 1, 1, 0],
    log2_chroma_h: 1,
};
const P010: Layout = Layout {
    name: "p010le",
    comp_planes: [0, 1, 1, 0],
    log2_chroma_h: 1,
};
const YUVA420P: Layout = Layout {
    name: "yuva420p",
    comp_planes: [0, 1, 2, 3],
    log2_chroma_h: 1,
};
const YUVA444P: Layout = Layout {
    name: "yuva444p",
    comp_planes: [0, 1, 2, 3],
    log2_chroma_h: 0,
};
const RGBA: Layout = Layout {
    name: "rgba",
    comp_planes: [0, 0, 0, 0],
    log2_chroma_h: 0,
};
const YUYV422: Layout = Layout {
    name: "yuyv422",
    comp_planes: [0, 0, 0, 0],
    log2_chroma_h: 0,
};

/// The heights as `video_scaler_create()` stores them: its zeroed array, or
/// the C test's `UNSET`, written only where the core gives a height.
fn heights_into(start: c_int, height: u32, l: &Layout) -> [c_int; 4] {
    let mut out = [start; 4];
    for (slot, h) in out
        .iter_mut()
        .zip(plane_heights(height, l.comp_planes, || l.log2_chroma_h))
    {
        if let Some(h) = h {
            *slot = h;
        }
    }
    out
}

fn check_heights(l: &Layout, height: u32, want: [c_int; 4]) {
    assert_eq!(
        heights_into(UNSET, height, l),
        want,
        "{} at {height}",
        l.name
    );
}

fn check_copy(scaled: usize, plane: usize, height: usize, whole: bool, size: usize) {
    let want = if whole {
        PlaneCopy::Whole { size }
    } else {
        PlaneCopy::Rows { row: size }
    };
    assert_eq!(
        plane_copy(scaled, plane, height),
        want,
        "copy {scaled} to {plane}, {height} rows"
    );
}

#[test]
fn steps_constants() {
    assert_eq!(sws::FAST_BILINEAR, 0x01);
    assert_eq!(sws::BILINEAR, 0x02);
    assert_eq!(sws::BICUBIC, 0x04);
    assert_eq!(sws::POINT, 0x10);
    assert_eq!(sws::AREA, 0x20);
    assert_eq!(sws::CS_ITU709, 1);
    assert_eq!(sws::CS_ITU601, 5);
    assert_eq!(sws::CS_BT2020, 9);
}

#[test]
fn steps_scale_flags() {
    assert_eq!(sws_flags(0), sws::FAST_BILINEAR);
    assert_eq!(sws_flags(1), sws::POINT);
    assert_eq!(sws_flags(2), sws::FAST_BILINEAR);
    assert_eq!(sws_flags(3), sws::BILINEAR | sws::AREA);
    assert_eq!(sws_flags(4), sws::BICUBIC);
    // characterized, not endorsed: anything outside the enum is point
    assert_eq!(sws_flags(5), sws::POINT);
    assert_eq!(sws_flags(-1), sws::POINT);
}

#[test]
fn steps_colorspaces() {
    assert_eq!(sws_colorspace(0), sws::CS_ITU709);
    assert_eq!(sws_colorspace(1), sws::CS_ITU601);
    assert_eq!(sws_colorspace(2), sws::CS_ITU709);
    assert_eq!(sws_colorspace(3), sws::CS_ITU709);
    assert_eq!(sws_colorspace(4), sws::CS_BT2020);
    assert_eq!(sws_colorspace(5), sws::CS_BT2020);
    // anything outside the enum is 709
    assert_eq!(sws_colorspace(6), sws::CS_ITU709);
    assert_eq!(sws_colorspace(-1), sws::CS_ITU709);
}

#[test]
fn steps_ranges() {
    assert_eq!(sws_range(0), 0);
    assert_eq!(sws_range(1), 0);
    assert_eq!(sws_range(2), 1);
    // anything outside the enum is partial
    assert_eq!(sws_range(3), 0);
    assert_eq!(sws_range(-1), 0);
}

#[test]
fn steps_plane_heights_planar() {
    check_heights(&YUV420P, 720, [720, 360, 360, UNSET]);
    check_heights(&YUV420P, 1080, [1080, 540, 540, UNSET]);
    check_heights(&YUV422P, 720, [720, 720, 720, UNSET]);
    check_heights(&YUV422P, 721, [721, 721, 721, UNSET]);
    check_heights(&YUV444P, 1080, [1080, 1080, 1080, UNSET]);
    check_heights(&YUV444P, 1, [1, 1, 1, UNSET]);
}

#[test]
fn steps_plane_heights_semi_planar() {
    check_heights(&NV12, 720, [720, 360, UNSET, UNSET]);
    check_heights(&NV12, 2160, [2160, 1080, UNSET, UNSET]);
    check_heights(&P010, 1080, [1080, 540, UNSET, UNSET]);
}

#[test]
fn steps_plane_heights_alpha() {
    // the alpha plane is never shifted
    check_heights(&YUVA420P, 720, [720, 360, 360, 720]);
    check_heights(&YUVA444P, 720, [720, 720, 720, 720]);
    check_heights(&YUVA444P, 719, [719, 719, 719, 719]);
}

#[test]
fn steps_plane_heights_packed() {
    check_heights(&RGBA, 720, [720, UNSET, UNSET, UNSET]);
    check_heights(&RGBA, 721, [721, UNSET, UNSET, UNSET]);
    check_heights(&YUYV422, 1080, [1080, UNSET, UNSET, UNSET]);
}

#[test]
fn steps_plane_heights_zero() {
    check_heights(&YUV420P, 0, [0, 0, 0, UNSET]);
    check_heights(&YUVA420P, 0, [0, 0, 0, 0]);
    check_heights(&RGBA, 0, [0, UNSET, UNSET, UNSET]);
}

#[test]
fn steps_chroma_shift_read_only_for_chroma() {
    // With no plane 1 or 2, the shift is never read.
    let unread = || -> u8 { panic!("the chroma shift was read") };
    assert_eq!(
        plane_heights(480, [0, 0, 0, 0], unread),
        [Some(480), None, None, None]
    );
    assert_eq!(
        plane_heights(480, [0, 0, 0, 3], unread),
        [Some(480), None, None, Some(480)]
    );
}

#[test]
fn steps_chroma_shift_by_two() {
    // yuv410p: a quarter of the height
    let yuv410p = Layout {
        name: "yuv410p",
        comp_planes: [0, 1, 2, 0],
        log2_chroma_h: 2,
    };
    check_heights(&yuv410p, 720, [720, 180, 180, UNSET]);
}

#[test]
fn steps_copy_equal_linesizes() {
    check_copy(1920, 1920, 1080, true, 1920 * 1080);
    check_copy(960, 960, 540, true, 960 * 540);
    check_copy(1, 1, 1, true, 1);
}

#[test]
fn steps_copy_wider_scaled() {
    // the scaled plane is padded to 32 bytes: one row of the output's width
    check_copy(1952, 1940, 1080, false, 1940);
    check_copy(992, 970, 540, false, 970);
}

#[test]
fn steps_copy_wider_output() {
    // the output plane is wider: one row of the scaled width
    check_copy(1920, 2048, 1080, false, 1920);
    check_copy(32, 4096, 1, false, 32);
}

#[test]
fn steps_copy_zero_height() {
    check_copy(1920, 1920, 0, true, 0);
    check_copy(1952, 1920, 0, false, 1920);
    check_copy(0, 0, 720, true, 0);
    check_copy(0, 64, 720, false, 0);
}

/// The heights the port gives: the C's, with a shifted plane's height rounded
/// up (`AV_CEIL_RSHIFT`), each cast to `int` as the C casts it.
fn model_heights(height: u32, comp_planes: [c_int; 4], shift: u8) -> [Option<c_int>; 4] {
    let mut has = [false; 4];
    for p in comp_planes {
        if let Ok(p) = usize::try_from(p)
            && let Some(slot) = has.get_mut(p)
        {
            *slot = true;
        }
    }
    let ceil = |s: u32| -> c_int {
        // a shift past the height's 32 bits leaves 1 of any height but 0
        let h = u64::from(height);
        let v = if s >= 32 {
            u64::from(h != 0)
        } else {
            (h + (1u64 << s) - 1) >> s
        };
        u32::try_from(v).map_or(0, |v| v as c_int)
    };
    let mut out = [None; 4];
    out[0] = Some(height as c_int);
    for i in 1..4 {
        if has[i] {
            let s = if i == 1 || i == 2 {
                u32::from(shift)
            } else {
                0
            };
            out[i] = Some(ceil(s));
        }
    }
    out
}

#[test]
fn edge_scale_flags_every_value() {
    let want = [
        sws::FAST_BILINEAR,
        sws::POINT,
        sws::FAST_BILINEAR,
        sws::BILINEAR | sws::AREA,
        sws::BICUBIC,
    ];
    for v in -3..=12 {
        let w = usize::try_from(v)
            .ok()
            .and_then(|i| want.get(i))
            .copied()
            .unwrap_or(sws::POINT);
        assert_eq!(sws_flags(v), w, "scale type {v}");
    }
    for v in [c_int::MIN, c_int::MIN + 1, c_int::MAX - 1, c_int::MAX] {
        assert_eq!(sws_flags(v), sws::POINT, "scale type {v}");
    }
}

#[test]
fn edge_colorspaces_every_value() {
    let want = [
        sws::CS_ITU709,
        sws::CS_ITU601,
        sws::CS_ITU709,
        sws::CS_ITU709,
        sws::CS_BT2020,
        sws::CS_BT2020,
    ];
    for v in -3..=12 {
        let w = usize::try_from(v)
            .ok()
            .and_then(|i| want.get(i))
            .copied()
            .unwrap_or(sws::CS_ITU709);
        assert_eq!(sws_colorspace(v), w, "colorspace {v}");
    }
    for v in [c_int::MIN, c_int::MAX] {
        assert_eq!(sws_colorspace(v), sws::CS_ITU709, "colorspace {v}");
    }
}

#[test]
fn edge_ranges_every_value() {
    for v in -3..=12 {
        assert_eq!(sws_range(v), c_int::from(v == 2), "range {v}");
    }
    for v in [c_int::MIN, c_int::MAX] {
        assert_eq!(sws_range(v), 0, "range {v}");
    }
}

#[test]
fn edge_odd_height_chroma_rounds_up() {
    // The intentional difference: the C gave 4:2:0 chroma 360 rows at 721
    // and left the output's 361st unwritten.
    check_heights(&YUV420P, 721, [721, 361, 361, UNSET]);
    check_heights(&NV12, 1, [1, 1, UNSET, UNSET]);
    check_heights(&P010, 1081, [1081, 541, UNSET, UNSET]);
    check_heights(&YUVA420P, 719, [719, 360, 360, 719]);
    let yuv410p = Layout {
        name: "yuv410p",
        comp_planes: [0, 1, 2, 0],
        log2_chroma_h: 2,
    };
    check_heights(&yuv410p, 721, [721, 181, 181, UNSET]);
    check_heights(&yuv410p, 1, [1, 1, 1, UNSET]);
}

#[test]
fn edge_heights_at_the_ends_of_uint32() {
    for height in [
        i32::MAX as u32 - 1,
        i32::MAX as u32,
        i32::MAX as u32 + 1,
        u32::MAX - 1,
        u32::MAX,
    ] {
        for l in [&YUV420P, &NV12, &YUVA420P, &YUV444P, &RGBA] {
            assert_eq!(
                plane_heights(height, l.comp_planes, || l.log2_chroma_h),
                model_heights(height, l.comp_planes, l.log2_chroma_h),
                "{} at {height}",
                l.name
            );
        }
    }
    // plane 0 is cast as the C casts it
    assert_eq!(plane_heights(u32::MAX, [0; 4], || 0)[0], Some(-1));
    // rounded up from UINT32_MAX without overflow: 2^31, cast to INT_MIN
    assert_eq!(
        plane_heights(u32::MAX, [0, 1, 2, 0], || 1)[1],
        Some(c_int::MIN)
    );
}

#[test]
fn edge_every_shift() {
    // FFmpeg's largest vertical shift is 2; the others are defined too.
    for shift in 0..=u8::MAX {
        for height in [0, 1, 2, 3, 719, 720, 721, 1 << 20, u32::MAX] {
            assert_eq!(
                plane_heights(height, [0, 1, 2, 3], || shift),
                model_heights(height, [0, 1, 2, 3], shift),
                "shift {shift} at {height}"
            );
        }
    }
}

#[test]
fn edge_comp_planes_outside_the_descriptor() {
    // The C indexes has_plane[] with them; the port ignores them.
    for bad in [-1, 4, 5, c_int::MIN, c_int::MAX] {
        assert_eq!(
            plane_heights(720, [0, bad, 2, bad], || 1),
            [Some(720), None, Some(360), None],
            "plane {bad}"
        );
    }
}

#[test]
fn edge_chroma_shift_read_once_per_chroma_plane() {
    for (planes, reads) in [
        ([0, 1, 2, 0], 2),
        ([0, 1, 2, 3], 2),
        ([0, 1, 1, 0], 1),
        ([0, 2, 2, 0], 1),
        ([0, 0, 0, 3], 0),
        ([0, 0, 0, 0], 0),
        ([3, 2, 1, 0], 2),
    ] {
        let mut n = 0;
        plane_heights(720, planes, || {
            n += 1;
            1
        });
        assert_eq!(n, reads, "{planes:?}");
    }
}

#[test]
fn edge_copy_at_the_ends_of_size_t() {
    check_copy(usize::MAX, usize::MAX, 1, true, usize::MAX);
    // the size wraps as size_t does
    check_copy(usize::MAX, usize::MAX, 2, true, usize::MAX - 1);
    check_copy(
        1 << 40,
        1 << 40,
        1 << 30,
        true,
        (1usize << 40).wrapping_mul(1 << 30),
    );
    check_copy(usize::MAX, 0, usize::MAX, false, 0);
    check_copy(0, usize::MAX, usize::MAX, false, 0);
    check_copy(usize::MAX - 1, usize::MAX, 7, false, usize::MAX - 1);
}

proptest! {
    #[test]
    fn prop_plane_heights(
        height in any::<u32>(),
        comp_planes in prop::array::uniform4(-1..5i32),
        shift in 0..=3u8,
    ) {
        prop_assert_eq!(
            plane_heights(height, comp_planes, || shift),
            model_heights(height, comp_planes, shift)
        );
    }

    #[test]
    fn prop_plane_copy(
        scaled in 0..=16384usize,
        plane in 0..=16384usize,
        height in 0..=4100usize,
    ) {
        let want = if scaled == plane {
            PlaneCopy::Whole { size: scaled * height }
        } else {
            PlaneCopy::Rows { row: scaled.min(plane) }
        };
        prop_assert_eq!(plane_copy(scaled, plane, height), want);
    }
}
