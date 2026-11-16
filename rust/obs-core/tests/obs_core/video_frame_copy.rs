//! Tier 1: `obs_core::video_frame_copy`, the output frame copy of
//! `libobs/obs-video-frame-copy.c.h`.
//!
//! The `frames_*` cases port `test/cmocka/test_video_frame_copy.c` over byte
//! buffers: its formats, GPU conversion on and off, its two sizes (64x32 and
//! 68x34, I420 only at 64x32) and its line sizes of the output frame. Each
//! input plane is painted as the format lays it out, one value per quadrant,
//! plane and sample, and each output pixel must hold its quadrant's value.
//! The C test runs only the staging surfaces OpenGL gives (line sizes equal
//! to the row, so every plane is one whole copy); these also run the padded
//! line sizes of Direct3D (a row at a time) and the packed NV12 and P010
//! frames whose UV plane follows plane 0 (input `linesize[1]` of 0), which
//! the C test cannot reach on Linux. RGBA, BGRA and BGRX have no GPU
//! conversion, so libobs turns it off for them (`obs.c`'s
//! `set_video_conversion()`): both of their cases run `copy_rgbx_frame()`.
//!
//! The rest are new: the whole-plane copy, the row copy at different
//! strides, the lengths and the advance, short slices, the loop bound and
//! the plane after plane 0, and proptests of [`copy_plane`] against a model
//! written from the C's statements.

use obs_core::video_copy_plan::{
    COPY_MAX, CopyPlan, FrameCopy, LineSizes, gpu_copy_plan, rgbx_copy_plan,
};
use obs_core::video_frame_copy::{
    Source, Step, copy_frame, copy_plane, copy_rgbx_frame, dst_len, line_sizes_read, src_advance,
    src_len, steps,
};
use obs_media_io::video_io::VideoFormat;
use proptest::prelude::*;

/// What a destination byte holds before the copy.
const UNTOUCHED: u8 = 0xEE;

fn whole(bytes: usize) -> FrameCopy {
    FrameCopy {
        src_after_plane0: false,
        whole: true,
        bytes,
        rows: 0,
        src_stride: 0,
        dst_stride: 0,
    }
}

fn rows(bytes: usize, rows: u32, src_stride: u32, dst_stride: u32) -> FrameCopy {
    FrameCopy {
        src_after_plane0: false,
        whole: false,
        bytes,
        rows,
        src_stride,
        dst_stride,
    }
}

fn counting(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i % 251) as u8).collect()
}

// ---------------------------------------------------------------------------
// one copy (set_gpu_converted_plane)

#[test]
fn plane_whole_copy() {
    let src = counting(40);
    let mut dst = vec![UNTOUCHED; 48];
    assert_eq!(copy_plane(&whole(40), &src, &mut dst), Some(40));
    assert_eq!(&dst[..40], &src[..]);
    assert!(dst[40..].iter().all(|&b| b == UNTOUCHED));
}

#[test]
fn plane_whole_copy_ignores_rows_and_strides() {
    let copy = FrameCopy {
        rows: 7,
        src_stride: 3,
        dst_stride: 100,
        ..whole(12)
    };
    let src = counting(12);
    let mut dst = vec![UNTOUCHED; 12];
    assert_eq!(copy_plane(&copy, &src, &mut dst), Some(12));
    assert_eq!(dst, src);
}

#[test]
fn plane_rows_at_different_strides() {
    // 3 rows of 8 bytes, read every 10 and written every 12
    let copy = rows(8, 3, 10, 12);
    let src = counting(30);
    let mut dst = vec![UNTOUCHED; 36];
    assert_eq!(copy_plane(&copy, &src, &mut dst), Some(30));
    for y in 0..3 {
        assert_eq!(
            &dst[y * 12..y * 12 + 8],
            &src[y * 10..y * 10 + 8],
            "row {y}"
        );
        assert!(dst[y * 12 + 8..y * 12 + 12].iter().all(|&b| b == UNTOUCHED));
    }
}

#[test]
fn plane_rows_narrower_destination() {
    // the destination stride below the row: each row overwrites the last's
    // tail, as the C's memcpy() calls do in order
    let copy = rows(4, 3, 4, 2);
    let src = counting(12);
    let mut dst = vec![UNTOUCHED; 8];
    assert_eq!(copy_plane(&copy, &src, &mut dst), Some(12));
    assert_eq!(dst, [0, 1, 4, 5, 8, 9, 10, 11]);
}

