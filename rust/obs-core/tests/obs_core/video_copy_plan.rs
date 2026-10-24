//! Tier 1: `obs_core::video_copy_plan`, the output frame copy plan and the
//! scale effect choice.
//!
//! The `steps_*` cases port `test/cmocka/test_video_copy_plan_steps.c` 1:1:
//! its 22 cases under the same names, in the same order, with the same
//! values and expectations, the "characterized, not endorsed" ones
//! included. The C calls the hidden `obs_video_*` functions; here the safe
//! core is called directly ([`gpu_copy_plan`], [`rgbx_copy_plan`],
//! [`resolution_close`], [`scale_effect`], [`mix_reuse_match`]). A C format
//! or scale type outside its enum is `None` here.
//!
//! The `edge_*` cases are new: proptests over full-range `u32` sizes and
//! line sizes for every format, `resolution_close` against an `i64` model
//! for every `u32` and against a 32-bit `long` (MSVC) for every size
//! `obs_video_size_valid()` allows, the scale effect over every type, mask
//! and full-range sizes, and the reuse match over full-range keys.

use obs_core::video_copy_plan::{
    COPY_MAX, CopyPlan, FrameCopy, LineSizes, MixReuseKey, ScaleEffect, ScaleInput, gpu_copy_plan,
    mix_reuse_match, resolution_close, rgbx_copy_plan, scale_effect,
};
use obs_core::video_plan::ObsScaleType;
use obs_media_io::video_io::VideoFormat;
use proptest::prelude::*;

const LOADED_ALL: u32 = 0x1f;

// ---------------------------------------------------------------------------
// helpers

#[allow(clippy::too_many_arguments)]
fn check_copy(
    what: &str,
    index: usize,
    got: &FrameCopy,
    after: bool,
    whole: bool,
    bytes: usize,
    rows: u32,
    src_stride: u32,
    dst_stride: u32,
) {
    let want = FrameCopy {
        src_after_plane0: after,
        whole,
        bytes,
        rows,
        src_stride,
        dst_stride,
    };
    assert_eq!(*got, want, "{what} copy {index}");
}

fn lines(l0: u32, l1: u32, l2: u32) -> LineSizes {
    [l0, l1, l2]
}

fn plan_of(
    format: Option<VideoFormat>,
    width: u32,
    height: u32,
    input: &LineSizes,
    output: &LineSizes,
) -> CopyPlan {
    let plan = gpu_copy_plan(format, width, height, input, output);
    assert!(plan.count <= COPY_MAX, "{format:?}: {} copies", plan.count);
    // the copies past the count are zero
    for unused in plan.copies.iter().skip(plan.count) {
        assert_eq!(*unused, FrameCopy::default(), "{format:?}");
    }
    plan
}

#[derive(Clone, Copy, Debug)]
enum RowSize {
    W,
    WD2,
    WX2,
    WX4,
}

#[derive(Clone, Copy, Debug)]
enum RowCount {
    H,
    HD2,
}

#[derive(Clone, Copy, Debug)]
struct Shape {
    row: RowSize,
    rows: RowCount,
    in_line: usize,
    out_line: usize,
    after: bool,
}

const fn shape(
    row: RowSize,
    rows: RowCount,
    in_line: usize,
    out_line: usize,
    after: bool,
) -> Shape {
    Shape {
        row,
        rows,
        in_line,
        out_line,
        after,
    }
}

struct FormatShape {
    format: VideoFormat,
    copies: &'static [Shape],
}

/// Every format with a GPU conversion, with input `linesize[1]` set.
const SHAPES: [FormatShape; 7] = [
    FormatShape {
        format: VideoFormat::I420,
        copies: &[
            shape(RowSize::W, RowCount::H, 0, 0, false),
            shape(RowSize::WD2, RowCount::HD2, 1, 1, false),
            shape(RowSize::WD2, RowCount::HD2, 2, 2, false),
        ],
    },
    FormatShape {
        format: VideoFormat::Nv12,
        copies: &[
            shape(RowSize::W, RowCount::H, 0, 0, false),
            shape(RowSize::W, RowCount::HD2, 1, 1, false),
        ],
    },
    FormatShape {
        format: VideoFormat::I444,
        copies: &[
            shape(RowSize::W, RowCount::H, 0, 0, false),
            shape(RowSize::W, RowCount::H, 1, 1, false),
            shape(RowSize::W, RowCount::H, 2, 2, false),
        ],
    },
    FormatShape {
        format: VideoFormat::I010,
        copies: &[
            shape(RowSize::WX2, RowCount::H, 0, 0, false),
            shape(RowSize::W, RowCount::HD2, 1, 1, false),
            shape(RowSize::W, RowCount::HD2, 2, 2, false),
        ],
    },
    FormatShape {
        format: VideoFormat::P010,
        copies: &[
            shape(RowSize::WX2, RowCount::H, 0, 0, false),
            shape(RowSize::WX2, RowCount::HD2, 1, 1, false),
        ],
    },
    FormatShape {
        format: VideoFormat::P216,
        copies: &[
            shape(RowSize::WX2, RowCount::H, 0, 0, false),
            shape(RowSize::WX2, RowCount::H, 1, 1, false),
        ],
    },
    FormatShape {
        format: VideoFormat::P416,
        copies: &[
            shape(RowSize::WX2, RowCount::H, 0, 0, false),
            shape(RowSize::WX4, RowCount::H, 1, 1, false),
        ],
    },
];

/// The packed NV12 and P010 (input `linesize[1]` of 0).
const PACKED_SHAPES: [FormatShape; 2] = [
    FormatShape {
        format: VideoFormat::Nv12,
        copies: &[
            shape(RowSize::W, RowCount::H, 0, 0, false),
            shape(RowSize::W, RowCount::HD2, 0, 1, true),
        ],
    },
    FormatShape {
        format: VideoFormat::P010,
        copies: &[
            shape(RowSize::WX2, RowCount::H, 0, 0, false),
            shape(RowSize::WX2, RowCount::HD2, 0, 1, true),
        ],
    },
];

fn row_bytes(row: RowSize, width: u32) -> u32 {
    match row {
        RowSize::W => width,
        RowSize::WD2 => width / 2,
        RowSize::WX2 => width.wrapping_mul(2),
        RowSize::WX4 => width.wrapping_mul(4),
    }
}

fn row_count(rows: RowCount, height: u32) -> u32 {
    match rows {
        RowCount::H => height,
        RowCount::HD2 => height / 2,
    }
}

/// The plan of `fs` at `width` x `height` with the line sizes `input` and
/// `output` matches its shape.
fn check_shape(fs: &FormatShape, width: u32, height: u32, input: &LineSizes, output: &LineSizes) {
    let what = format!(
        "format {:?} {width}x{height} in {input:?} out {output:?}",
        fs.format
    );
    let plan = plan_of(Some(fs.format), width, height, input, output);
    assert_eq!(plan.count, fs.copies.len(), "{what}");
    for (i, s) in fs.copies.iter().enumerate() {
        let row = row_bytes(s.row, width);
        let rows = row_count(s.rows, height);
        let src = input[s.in_line];
        let dst = output[s.out_line];
        let whole = row == src && row == dst;
        let bytes = if whole {
            (row as usize).wrapping_mul(rows as usize)
        } else {
            row as usize
        };
        check_copy(
            &what,
            i,
            &plan.copies[i],
            s.after,
            whole,
            bytes,
            rows,
            src,
            dst,
        );
    }
}

fn has_gpu_copy(format: VideoFormat) -> bool {
    SHAPES.iter().any(|fs| fs.format == format)
}

fn tight(fs: &FormatShape, width: u32) -> (LineSizes, LineSizes) {
    let mut input = [0; 3];
    let mut output = [0; 3];
    for s in fs.copies {
        input[s.in_line] = row_bytes(s.row, width);
        output[s.out_line] = row_bytes(s.row, width);
    }
    (input, output)
}

// ---------------------------------------------------------------------------
// constants

#[test]
fn steps_constants() {
    assert_eq!(COPY_MAX, 3);
    assert_eq!(ObsScaleType::Disable.to_c(), 0);
    assert_eq!(ObsScaleType::Point.to_c(), 1);
    assert_eq!(ObsScaleType::Bicubic.to_c(), 2);
    assert_eq!(ObsScaleType::Bilinear.to_c(), 3);
    assert_eq!(ObsScaleType::Lanczos.to_c(), 4);
    assert_eq!(ObsScaleType::Area.to_c(), 5);
    assert_eq!(
        ScaleEffect::ALL,
        [
            ScaleEffect::Default,
            ScaleEffect::BilinearLowres,
            ScaleEffect::Bicubic,
            ScaleEffect::Lanczos,
            ScaleEffect::Area
        ]
    );
    assert_eq!(ScaleEffect::Default.loaded_bit(), 0x1);
    assert_eq!(ScaleEffect::Area.loaded_bit(), 0x10);
}

// ---------------------------------------------------------------------------
// gpu_copy_plan

#[test]
fn steps_gpu_formats_without_copy() {
    for format in VideoFormat::ALL {
        if has_gpu_copy(format) {
            continue;
        }
        let plan = plan_of(
            Some(format),
            1920,
            1080,
            &lines(1920, 1920, 1920),
            &lines(1920, 1920, 1920),
        );
        assert_eq!(plan.count, 0, "{format:?}");
        let plan = plan_of(
            Some(format),
            1920,
            1080,
            &lines(0, 0, 0),
            &lines(7680, 3840, 3840),
        );
        assert_eq!(plan.count, 0, "{format:?}");
    }
    // the two values outside the enum (VIDEO_FORMAT_R10L + 1, 0x7fffffff)
    let plan = plan_of(
        None,
        1920,
        1080,
        &lines(1920, 1920, 1920),
        &lines(1920, 1920, 1920),
    );
    assert_eq!(plan.count, 0);
}

#[test]
fn steps_gpu_every_format_has_its_copies() {
    for format in VideoFormat::ALL {
        let want = SHAPES
            .iter()
            .find(|fs| fs.format == format)
            .map_or(0, |fs| fs.copies.len());
        let plan = plan_of(
            Some(format),
            1920,
            1080,
            &lines(4096, 4096, 4096),
            &lines(1920, 1920, 1920),
        );
        assert_eq!(plan.count, want, "{format:?}");
    }
}