#[test]
fn plane_rows_with_no_stride() {
    // a zero source stride copies the first row every time
    let copy = rows(3, 3, 0, 3);
    let src = counting(3);
    let mut dst = vec![UNTOUCHED; 9];
    assert_eq!(copy_plane(&copy, &src, &mut dst), Some(0));
    assert_eq!(dst, [0, 1, 2, 0, 1, 2, 0, 1, 2]);
}

#[test]
fn plane_lengths() {
    assert_eq!(src_len(&whole(40)), Some(40));
    assert_eq!(dst_len(&whole(40)), Some(40));
    assert_eq!(src_advance(&whole(40)), Some(40));

    // the last row ends at its bytes, not at its stride
    let copy = rows(8, 3, 10, 12);
    assert_eq!(src_len(&copy), Some(28));
    assert_eq!(dst_len(&copy), Some(32));
    // the source still moves on by every stride
    assert_eq!(src_advance(&copy), Some(30));

    // no rows, or no bytes: nothing touched; the advance is the C's
    assert_eq!(src_len(&rows(8, 0, 10, 12)), Some(0));
    assert_eq!(dst_len(&rows(8, 0, 10, 12)), Some(0));
    assert_eq!(src_advance(&rows(8, 0, 10, 12)), Some(0));
    assert_eq!(src_len(&rows(0, 5, 10, 12)), Some(0));
    assert_eq!(dst_len(&rows(0, 5, 10, 12)), Some(0));
    assert_eq!(src_advance(&rows(0, 5, 10, 12)), Some(50));
    assert_eq!(src_len(&whole(0)), Some(0));
}

#[test]
fn plane_nothing_to_copy() {
    let mut dst = [UNTOUCHED; 4];
    assert_eq!(copy_plane(&whole(0), &[], &mut dst), Some(0));
    assert_eq!(copy_plane(&rows(8, 0, 10, 12), &[], &mut dst), Some(0));
    assert_eq!(copy_plane(&rows(0, 5, 10, 12), &[], &mut []), Some(50));
    assert_eq!(dst, [UNTOUCHED; 4]);
}

#[test]
fn plane_short_slices_copy_nothing() {
    let src = counting(40);
    let mut dst = vec![UNTOUCHED; 40];
    assert_eq!(copy_plane(&whole(40), &src[..39], &mut dst), None);
    assert_eq!(copy_plane(&whole(40), &src, &mut dst[..39]), None);
    let copy = rows(8, 3, 10, 12);
    assert_eq!(copy_plane(&copy, &src[..27], &mut dst), None);
    assert_eq!(copy_plane(&copy, &src, &mut dst[..31]), None);
    assert!(dst.iter().all(|&b| b == UNTOUCHED));
    // exactly long enough is enough
    assert_eq!(copy_plane(&copy, &src[..28], &mut dst[..32]), Some(30));
}

#[test]
fn plane_overflowing_lengths() {
    let copy = rows(usize::MAX, 2, u32::MAX, 1);
    assert_eq!(src_len(&copy), None);
    assert_eq!(copy_plane(&copy, &[], &mut []), None);
}

// ---------------------------------------------------------------------------
// the plan's loop (set_gpu_converted_data)

fn plan_of(copies: &[FrameCopy], count: usize) -> CopyPlan {
    let mut plan = CopyPlan {
        count,
        ..CopyPlan::default()
    };
    plan.copies[..copies.len()].copy_from_slice(copies);
    plan
}

#[test]
fn steps_read_their_own_planes() {
    let plan = plan_of(&[whole(4), whole(2), whole(2)], 3);
    let got: Vec<Step> = steps(&plan).collect();
    assert_eq!(got.len(), 3);
    for (i, step) in got.iter().enumerate() {
        assert_eq!(step.plane, i);
        assert_eq!(step.copy, plan.copies[i]);
        assert_eq!(step.source, Some(Source::Plane(i)));
    }
}

#[test]
fn steps_stop_at_count_or_max() {
    let three = [whole(1), whole(2), whole(3)];
    assert_eq!(steps(&plan_of(&three, 0)).count(), 0);
    assert_eq!(steps(&plan_of(&three, 1)).count(), 1);
    assert_eq!(steps(&plan_of(&three, 2)).count(), 2);
    assert_eq!(steps(&plan_of(&three, 3)).count(), COPY_MAX);
    // a count past OBS_VIDEO_COPY_MAX stops at it
    assert_eq!(steps(&plan_of(&three, 4)).count(), COPY_MAX);
    assert_eq!(steps(&plan_of(&three, usize::MAX)).count(), COPY_MAX);
}