#[test]
fn steps_gpu_i420() {
    let out = lines(1920, 960, 960);
    let p = plan_of(
        Some(VideoFormat::I420),
        1920,
        1080,
        &lines(1920, 960, 960),
        &out,
    );
    assert_eq!(p.count, 3);
    check_copy(
        "i420",
        0,
        &p.copies[0],
        false,
        true,
        1920 * 1080,
        1080,
        1920,
        1920,
    );
    check_copy(
        "i420",
        1,
        &p.copies[1],
        false,
        true,
        960 * 540,
        540,
        960,
        960,
    );
    check_copy(
        "i420",
        2,
        &p.copies[2],
        false,
        true,
        960 * 540,
        540,
        960,
        960,
    );

    let p = plan_of(
        Some(VideoFormat::I420),
        1920,
        1080,
        &lines(2048, 1024, 1024),
        &out,
    );
    check_copy(
        "i420 padded",
        0,
        &p.copies[0],
        false,
        false,
        1920,
        1080,
        2048,
        1920,
    );
    check_copy(
        "i420 padded",
        1,
        &p.copies[1],
        false,
        false,
        960,
        540,
        1024,
        960,
    );
    check_copy(
        "i420 padded",
        2,
        &p.copies[2],
        false,
        false,
        960,
        540,
        1024,
        960,
    );
}

#[test]
fn steps_gpu_nv12() {
    let out = lines(1920, 1920, 0);
    let p = plan_of(
        Some(VideoFormat::Nv12),
        1920,
        1080,
        &lines(1920, 1920, 0),
        &out,
    );
    assert_eq!(p.count, 2);
    check_copy(
        "nv12",
        0,
        &p.copies[0],
        false,
        true,
        1920 * 1080,
        1080,
        1920,
        1920,
    );
    check_copy(
        "nv12",
        1,
        &p.copies[1],
        false,
        true,
        1920 * 540,
        540,
        1920,
        1920,
    );

    let p = plan_of(
        Some(VideoFormat::Nv12),
        1920,
        1080,
        &lines(2048, 2560, 0),
        &out,
    );
    check_copy(
        "nv12 padded",
        0,
        &p.copies[0],
        false,
        false,
        1920,
        1080,
        2048,
        1920,
    );
    check_copy(
        "nv12 padded",
        1,
        &p.copies[1],
        false,
        false,
        1920,
        540,
        2560,
        1920,
    );
}

#[test]
fn steps_gpu_nv12_packed() {
    let out = lines(1920, 1920, 0);
    let p = plan_of(
        Some(VideoFormat::Nv12),
        1920,
        1080,
        &lines(1920, 0, 0),
        &out,
    );
    assert_eq!(p.count, 2);
    check_copy(
        "nv12 packed",
        0,
        &p.copies[0],
        false,
        true,
        1920 * 1080,
        1080,
        1920,
        1920,
    );
    check_copy(
        "nv12 packed",
        1,
        &p.copies[1],
        true,
        true,
        1920 * 540,
        540,
        1920,
        1920,
    );

    let p = plan_of(
        Some(VideoFormat::Nv12),
        1920,
        1080,
        &lines(2048, 0, 0),
        &out,
    );
    check_copy(
        "nv12 packed padded",
        0,
        &p.copies[0],
        false,
        false,
        1920,
        1080,
        2048,
        1920,
    );
    check_copy(
        "nv12 packed padded",
        1,
        &p.copies[1],
        true,
        false,
        1920,
        540,
        2048,
        1920,
    );

    // input linesize[2] does not matter, only [1]
    let p = plan_of(
        Some(VideoFormat::Nv12),
        1920,
        1080,
        &lines(1920, 0, 1920),
        &out,
    );
    assert!(p.copies[1].src_after_plane0);
}

#[test]
fn steps_gpu_i444() {
    let input = lines(1280, 1280, 1280);
    let p = plan_of(
        Some(VideoFormat::I444),
        1280,
        720,
        &input,
        &lines(1280, 1280, 1280),
    );
    assert_eq!(p.count, 3);
    for i in 0..3 {
        check_copy(
            "i444",
            i,
            &p.copies[i],
            false,
            true,
            1280 * 720,
            720,
            1280,
            1280,
        );
    }
    let p = plan_of(
        Some(VideoFormat::I444),
        1280,
        720,
        &input,
        &lines(1280, 1344, 1280),
    );
    check_copy(
        "i444",
        0,
        &p.copies[0],
        false,
        true,
        1280 * 720,
        720,
        1280,
        1280,
    );
    check_copy("i444", 1, &p.copies[1], false, false, 1280, 720, 1280, 1344);
    check_copy(
        "i444",
        2,
        &p.copies[2],
        false,
        true,
        1280 * 720,
        720,
        1280,
        1280,
    );
}

#[test]
fn steps_gpu_i010() {
    let p = plan_of(
        Some(VideoFormat::I010),
        1920,
        1080,
        &lines(3840, 1920, 1920),
        &lines(3840, 1920, 1920),
    );
    assert_eq!(p.count, 3);
    check_copy(
        "i010",
        0,
        &p.copies[0],
        false,
        true,
        3840 * 1080,
        1080,
        3840,
        3840,
    );
    check_copy(
        "i010",
        1,
        &p.copies[1],
        false,
        true,
        1920 * 540,
        540,
        1920,
        1920,
    );
    check_copy(
        "i010",
        2,
        &p.copies[2],
        false,
        true,
        1920 * 540,
        540,
        1920,
        1920,
    );
}

#[test]
fn steps_gpu_p010() {
    let out = lines(3840, 3840, 0);
    let p = plan_of(
        Some(VideoFormat::P010),
        1920,
        1080,
        &lines(3840, 3840, 0),
        &out,
    );
    assert_eq!(p.count, 2);
    check_copy(
        "p010",
        0,
        &p.copies[0],
        false,
        true,
        3840 * 1080,
        1080,
        3840,
        3840,
    );
    check_copy(
        "p010",
        1,
        &p.copies[1],
        false,
        true,
        3840 * 540,
        540,
        3840,
        3840,
    );

    let p = plan_of(
        Some(VideoFormat::P010),
        1920,
        1080,
        &lines(4096, 0, 0),
        &out,
    );
    check_copy(
        "p010 packed",
        0,
        &p.copies[0],
        false,
        false,
        3840,
        1080,
        4096,
        3840,
    );
    check_copy(
        "p010 packed",
        1,
        &p.copies[1],
        true,
        false,
        3840,
        540,
        4096,
        3840,
    );
    let p = plan_of(
        Some(VideoFormat::P010),
        1920,
        1080,
        &lines(3840, 0, 0),
        &out,
    );
    check_copy(
        "p010 packed tight",
        1,
        &p.copies[1],
        true,
        true,
        3840 * 540,
        540,
        3840,
        3840,
    );
}

#[test]
fn steps_gpu_p216_p416() {
    let out = lines(3840, 3840, 0);
    let p = plan_of(
        Some(VideoFormat::P216),
        1920,
        1080,
        &lines(3840, 3840, 0),
        &out,
    );
    assert_eq!(p.count, 2);
    check_copy(
        "p216",
        0,
        &p.copies[0],
        false,
        true,
        3840 * 1080,
        1080,
        3840,
        3840,
    );
    check_copy(
        "p216",
        1,
        &p.copies[1],
        false,
        true,
        3840 * 1080,
        1080,
        3840,
        3840,
    );
    let p = plan_of(
        Some(VideoFormat::P216),
        1920,
        1080,
        &lines(3840, 0, 0),
        &out,
    );
    check_copy(
        "p216 zero",
        1,
        &p.copies[1],
        false,
        false,
        3840,
        1080,
        0,
        3840,
    );

    let p = plan_of(
        Some(VideoFormat::P416),
        1920,
        1080,
        &lines(3840, 7680, 0),
        &lines(3840, 7680, 0),
    );
    assert_eq!(p.count, 2);
    check_copy(
        "p416",
        0,
        &p.copies[0],
        false,
        true,
        3840 * 1080,
        1080,
        3840,
        3840,
    );
    check_copy(
        "p416",
        1,
        &p.copies[1],
        false,
        true,
        7680 * 1080,
        1080,
        7680,
        7680,
    );
}

#[test]
fn steps_gpu_each_line_size() {
    let (w, h) = (640, 360);
    for packed in [false, true] {
        let list: &[FormatShape] = if packed { &PACKED_SHAPES } else { &SHAPES };
        for fs in list {
            let (tight_in, tight_out) = tight(fs, w);
            check_shape(fs, w, h, &tight_in, &tight_out);
            for line in 0..3 {
                for side in [false, true] {
                    if packed && !side && line == 1 {
                        continue; // stays 0: packed
                    }
                    let (mut input, mut output) = (tight_in, tight_out);
                    let l = if side { &mut output } else { &mut input };
                    l[line] += 64;
                    check_shape(fs, w, h, &input, &output);
                }
            }
        }
    }
}

#[test]
fn steps_gpu_widths_and_heights() {
    for w in [0, 1, 2, 3, 639, 640, 1919, 1921] {
        for h in [0, 1, 2, 3, 359, 1081] {
            for packed in [false, true] {
                let list: &[FormatShape] = if packed { &PACKED_SHAPES } else { &SHAPES };
                for fs in list {
                    let (mut input, mut output) = tight(fs, w);
                    if !packed && input[1] == 0 {
                        input[1] = 1;
                    }
                    check_shape(fs, w, h, &input, &output);
                    for l in 0..3 {
                        input[l] = (input[l] + 64) & !63;
                        output[l] = (output[l] + 32) & !31;
                    }
                    if packed {
                        input[1] = 0;
                    }
                    check_shape(fs, w, h, &input, &output);
                }
            }
        }
    }

    let p = plan_of(
        Some(VideoFormat::Nv12),
        0,
        1080,
        &lines(0, 1, 0),
        &lines(0, 0, 0),
    );
    check_copy("nv12 0 wide", 0, &p.copies[0], false, true, 0, 1080, 0, 0);
    check_copy("nv12 0 wide", 1, &p.copies[1], false, false, 0, 540, 1, 0);
}