#[test]
fn steps_after_plane0_whole() {
    let uv = FrameCopy {
        src_after_plane0: true,
        ..whole(8)
    };
    let plan = plan_of(&[whole(16), uv], 2);
    let got: Vec<Step> = steps(&plan).collect();
    assert_eq!(got[1].source, Some(Source::AfterPlane0(16)));
}

#[test]
fn steps_after_plane0_rows() {
    // plane 0's source ends after every stride, padding included
    let uv = FrameCopy {
        src_after_plane0: true,
        ..rows(4, 2, 6, 4)
    };
    let plan = plan_of(&[rows(4, 4, 6, 4), uv], 2);
    let got: Vec<Step> = steps(&plan).collect();
    assert_eq!(got[1].source, Some(Source::AfterPlane0(24)));
}

#[test]
fn steps_after_plane0_as_copy_0() {
    // no plan has it: the C reads from NULL; the step has no source
    let first = FrameCopy {
        src_after_plane0: true,
        ..whole(4)
    };
    let plan = plan_of(&[first, whole(2)], 2);
    let got: Vec<Step> = steps(&plan).collect();
    assert_eq!(got[0].source, None);
    assert_eq!(got[1].source, Some(Source::Plane(1)));

    let src = counting(4);
    let mut d0 = [UNTOUCHED; 4];
    let mut d1 = [UNTOUCHED; 2];
    let mut d2: [u8; 0] = [];
    let mut dst = [&mut d0[..], &mut d1[..], &mut d2[..]];
    assert_eq!(copy_frame(&plan, [&src, &src, &[]], &mut dst), 1);
    assert_eq!(d0, [UNTOUCHED; 4]);
    assert_eq!(d1, [0, 1]);
}

#[test]
fn frame_copies_the_plane_after_plane0() {
    // a packed NV12 frame, 4x4 at a stride of 6: Y then UV in one buffer
    let plan = gpu_copy_plan(Some(VideoFormat::Nv12), 4, 4, &[6, 0, 0], &[4, 4, 0]);
    assert_eq!(plan.count, 2);
    assert!(plan.copies[1].src_after_plane0);
    let src = counting(6 * 6);
    let mut y = [UNTOUCHED; 16];
    let mut uv = [UNTOUCHED; 8];
    let mut none: Vec<u8> = Vec::new();
    let mut dst = [&mut y[..], &mut uv[..], &mut none[..]];
    assert_eq!(copy_frame(&plan, [&src, &[], &[]], &mut dst), 2);
    for row in 0..4 {
        assert_eq!(&y[row * 4..row * 4 + 4], &src[row * 6..row * 6 + 4]);
    }
    for row in 0..2 {
        let at = 24 + row * 6;
        assert_eq!(&uv[row * 4..row * 4 + 4], &src[at..at + 4]);
    }
}

#[test]
fn frame_stops_at_a_short_plane() {
    let plan = plan_of(&[whole(4), whole(4), whole(4)], 3);
    let src = counting(4);
    let mut d0 = [UNTOUCHED; 4];
    let mut d1 = [UNTOUCHED; 3];
    let mut d2 = [UNTOUCHED; 4];
    let mut dst = [&mut d0[..], &mut d1[..], &mut d2[..]];
    assert_eq!(copy_frame(&plan, [&src, &src, &src], &mut dst), 1);
    assert_eq!(d0, [0, 1, 2, 3]);
    assert_eq!(d1, [UNTOUCHED; 3]);
    assert_eq!(d2, [UNTOUCHED; 4]);
}

#[test]
fn frame_with_no_copies() {
    let plan = gpu_copy_plan(Some(VideoFormat::Rgba), 4, 4, &[16; 3], &[16; 3]);
    let mut d = [UNTOUCHED; 4];
    let mut e: [u8; 0] = [];
    let mut f: [u8; 0] = [];
    let mut dst = [&mut d[..], &mut e[..], &mut f[..]];
    assert_eq!(copy_frame(&plan, [&[1; 4], &[], &[]], &mut dst), 0);
    assert_eq!(d, [UNTOUCHED; 4]);
}