/// Characterized, not endorsed: the row sizes are `u32`, so `width * 2`
/// and `width * 4` wrap; the single copy's size is a `size_t` product.
#[test]
fn steps_gpu_width_wrap() {
    for w in [
        0x4000_0000u32,
        0x7fff_ffff,
        0x8000_0000,
        0x8000_0001,
        0xffff_ffff,
    ] {
        for h in [0u32, 2, 0xffff_fffe, 0xffff_ffff] {
            for fs in &SHAPES {
                let (mut input, output) = tight(fs, w);
                if input[1] == 0 {
                    input[1] = 1;
                }
                check_shape(fs, w, h, &input, &output);
                check_shape(fs, w, h, &lines(1, 1, 1), &lines(2, 2, 2));
            }
        }
    }

    let one = lines(1, 1, 1);
    let p = plan_of(Some(VideoFormat::P416), 0x4000_0000, 4, &one, &one);
    assert_eq!(p.copies[0].bytes, 0x8000_0000);
    assert_eq!(p.copies[1].bytes, 0);
    let p = plan_of(Some(VideoFormat::I010), 0x8000_0000, 4, &one, &one);
    assert_eq!(p.copies[0].bytes, 0);
    assert_eq!(p.copies[1].bytes, 0x8000_0000);
    let p = plan_of(Some(VideoFormat::P010), 0x8000_0001, 4, &one, &one);
    assert_eq!(p.copies[0].bytes, 2);
    let p = plan_of(Some(VideoFormat::P216), 0xffff_ffff, 4, &one, &one);
    assert_eq!(p.copies[1].bytes, 0xffff_fffe);
    let p = plan_of(Some(VideoFormat::I420), 0xffff_ffff, 4, &one, &one);
    assert_eq!(p.copies[1].bytes, 0x7fff_ffff);

    let zero = lines(0, 0, 0);
    let p = plan_of(Some(VideoFormat::P416), 0x4000_0000, 1080, &zero, &zero);
    check_copy("p416 wrap", 1, &p.copies[1], false, true, 0, 1080, 0, 0);

    let max = lines(u32::MAX, 0, 0);
    let p = plan_of(Some(VideoFormat::I444), u32::MAX, u32::MAX, &max, &max);
    check_copy(
        "i444 max",
        0,
        &p.copies[0],
        false,
        true,
        (u32::MAX as usize).wrapping_mul(u32::MAX as usize),
        u32::MAX,
        u32::MAX,
        u32::MAX,
    );
}

// ---------------------------------------------------------------------------
// rgbx_copy_plan

#[test]
fn steps_rgbx_equal_line_sizes() {
    let c = rgbx_copy_plan(1920, 1080, 7680, 7680);
    check_copy("rgbx", 0, &c, false, true, 7680 * 1080, 1080, 7680, 7680);
    let c = rgbx_copy_plan(1920, 1080, 8192, 8192);
    check_copy(
        "rgbx padded",
        0,
        &c,
        false,
        true,
        8192 * 1080,
        1080,
        8192,
        8192,
    );
    // characterized, not endorsed: matching line sizes under width * 4
    let c = rgbx_copy_plan(1920, 1080, 1920, 1920);
    check_copy(
        "rgbx short",
        0,
        &c,
        false,
        true,
        1920 * 1080,
        1080,
        1920,
        1920,
    );
    let c = rgbx_copy_plan(1920, 1080, 0, 0);
    check_copy("rgbx zero", 0, &c, false, true, 0, 1080, 0, 0);
    let c = rgbx_copy_plan(u32::MAX, u32::MAX, u32::MAX, u32::MAX);
    check_copy(
        "rgbx max",
        0,
        &c,
        false,
        true,
        (u32::MAX as usize).wrapping_mul(u32::MAX as usize),
        u32::MAX,
        u32::MAX,
        u32::MAX,
    );
}

#[test]
fn steps_rgbx_rows() {
    let c = rgbx_copy_plan(1920, 1080, 8192, 7680);
    check_copy("rgbx rows", 0, &c, false, false, 1920 * 4, 1080, 8192, 7680);
    let c = rgbx_copy_plan(0, 0, 1, 2);
    check_copy("rgbx 0", 0, &c, false, false, 0, 0, 1, 2);
    let c = rgbx_copy_plan(1, 3, 4, 8);
    check_copy("rgbx 1", 0, &c, false, false, 4, 3, 4, 8);
    let c = rgbx_copy_plan(641, 361, 2564, 2576);
    check_copy("rgbx odd", 0, &c, false, false, 2564, 361, 2564, 2576);
    let c = rgbx_copy_plan(0x4000_0000, 2, 0, 1);
    check_copy(
        "rgbx wide",
        0,
        &c,
        false,
        false,
        (0x4000_0000usize).wrapping_mul(4),
        2,
        0,
        1,
    );
    let c = rgbx_copy_plan(u32::MAX, 2, 0, 1);
    check_copy(
        "rgbx max",
        0,
        &c,
        false,
        false,
        (u32::MAX as usize).wrapping_mul(4),
        2,
        0,
        1,
    );
}

// ---------------------------------------------------------------------------
// resolution_close

#[test]
fn steps_resolution_close_edges() {
    for d in [-17i64, -16, -15, 0, 15, 16, 17] {
        let want = (-16..=16).contains(&d);
        let w = u32::try_from(1920 + d).unwrap();
        let h = u32::try_from(1080 + d).unwrap();
        assert_eq!(resolution_close(1920, 1080, w, 1080), want, "{d}");
        assert_eq!(resolution_close(1920, 1080, 1920, h), want, "{d}");
        assert_eq!(resolution_close(1920, 1080, w, h), want, "{d}");
        assert!(!resolution_close(1920, 1080, w, 720), "{d}");
        assert!(!resolution_close(1920, 1080, 1280, h), "{d}");
    }
    assert!(resolution_close(2, 2, 18, 18));
    assert!(!resolution_close(2, 2, 19, 2));
    assert!(resolution_close(32768, 32768, 32752, 32752));
    assert!(!resolution_close(32768, 32768, 32751, 32768));
    assert!(!resolution_close(32768, 2, 2, 2));
    assert!(resolution_close(0, 0, 16, 16));
    assert!(!resolution_close(0, 0, 17, 0));
}

// ---------------------------------------------------------------------------
// scale_effect

#[allow(clippy::too_many_arguments)]
fn choose(
    base_w: u32,
    base_h: u32,
    target_w: u32,
    target_h: u32,
    out_w: u32,
    out_h: u32,
    scale_type: Option<ObsScaleType>,
    loaded: u32,
) -> ScaleEffect {
    scale_effect(&ScaleInput {
        base_width: base_w,
        base_height: base_h,
        target_width: target_w,
        target_height: target_h,
        output_width: out_w,
        output_height: out_h,
        scale_type,
        loaded,
    })
}

/// The C's `every_type`: the six types, then -1, 6 and 0x7fffffff (outside
/// the enum: `None`).
const EVERY_TYPE: [Option<ObsScaleType>; 9] = [
    Some(ObsScaleType::Disable),
    Some(ObsScaleType::Point),
    Some(ObsScaleType::Bicubic),
    Some(ObsScaleType::Bilinear),
    Some(ObsScaleType::Lanczos),
    Some(ObsScaleType::Area),
    None,
    None,
    None,
];

fn type_effect(scale_type: Option<ObsScaleType>) -> ScaleEffect {
    match scale_type {
        Some(ObsScaleType::Bilinear) => ScaleEffect::Default,
        Some(ObsScaleType::Lanczos) => ScaleEffect::Lanczos,
        Some(ObsScaleType::Area) => ScaleEffect::Area,
        _ => ScaleEffect::Bicubic,
    }
}

#[test]
fn steps_scale_close_is_default() {
    for t in EVERY_TYPE {
        for loaded in 0..=LOADED_ALL {
            assert_eq!(
                choose(1920, 1080, 1904, 1096, 1920, 1080, t, loaded),
                ScaleEffect::Default
            );
            assert_eq!(
                choose(1920, 1080, 1920, 1080, 100, 100, t, loaded),
                ScaleEffect::Default
            );
            assert_eq!(
                choose(1920, 1080, 1936, 1064, 0, 0, t, loaded),
                ScaleEffect::Default
            );
        }
    }
}

#[test]
fn steps_scale_type_table() {
    for t in EVERY_TYPE {
        assert_eq!(
            choose(1920, 1080, 1280, 720, 1280, 720, t, LOADED_ALL),
            type_effect(t),
            "{t:?}"
        );
        assert_eq!(
            choose(1920, 1080, 640, 360, 640, 360, t, LOADED_ALL),
            ScaleEffect::BilinearLowres,
            "{t:?}"
        );
        assert_eq!(
            choose(1280, 720, 1920, 1080, 1920, 1080, t, LOADED_ALL),
            type_effect(t),
            "{t:?}"
        );
    }
}

#[test]
fn steps_scale_lowres_rule() {
    let t = Some(ObsScaleType::Lanczos);
    let lowres = ScaleEffect::BilinearLowres;
    let lanczos = ScaleEffect::Lanczos;
    assert_eq!(
        choose(1920, 1080, 959, 539, 959, 539, t, LOADED_ALL),
        lowres
    );
    assert_eq!(
        choose(1920, 1080, 960, 539, 960, 539, t, LOADED_ALL),
        lanczos
    );
    assert_eq!(
        choose(1920, 1080, 959, 540, 959, 540, t, LOADED_ALL),
        lanczos
    );
    assert_eq!(
        choose(1920, 1080, 959, 1080, 959, 1080, t, LOADED_ALL),
        lanczos
    );
    assert_eq!(
        choose(1921, 1081, 959, 539, 959, 539, t, LOADED_ALL),
        lowres
    );
    assert_eq!(
        choose(1921, 1081, 960, 539, 960, 539, t, LOADED_ALL),
        lanczos
    );
    assert_eq!(
        choose(1920, 1080, 1280, 720, 640, 360, t, LOADED_ALL),
        lowres
    );
    assert_eq!(
        choose(1920, 1080, 640, 360, 1280, 720, t, LOADED_ALL),
        lanczos
    );
    assert_eq!(choose(1920, 1080, 640, 360, 0, 0, t, LOADED_ALL), lowres);
    assert_eq!(choose(1, 1, 100, 100, 0, 0, t, LOADED_ALL), lanczos);
    assert_eq!(choose(0, 1080, 100, 100, 0, 0, t, LOADED_ALL), lanczos);
}

fn expected(scale_type: Option<ObsScaleType>, lowres: bool, loaded: u32) -> ScaleEffect {
    let pick = if lowres {
        ScaleEffect::BilinearLowres
    } else {
        type_effect(scale_type)
    };
    if loaded & pick.loaded_bit() != 0 {
        pick
    } else if loaded & ScaleEffect::Bicubic.loaded_bit() != 0 {
        ScaleEffect::Bicubic
    } else {
        ScaleEffect::Default
    }
}