// ---------------------------------------------------------------------------
// copy_rgbx_frame

#[test]
fn rgbx_matching_line_sizes() {
    let src = counting(16 * 3);
    let mut dst = vec![UNTOUCHED; 16 * 3];
    assert!(copy_rgbx_frame(4, 3, &src, 16, &mut dst, 16));
    assert_eq!(dst, src);
}

#[test]
fn rgbx_padded_line_sizes() {
    // rows of width * 4 bytes; the padding of each is left as it was
    let src = counting(20 * 3);
    let mut dst = vec![UNTOUCHED; 24 * 3];
    assert!(copy_rgbx_frame(4, 3, &src, 20, &mut dst, 24));
    for y in 0..3 {
        assert_eq!(&dst[y * 24..y * 24 + 16], &src[y * 20..y * 20 + 16]);
        assert!(
            dst[y * 24 + 16..y * 24 + 24]
                .iter()
                .all(|&b| b == UNTOUCHED)
        );
    }
}

#[test]
fn rgbx_short_slices() {
    let src = counting(16 * 3);
    let mut dst = vec![UNTOUCHED; 16 * 3];
    assert!(!copy_rgbx_frame(4, 3, &src[..47], 16, &mut dst, 16));
    assert!(!copy_rgbx_frame(4, 3, &src, 16, &mut dst[..47], 16));
    assert!(dst.iter().all(|&b| b == UNTOUCHED));
}

// ---------------------------------------------------------------------------
// test_video_frame_copy's cases

/// `struct plane_layout`: the width and height divisors (`wdiv` 0: no
/// plane), samples per pixel and bytes per sample.
#[derive(Clone, Copy)]
struct Layout {
    wdiv: u32,
    hdiv: u32,
    comps: u32,
    bytes: u32,
}

const NO_PLANE: Layout = Layout {
    wdiv: 0,
    hdiv: 0,
    comps: 0,
    bytes: 0,
};

const fn l(wdiv: u32, hdiv: u32, comps: u32, bytes: u32) -> Layout {
    Layout {
        wdiv,
        hdiv,
        comps,
        bytes,
    }
}

/// `layout_of()`.
fn layout_of(format: VideoFormat) -> [Layout; 3] {
    match format {
        VideoFormat::I420 => [l(1, 1, 1, 1), l(2, 2, 1, 1), l(2, 2, 1, 1)],
        VideoFormat::Nv12 => [l(1, 1, 1, 1), l(2, 2, 2, 1), NO_PLANE],
        VideoFormat::I444 => [l(1, 1, 1, 1), l(1, 1, 1, 1), l(1, 1, 1, 1)],
        VideoFormat::I010 => [l(1, 1, 1, 2), l(2, 2, 1, 2), l(2, 2, 1, 2)],
        VideoFormat::P010 => [l(1, 1, 1, 2), l(2, 2, 2, 2), NO_PLANE],
        VideoFormat::P216 => [l(1, 1, 1, 2), l(2, 1, 2, 2), NO_PLANE],
        VideoFormat::P416 => [l(1, 1, 1, 2), l(1, 1, 2, 2), NO_PLANE],
        VideoFormat::Rgba | VideoFormat::Bgra | VideoFormat::Bgrx => {
            [l(1, 1, 4, 1), NO_PLANE, NO_PLANE]
        }
        _ => [NO_PLANE; 3],
    }
}

/// `struct copy_case`, without the measured colors (the quadrants here get
/// values of their own).
struct Case {
    format: VideoFormat,
    gpu_conversion: bool,
    /// The output frame's line size per pixel of width; 0 for the
    /// half-width I420 chroma planes (`want_linesize()`).
    linesize_per_px: [u32; 3],
}

const fn case(format: VideoFormat, gpu_conversion: bool, linesize_per_px: [u32; 3]) -> Case {
    Case {
        format,
        gpu_conversion,
        linesize_per_px,
    }
}