#[test]
fn steps_scale_effect_missing() {
    for t in EVERY_TYPE {
        for lowres in [false, true] {
            let (out_w, out_h) = if lowres { (640, 360) } else { (1280, 720) };
            for missing in ScaleEffect::ALL {
                let loaded = LOADED_ALL & !missing.loaded_bit();
                let got = choose(1920, 1080, out_w, out_h, out_w, out_h, t, loaded);
                let pick = if lowres {
                    ScaleEffect::BilinearLowres
                } else {
                    type_effect(t)
                };
                let want = if pick != missing {
                    pick
                } else if missing == ScaleEffect::Bicubic {
                    ScaleEffect::Default
                } else {
                    ScaleEffect::Bicubic
                };
                assert_eq!(got, want, "type {t:?} lowres {lowres} missing {missing:?}");
            }
        }
    }
    assert_eq!(
        choose(
            1920,
            1080,
            1280,
            720,
            1280,
            720,
            Some(ObsScaleType::Bilinear),
            0x1e
        ),
        ScaleEffect::Bicubic
    );
    assert_eq!(
        choose(
            1920,
            1080,
            1280,
            720,
            1280,
            720,
            Some(ObsScaleType::Area),
            0
        ),
        ScaleEffect::Default
    );
    assert_eq!(
        choose(1920, 1080, 640, 360, 640, 360, Some(ObsScaleType::Area), 0),
        ScaleEffect::Default
    );
    assert_eq!(
        choose(
            1920,
            1080,
            1280,
            720,
            1280,
            720,
            Some(ObsScaleType::Area),
            0xffff_ffe0
        ),
        ScaleEffect::Default
    );
}

#[test]
fn steps_scale_every_mask() {
    for t in EVERY_TYPE {
        for lowres in [false, true] {
            for close in [false, true] {
                for loaded in 0..=LOADED_ALL {
                    let (out_w, out_h) = if lowres { (640, 360) } else { (1280, 720) };
                    let (tw, th) = if close { (1910, 1070) } else { (out_w, out_h) };
                    let got = choose(1920, 1080, tw, th, out_w, out_h, t, loaded);
                    let want = if close {
                        ScaleEffect::Default
                    } else {
                        expected(t, lowres, loaded)
                    };
                    assert_eq!(
                        got, want,
                        "type {t:?} lowres {lowres} close {close} loaded {loaded:#x}"
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// mix_reuse_match

fn key(view: usize, space: i32, w: u32, h: u32, rendered: bool) -> MixReuseKey {
    MixReuseKey {
        view,
        render_space: space,
        base_width: w,
        base_height: h,
        rendered,
    }
}

fn reuse(other: MixReuseKey, mix: MixReuseKey) -> bool {
    mix_reuse_match(&other, &mix)
}

#[test]
fn steps_reuse_match() {
    let view = 0x1000;
    assert!(reuse(
        key(view, 0, 1920, 1080, true),
        key(view, 0, 1920, 1080, false)
    ));
    assert!(reuse(
        key(view, 2, 1920, 1080, true),
        key(view, 2, 1920, 1080, true)
    ));
    assert!(!reuse(
        key(view, 0, 1920, 1080, false),
        key(view, 0, 1920, 1080, false)
    ));
    assert!(!reuse(
        key(view + 1, 0, 1920, 1080, true),
        key(view, 0, 1920, 1080, false)
    ));
    assert!(!reuse(
        key(view, 1, 1920, 1080, true),
        key(view, 0, 1920, 1080, false)
    ));
    assert!(!reuse(
        key(view, 3, 1920, 1080, true),
        key(view, 2, 1920, 1080, false)
    ));
    assert!(!reuse(
        key(view, 0, 1921, 1080, true),
        key(view, 0, 1920, 1080, false)
    ));
    assert!(!reuse(
        key(view, 0, 1920, 1081, true),
        key(view, 0, 1920, 1080, false)
    ));
    assert!(!reuse(
        key(view, 0, 1080, 1920, true),
        key(view, 0, 1920, 1080, false)
    ));
    assert!(reuse(key(0, 0, 0, 0, true), key(0, 0, 0, 0, false)));
    assert!(reuse(
        key(usize::MAX, -1, u32::MAX, u32::MAX, true),
        key(usize::MAX, -1, u32::MAX, u32::MAX, false)
    ));
    assert!(!reuse(
        key(view, -1, 1920, 1080, true),
        key(view, 0, 1920, 1080, false)
    ));
}

// ---------------------------------------------------------------------------
// edges

/// The C's `long` subtraction with a 64-bit `long` (LP64).
fn close_i64(base: u32, size: u32) -> bool {
    (i64::from(base) - i64::from(size)).abs() <= 16
}

/// The C's with a 32-bit `long` (MSVC): the conversion wraps a size of 2^31
/// or more, and the subtraction or `labs()` would overflow (`None`).
fn close_i32(base: u32, size: u32) -> Option<bool> {
    let (a, b) = (base as i32, size as i32);
    let d = a.checked_sub(b)?;
    Some(d.checked_abs()? <= 16)
}

#[test]
fn edge_resolution_close_long32_every_valid_size() {
    // Every size obs_video_size_valid() allows (2 to 32768), against every
    // size that can be close to it, and the far ones at the ends: a 32-bit
    // long gives the same answer as i64 and abs_diff, with no overflow.
    for base in 2u32..=32768 {
        for size in base.saturating_sub(40).max(2)..=(base + 40).min(32768) {
            let want = close_i64(base, size);
            assert_eq!(close_i32(base, size), Some(want), "{base} {size}");
            assert_eq!(resolution_close(base, 2, size, 2), want, "{base} {size}");
            assert_eq!(resolution_close(2, base, 2, size), want, "{base} {size}");
        }
        for size in [2, 32768] {
            assert_eq!(close_i32(base, size), Some(close_i64(base, size)));
            assert_eq!(
                resolution_close(base, base, size, size),
                close_i64(base, size)
            );
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    #[test]
    fn edge_resolution_close_is_i64(bw: u32, bh: u32, w: u32, h: u32, dw in -20i64..=20, dh in -20i64..=20) {
        prop_assert_eq!(resolution_close(bw, bh, w, h), close_i64(bw, w) && close_i64(bh, h));
        let near_w = u32::try_from(i64::from(bw) + dw).unwrap_or(bw);
        let near_h = u32::try_from(i64::from(bh) + dh).unwrap_or(bh);
        prop_assert_eq!(
            resolution_close(bw, bh, near_w, near_h),
            close_i64(bw, near_w) && close_i64(bh, near_h)
        );
    }

    #[test]
    fn edge_resolution_close_long32_below_2_31(bw in 0u32..0x8000_0000, w in 0u32..0x8000_0000, d in -20i64..=20) {
        let near = u32::try_from(i64::from(bw) + d).unwrap_or(bw).min(0x7fff_ffff);
        for size in [w, near] {
            prop_assert_eq!(close_i32(bw, size), Some(close_i64(bw, size)));
            prop_assert_eq!(resolution_close(bw, 0, size, 0), close_i64(bw, size));
        }
    }

    #[test]
    fn edge_gpu_plan_matches_its_shape(
        f in 0usize..VideoFormat::ALL.len(),
        w: u32,
        h: u32,
        input: [u32; 3],
        output: [u32; 3],
        tight_mask in 0u32..64,
    ) {
        let format = VideoFormat::ALL[f];
        let packed = input[1] == 0;
        let list: &[FormatShape] = if packed { &PACKED_SHAPES } else { &SHAPES };
        let fs = list
            .iter()
            .find(|fs| fs.format == format)
            .or_else(|| SHAPES.iter().find(|fs| fs.format == format));
        let Some(fs) = fs else {
            prop_assert_eq!(plan_of(Some(format), w, h, &input, &output).count, 0);
            return Ok(());
        };
        // bias each line size to the row size, so the single copy is taken
        let (tin, tout) = tight(fs, w);
        let (mut input, mut output) = (input, output);
        for l in 0..3 {
            if tight_mask & (1 << l) != 0 && !(packed && l == 1) {
                input[l] = tin[l];
            }
            if tight_mask & (1 << (l + 3)) != 0 {
                output[l] = tout[l];
            }
        }
        if !packed && input[1] == 0 {
            input[1] = 1;
        }
        check_shape(fs, w, h, &input, &output);
    }

    #[test]
    fn edge_gpu_plan_outside_the_enum(w: u32, h: u32, input: [u32; 3], output: [u32; 3]) {
        prop_assert_eq!(plan_of(None, w, h, &input, &output), CopyPlan::default());
    }

    #[test]
    fn edge_rgbx_plan(w: u32, h: u32, l_in: u32, l_out: u32, same: bool) {
        let l_out = if same { l_in } else { l_out };
        let c = rgbx_copy_plan(w, h, l_in, l_out);
        let whole = l_in == l_out;
        let bytes = if whole {
            (l_in as usize).wrapping_mul(h as usize)
        } else {
            (w as usize).wrapping_mul(4)
        };
        prop_assert_eq!(c, FrameCopy {
            src_after_plane0: false,
            whole,
            bytes,
            rows: h,
            src_stride: l_in,
            dst_stride: l_out,
        });
    }

    #[test]
    fn edge_scale_effect_full_range(
        bw: u32, bh: u32, tw: u32, th: u32, ow: u32, oh: u32,
        t in 0usize..EVERY_TYPE.len(),
        loaded: u32,
        near in any::<bool>(),
    ) {
        let (tw, th) = if near { (bw.wrapping_add(tw % 33).wrapping_sub(16), bh.wrapping_add(th % 33).wrapping_sub(16)) } else { (tw, th) };
        let got = choose(bw, bh, tw, th, ow, oh, EVERY_TYPE[t], loaded);
        let close = close_i64(bw, tw) && close_i64(bh, th);
        let lowres = ow < bw / 2 && oh < bh / 2;
        let want = if close { ScaleEffect::Default } else { expected(EVERY_TYPE[t], lowres, loaded) };
        prop_assert_eq!(got, want);
    }

    #[test]
    fn edge_mix_reuse_full_range(view: usize, space: i32, w: u32, h: u32, rendered: bool, mix_rendered: bool, flips in 0u32..32) {
        let other = key(view, space, w, h, rendered);
        let mut mix = key(view, space, w, h, mix_rendered);
        if flips & 1 != 0 { mix.view = mix.view.wrapping_add(1); }
        if flips & 2 != 0 { mix.render_space = mix.render_space.wrapping_add(1); }
        if flips & 4 != 0 { mix.base_width ^= 1; }
        if flips & 8 != 0 { mix.base_height ^= 0x8000_0000; }
        let want = flips & 15 == 0 && rendered;
        prop_assert_eq!(reuse(other, mix), want);
    }
}