/// `cases[]`, in its order.
const CASES: [Case; 13] = [
    case(VideoFormat::I420, true, [1, 0, 0]),
    case(VideoFormat::Nv12, true, [1, 1, 0]),
    case(VideoFormat::I444, true, [1, 1, 1]),
    case(VideoFormat::I010, true, [2, 1, 1]),
    case(VideoFormat::P010, true, [2, 2, 0]),
    case(VideoFormat::P216, true, [2, 2, 0]),
    case(VideoFormat::P416, true, [2, 4, 0]),
    case(VideoFormat::Rgba, true, [4, 0, 0]),
    case(VideoFormat::Bgra, true, [4, 0, 0]),
    case(VideoFormat::Bgrx, true, [4, 0, 0]),
    case(VideoFormat::Rgba, false, [4, 0, 0]),
    case(VideoFormat::Bgra, false, [4, 0, 0]),
    case(VideoFormat::Bgrx, false, [4, 0, 0]),
];

/// `want_linesize()`.
fn want_linesize(cc: &Case, c: usize, width: u32) -> u32 {
    if cc.linesize_per_px[c] != 0 {
        cc.linesize_per_px[c] * width
    } else if c < 3 && layout_of(cc.format)[c].wdiv != 0 {
        width / 2
    } else {
        0
    }
}

/// The value of sample `k` of plane `c` in quadrant `q`.
fn value(q: usize, c: usize, k: usize, bytes: u32) -> u32 {
    let v = (q * 16 + c * 4 + k + 1) as u32;
    if bytes == 2 { v * 0x0101 } else { v }
}

/// The quadrant of (`x`, `y`) in a plane of `w` x `h`.
fn quadrant(x: u32, y: u32, w: u32, h: u32) -> usize {
    usize::from(x >= w / 2) + 2 * usize::from(y >= h / 2)
}

/// How the staging surfaces lay the GPU-converted frame out.
#[derive(Clone, Copy, Debug)]
enum Staging {
    /// Line sizes of the row's bytes, each plane on its own (OpenGL).
    Tight,
    /// Every line padded (Direct3D), each plane on its own.
    Padded,
    /// Padded, with the UV plane after plane 0 in one surface (input
    /// `linesize[1]` of 0; NV12 and P010 only).
    Packed,
}

fn pad(row: u32) -> u32 {
    row.div_ceil(64) * 64 + 32
}

/// Paints plane `c` of a `width` x `height` frame at `linesize` from `at`.
fn paint(
    buf: &mut [u8],
    at: usize,
    layout: Layout,
    c: usize,
    linesize: u32,
    width: u32,
    height: u32,
) {
    let (w, h) = (width / layout.wdiv, height / layout.hdiv);
    for y in 0..h {
        for x in 0..w {
            for k in 0..layout.comps {
                let v = value(quadrant(x, y, w, h), c, k as usize, layout.bytes);
                let o = at
                    + y as usize * linesize as usize
                    + ((x * layout.comps + k) * layout.bytes) as usize;
                buf[o] = v as u8;
                if layout.bytes == 2 {
                    buf[o + 1] = (v >> 8) as u8;
                }
            }
        }
    }
}

/// `sample_at()`.
fn sample_at(data: &[u8], linesize: u32, layout: Layout, x: u32, y: u32, k: u32) -> u32 {
    let o = y as usize * linesize as usize + ((x * layout.comps + k) * layout.bytes) as usize;
    if layout.bytes == 2 {
        u32::from(data[o]) | u32::from(data[o + 1]) << 8
    } else {
        u32::from(data[o])
    }
}

fn check_case(cc: &Case, width: u32, height: u32, staging: Staging) {
    let layouts = layout_of(cc.format);
    let row = |c: usize| (width / layouts[c].wdiv) * layouts[c].comps * layouts[c].bytes;
    let plane_rows = |c: usize| height / layouts[c].hdiv;
    let planes: Vec<usize> = (0..3).filter(|&c| layouts[c].wdiv != 0).collect();
    let packed = matches!(staging, Staging::Packed);

    // the output frame, as video-io lays it out
    let out_ls: LineSizes = [0, 1, 2].map(|c| want_linesize(cc, c, width));
    let mut out: [Vec<u8>; 3] = std::array::from_fn(|c| {
        let rows = if c < planes.len() { plane_rows(c) } else { 0 };
        vec![UNTOUCHED; (out_ls[c] * rows) as usize]
    });

    // the staging surfaces
    let mut in_ls: LineSizes = [0; 3];
    for &c in &planes {
        in_ls[c] = match staging {
            Staging::Tight => row(c),
            Staging::Padded | Staging::Packed => pad(row(c)),
        };
    }
    let mut input: Vec<Vec<u8>> = vec![Vec::new(); 3];
    if packed {
        // the UV plane is laid out at plane 0's stride, after its rows
        let y_len = (in_ls[0] * plane_rows(0)) as usize;
        input[0] = vec![0; y_len + (in_ls[0] * plane_rows(1)) as usize];
        paint(&mut input[0], 0, layouts[0], 0, in_ls[0], width, height);
        paint(&mut input[0], y_len, layouts[1], 1, in_ls[0], width, height);
        in_ls[1] = 0;
    } else {
        for &c in &planes {
            input[c] = vec![0; (in_ls[c] * plane_rows(c)) as usize];
            paint(&mut input[c], 0, layouts[c], c, in_ls[c], width, height);
        }
    }

    // output_video_data(): GPU conversion only where the format has one
    let gpu = cc.gpu_conversion
        && !matches!(
            cc.format,
            VideoFormat::Rgba | VideoFormat::Bgra | VideoFormat::Bgrx
        );
    if gpu {
        let plan = gpu_copy_plan(Some(cc.format), width, height, &in_ls, &out_ls);
        assert_eq!(plan.count, planes.len(), "{:?}", cc.format);
        let src = [&input[0][..], &input[1][..], &input[2][..]];
        let [o0, o1, o2] = &mut out;
        let mut dst = [&mut o0[..], &mut o1[..], &mut o2[..]];
        assert_eq!(copy_frame(&plan, src, &mut dst), planes.len());
    } else {
        assert!(copy_rgbx_frame(
            width,
            height,
            &input[0],
            in_ls[0],
            &mut out[0],
            out_ls[0]
        ));
    }

    let what = format!(
        "{:?} gpu {} {width}x{height} {staging:?}",
        cc.format, cc.gpu_conversion
    );
    for &c in &planes {
        let lay = layouts[c];
        let (w, h) = (width / lay.wdiv, height / lay.hdiv);
        for y in 0..h {
            for x in 0..w {
                for k in 0..lay.comps {
                    let want = value(quadrant(x, y, w, h), c, k as usize, lay.bytes);
                    let got = sample_at(&out[c], out_ls[c], lay, x, y, k);
                    assert_eq!(got, want, "{what}: plane {c} ({x}, {y}) sample {k}");
                }
            }
            // what follows the row is left as video-io gave it
            let tail = &out[c][(y * out_ls[c] + row(c)) as usize..((y + 1) * out_ls[c]) as usize];
            assert!(
                tail.iter().all(|&b| b == UNTOUCHED),
                "{what}: plane {c} row {y}"
            );
        }
    }
    // no plane past the format's is written
    for (c, plane) in out.iter().enumerate().skip(planes.len()) {
        assert!(plane.is_empty(), "{what}: plane {c}");
    }
}

fn run_cases(width: u32, height: u32) {
    for cc in &CASES {
        // I420 at 68x34: left out, as the C test leaves it out
        if cc.format == VideoFormat::I420 && width == 68 {
            continue;
        }
        check_case(cc, width, height, Staging::Tight);
        check_case(cc, width, height, Staging::Padded);
        if matches!(cc.format, VideoFormat::Nv12 | VideoFormat::P010) && cc.gpu_conversion {
            check_case(cc, width, height, Staging::Packed);
        }
    }
}

/// Mirrors `frames_64x32`.
#[test]
fn frames_64x32() {
    run_cases(64, 32);
}

/// Mirrors `frames_68x34`.
#[test]
fn frames_68x34() {
    run_cases(68, 34);
}

#[test]
fn frames_tight_staging_is_one_whole_copy() {
    // what the C test sees with OpenGL: every plane one copy
    let plan = gpu_copy_plan(Some(VideoFormat::I444), 64, 32, &[64; 3], &[64; 3]);
    assert!(plan.copies.iter().all(|c| c.whole));
    let plan = rgbx_copy_plan(64, 32, 256, 256);
    assert!(plan.whole);
}

// ---------------------------------------------------------------------------
// proptests against the C's statements

/// `set_gpu_converted_plane()` written out: the memcpy() calls in order.
fn model(copy: &FrameCopy, src: &[u8], dst: &mut [u8]) -> usize {
    if copy.whole {
        dst[..copy.bytes].copy_from_slice(&src[..copy.bytes]);
        copy.bytes
    } else {
        let (mut i, mut o) = (0, 0);
        for _ in 0..copy.rows {
            // memcpy() of no bytes touches neither pointer
            if copy.bytes != 0 {
                dst[o..o + copy.bytes].copy_from_slice(&src[i..i + copy.bytes]);
            }
            i += copy.src_stride as usize;
            o += copy.dst_stride as usize;
        }
        i
    }
}

fn any_copy() -> impl Strategy<Value = FrameCopy> {
    (any::<bool>(), 0..48_usize, 0..8_u32, 0..64_u32, 0..64_u32).prop_map(
        |(whole, bytes, rows, src_stride, dst_stride)| FrameCopy {
            src_after_plane0: false,
            whole,
            bytes,
            rows,
            src_stride,
            dst_stride,
        },
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(256)))]

    #[test]
    fn plane_matches_the_c(copy in any_copy(), src_slack in 0..16_usize, dst_slack in 0..16_usize) {
        let need_src = src_len(&copy).unwrap();
        let need_dst = dst_len(&copy).unwrap();
        let src = counting(need_src + src_slack);
        let mut got = vec![UNTOUCHED; need_dst + dst_slack];
        let mut want = got.clone();
        let advance = copy_plane(&copy, &src, &mut got);
        let model_advance = model(&copy, &src, &mut want);
        prop_assert_eq!(advance, Some(model_advance));
        prop_assert_eq!(got, want);
    }

    #[test]
    fn plane_short_copies_nothing(copy in any_copy(), short_src in any::<bool>()) {
        let need_src = src_len(&copy).unwrap();
        let need_dst = dst_len(&copy).unwrap();
        prop_assume!(if short_src { need_src > 0 } else { need_dst > 0 });
        let src = counting(need_src - usize::from(short_src));
        let mut dst = vec![UNTOUCHED; need_dst - usize::from(!short_src)];
        prop_assert_eq!(copy_plane(&copy, &src, &mut dst), None);
        prop_assert!(dst.iter().all(|&b| b == UNTOUCHED));
    }
}

// ---------------------------------------------------------------------------
// the line sizes the plan reads

#[test]
fn line_sizes_read_per_format() {
    use VideoFormat as F;
    for format in VideoFormat::ALL {
        let want = match format {
            F::I420 | F::I444 | F::I010 => 3,
            F::Nv12 | F::P010 | F::P216 | F::P416 => 2,
            _ => 0,
        };
        assert_eq!(line_sizes_read(Some(format)), want, "{format:?}");
    }
    assert_eq!(line_sizes_read(None), 0);
}

#[test]
fn line_sizes_past_the_count_change_no_plan() {
    // the entries past line_sizes_read() never change the plan, so zeroing
    // them (the shim does not read them) gives the C's plan
    let sizes = [(1, 1), (2, 2), (64, 32), (67, 33)];
    for format in VideoFormat::ALL.map(Some).into_iter().chain([None]) {
        let n = line_sizes_read(format);
        for (w, h) in sizes {
            for (row_in, row_out) in [(0, 0), (8, 0), (0, 12), (3, 5)] {
                let full_in = [w * 4 + row_in, w * 2 + row_in, w + row_in];
                let full_out = [w * 4 + row_out, w * 2 + row_out, w + row_out];
                let mut cut_in = [0; 3];
                let mut cut_out = [0; 3];
                cut_in[..n].copy_from_slice(&full_in[..n]);
                cut_out[..n].copy_from_slice(&full_out[..n]);
                assert_eq!(
                    gpu_copy_plan(format, w, h, &cut_in, &cut_out),
                    gpu_copy_plan(format, w, h, &full_in, &full_out),
                    "{format:?} {w}x{h}"
                );
            }
        }
    }
}

#[test]
fn line_sizes_up_to_the_count_change_the_plan() {
    // each entry the count covers is read: changing it changes the plan
    for format in VideoFormat::ALL {
        let n = line_sizes_read(Some(format));
        for i in 0..n {
            let base = [4096_u32; 3];
            let mut moved = base;
            moved[i] = 4100;
            assert_ne!(
                gpu_copy_plan(Some(format), 64, 32, &moved, &base),
                gpu_copy_plan(Some(format), 64, 32, &base, &base),
                "{format:?} input {i}"
            );
            assert_ne!(
                gpu_copy_plan(Some(format), 64, 32, &base, &moved),
                gpu_copy_plan(Some(format), 64, 32, &base, &base),
                "{format:?} output {i}"
            );
        }
    }
}
