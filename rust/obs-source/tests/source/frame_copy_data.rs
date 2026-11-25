//! Tier 1: `obs_source::frame_copy_data`, `copy_frame_data()` of
//! `obs_source_frame_copy()`.
//!
//! The `test_source_frame_copy_*` cases port
//! `test/cmocka/test_source_frame_copy.c` 1:1, under the same case names,
//! with the same formats, sizes, line sizes, fills and expectations: every
//! frame pair has the same format, width and height, the source laid out as
//! `obs_source_frame_create()` lays it out.
//!
//! The `edge_*` cases are new: the order of the writes, [`plane_extent`]
//! for one copy, no rows, no bytes and row starts that wrap in `u32`, and
//! [`copy_plane`] on slices too short for a row.

use obs_media_io::video_frame::{linesizes, plane_heights};
use obs_media_io::video_io::video_format_from_c;
use obs_source::frame_copy::{PLANES, PlaneCopy, video_format as F};
use obs_source::frame_copy_data::{
    ColorRange, Destination, Frames, Metadata, Source, copy_frame_data, copy_plane, plane_extent,
};

const FILL_DST: u8 = 0xEE;

fn fill_src(plane: usize, i: usize) -> u8 {
    (plane * 61 + i * 7 + 3) as u8
}

/// The C test's `formats`: the planes `obs_source_frame_create()` gives,
/// how many `copy_frame_data()` copies, and which are 4:2:0 chroma.
struct FormatCase {
    format: i32,
    planes: usize,
    copied: usize,
    half: [bool; PLANES],
}

const fn case(format: i32, planes: usize, copied: usize, half: [bool; PLANES]) -> FormatCase {
    FormatCase {
        format,
        planes,
        copied,
        half,
    }
}

const NO: [bool; PLANES] = [false; PLANES];
const HALF3: [bool; PLANES] = [false, true, true, false];
const HALF2: [bool; PLANES] = [false, true, false, false];

const FORMATS: [FormatCase; 25] = [
    case(F::I420, 3, 3, HALF3),
    case(F::NV12, 2, 2, HALF2),
    case(F::YVYU, 1, 1, NO),
    case(F::YUY2, 1, 1, NO),
    case(F::UYVY, 1, 1, NO),
    case(F::RGBA, 1, 1, NO),
    case(F::BGRA, 1, 1, NO),
    case(F::BGRX, 1, 1, NO),
    case(F::Y800, 1, 1, NO),
    case(F::I444, 3, 3, NO),
    case(F::BGR3, 1, 1, NO),
    case(F::I422, 3, 3, NO),
    case(F::I40A, 4, 4, HALF3),
    case(F::I42A, 4, 4, NO),
    case(F::YUVA, 4, 4, NO),
    case(F::AYUV, 1, 1, NO),
    case(F::I010, 3, 3, HALF3),
    case(F::P010, 2, 2, HALF2),
    case(F::I210, 3, 3, NO),
    case(F::I412, 3, 3, NO),
    case(F::YA2L, 4, 4, NO),
    // Characterized, not endorsed: P216 and P416 copy no plane.
    case(F::P216, 2, 0, NO),
    case(F::P416, 2, 0, NO),
    case(F::V210, 1, 1, NO),
    case(F::R10L, 1, 1, NO),
];

const SIZES: [(u32, u32); 4] = [(1, 1), (13, 7), (32, 16), (5, 2)];

fn plane_rows(fc: &FormatCase, plane: usize, height: u32) -> u32 {
    if fc.half[plane] {
        height.div_ceil(2)
    } else {
        height
    }
}

/// A frame: its metadata, colour range, format, size, line sizes and one
/// buffer per plane.
#[derive(Debug, Clone, PartialEq)]
struct Frame {
    metadata: Metadata,
    color_range: ColorRange,
    format: i32,
    width: u32,
    height: u32,
    linesize: [u32; PLANES],
    planes: Vec<Vec<u8>>,
}

impl Frame {
    /// `obs_source_frame_create()`'s layout, every plane exactly line size x
    /// rows (the frame's planes are one block; each plane's part of it).
    fn create(fc: &FormatCase, width: u32, height: u32) -> Self {
        let format = video_format_from_c(fc.format).unwrap();
        let lines = linesizes(format, width);
        let heights = plane_heights(format, height);
        let mut linesize = [0; PLANES];
        let mut planes = Vec::new();
        for p in 0..fc.planes {
            assert_eq!(heights[p], plane_rows(fc, p, height));
            linesize[p] = lines[p];
            planes.push(vec![0; lines[p] as usize * heights[p] as usize]);
        }
        Self {
            metadata: Metadata::default(),
            color_range: ColorRange::default(),
            format: fc.format,
            width,
            height,
            linesize,
            planes,
        }
    }

    /// The C test's `build_destination()`: the format and size, its own line
    /// sizes, each plane filled with `FILL_DST`.
    fn build(fc: &FormatCase, width: u32, height: u32, linesize: [u32; PLANES]) -> Self {
        let planes = (0..fc.planes)
            .map(|p| vec![FILL_DST; linesize[p] as usize * plane_rows(fc, p, height) as usize])
            .collect();
        Self {
            metadata: Metadata::default(),
            color_range: ColorRange::default(),
            format: fc.format,
            width,
            height,
            linesize,
            planes,
        }
    }

    fn fill_source(&mut self) {
        for (p, plane) in self.planes.iter_mut().enumerate() {
            for (i, byte) in plane.iter_mut().enumerate() {
                *byte = fill_src(p, i);
            }
        }
    }

    fn fill_destination(&mut self) {
        for plane in &mut self.planes {
            plane.fill(FILL_DST);
        }
    }
}

struct Pair<'a> {
    dst: &'a mut Frame,
    src: &'a Frame,
}

impl Frames for Pair<'_> {
    fn write_metadata(&mut self, metadata: Metadata) {
        self.dst.metadata = metadata;
    }

    fn write_color_range(&mut self, range: ColorRange) {
        self.dst.color_range = range;
    }

    fn copy_plane(&mut self, plane: usize, copy: &PlaneCopy) {
        let (dst, src) = (&mut self.dst.planes[plane], &self.src.planes[plane]);
        // Every row the plan copies lies inside both planes.
        let (dst_len, src_len) = plane_extent(copy);
        assert!(
            dst_len <= dst.len(),
            "plane {plane}: {dst_len} > {}",
            dst.len()
        );
        assert!(
            src_len <= src.len(),
            "plane {plane}: {src_len} > {}",
            src.len()
        );
        copy_plane(dst, src, copy);
    }
}

/// `obs_source_frame_copy(dst, src)`.
fn frame_copy(dst: &mut Frame, src: &Frame) {
    let source = Source {
        metadata: src.metadata,
        color_range: src.color_range,
        format: src.format,
        linesize: src.linesize,
    };
    let shape = Destination {
        height: dst.height,
        linesize: dst.linesize,
    };
    copy_frame_data(&mut Pair { dst, src }, &source, shape);
}

/// The C test's `check_planes()`.
fn check_planes(dst: &Frame, src: &Frame, fc: &FormatCase) {
    for p in 0..fc.planes {
        let rows = plane_rows(fc, p, dst.height) as usize;
        let (dst_line, src_line) = (dst.linesize[p] as usize, src.linesize[p] as usize);
        let bytes = dst_line.min(src_line);
        for y in 0..rows {
            let row = &dst.planes[p][y * dst_line..][..dst_line];
            let from = &src.planes[p][y * src_line..];
            for (x, &got) in row.iter().enumerate() {
                let want = if p < fc.copied && x < bytes {
                    from[x]
                } else {
                    FILL_DST
                };
                assert_eq!(
                    got, want,
                    "format {} {}x{} plane {p} row {y} byte {x}",
                    fc.format, dst.width, dst.height
                );
            }
        }
    }
}

fn check_equal_line_sizes(fc: &FormatCase, width: u32, height: u32) {
    let mut src = Frame::create(fc, width, height);
    let mut dst = Frame::create(fc, width, height);
    src.fill_source();
    assert_eq!(dst.linesize, src.linesize);
    dst.fill_destination();

    frame_copy(&mut dst, &src);
    check_planes(&dst, &src, fc);
}

#[test]
fn test_source_frame_copy_every_format_equal_line_sizes() {
    for fc in &FORMATS {
        for (width, height) in SIZES {
            check_equal_line_sizes(fc, width, height);
        }
    }
}

fn check_unequal_line_sizes(fc: &FormatCase, width: u32, height: u32, delta: [i64; PLANES]) {
    let mut src = Frame::create(fc, width, height);
    src.fill_source();
    let mut linesize = [0; PLANES];
    for p in 0..fc.planes {
        linesize[p] = (i64::from(src.linesize[p]) + delta[p]).max(1) as u32;
    }
    let mut dst = Frame::build(fc, width, height, linesize);

    frame_copy(&mut dst, &src);
    check_planes(&dst, &src, fc);
    assert_eq!(dst.linesize, linesize);
}

#[test]
fn test_source_frame_copy_every_format_unequal_line_sizes() {
    const DELTAS: [[i64; PLANES]; 5] = [
        [16, 16, 16, 16],
        [-1, -1, -1, -1],
        [5, -1, 3, -1],
        [-3, 0, 7, 0],
        [0, 9, -1, 2],
    ];
    for fc in &FORMATS {
        for (width, height) in SIZES {
            for delta in DELTAS {
                check_unequal_line_sizes(fc, width, height, delta);
            }
        }
    }
}

const MATRIX_BITS: [u32; 16] = [
    0x3f80_0000,
    0x0000_0000,
    0x8000_0000,
    0x7f80_0000,
    0xff80_0000,
    0x7fc0_0001,
    0x7f80_0001,
    0x0000_0001,
    0x3eaa_aaab,
    0xbf00_0000,
    0x4120_0000,
    0x1234_5678,
    0xffff_ffff,
    0x7f7f_ffff,
    0x0080_0000,
    0xc2c8_0000,
];
const RANGE_MIN_BITS: [u32; 3] = [0x3d80_8081, 0x7fc0_0002, 0x8000_0000];
const RANGE_MAX_BITS: [u32; 3] = [0x3f6b_6b6c, 0x3f70_0000, 0x7f80_0001];

fn set_source_metadata(src: &mut Frame, full_range: bool) {
    src.metadata = Metadata {
        flip: true,
        flags: 0xa5,
        trc: 0x3c,
        full_range,
        max_luminance: 0xbeef,
        timestamp: 0x0123_4567_89ab_cdef,
        color_matrix: MATRIX_BITS,
    };
    src.color_range = ColorRange {
        min: RANGE_MIN_BITS,
        max: RANGE_MAX_BITS,
    };
}

fn set_destination_metadata(dst: &mut Frame, full_range: bool) {
    dst.metadata = Metadata {
        flip: false,
        flags: 0x11,
        trc: 0x01,
        full_range,
        max_luminance: 7,
        timestamp: 42,
        color_matrix: [0x2222_2222; 16],
    };
    dst.color_range = ColorRange {
        min: [0x3333_3333; 3],
        max: [0x4444_4444; 3],
    };
}

const NV12: usize = 1;

fn check_metadata(src_full: bool, dst_full: bool) {
    let fc = &FORMATS[NV12];
    let mut src = Frame::create(fc, 6, 4);
    let mut dst = Frame::create(fc, 6, 4);
    set_source_metadata(&mut src, src_full);
    set_destination_metadata(&mut dst, dst_full);
    let before = dst.clone();
    let source = src.clone();

    frame_copy(&mut dst, &src);

    assert_eq!(dst.metadata, src.metadata);
    if src_full {
        assert_eq!(dst.color_range, before.color_range);
    } else {
        assert_eq!(dst.color_range.min, RANGE_MIN_BITS);
        assert_eq!(dst.color_range.max, RANGE_MAX_BITS);
    }
    assert_eq!(dst.format, before.format);
    assert_eq!(dst.width, before.width);
    assert_eq!(dst.height, before.height);
    assert_eq!(dst.linesize, before.linesize);
    // The source is only read.
    assert_eq!(src, source);
}

#[test]
fn test_source_frame_copy_metadata_and_colour_range() {
    check_metadata(false, false);
    check_metadata(false, true);
    check_metadata(true, false);
    check_metadata(true, true);
}

#[test]
fn test_source_frame_copy_metadata_copy_also_copies_planes() {
    let fc = &FORMATS[NV12];
    let mut src = Frame::create(fc, 6, 4);
    let mut dst = Frame::create(fc, 6, 4);
    src.fill_source();
    set_source_metadata(&mut src, false);
    dst.fill_destination();

    frame_copy(&mut dst, &src);
    check_planes(&dst, &src, fc);
    assert_eq!(dst.color_range.min, RANGE_MIN_BITS);
}

/// Records the order of the writes.
#[derive(Default)]
struct Recorder {
    calls: Vec<String>,
}

impl Frames for Recorder {
    fn write_metadata(&mut self, metadata: Metadata) {
        self.calls.push(format!("metadata {}", metadata.full_range));
    }

    fn write_color_range(&mut self, _range: ColorRange) {
        self.calls.push("range".into());
    }

    fn copy_plane(&mut self, plane: usize, copy: &PlaneCopy) {
        self.calls.push(format!("plane {plane} {}", copy.rows));
    }
}

#[test]
fn edge_metadata_then_range_then_planes_in_order() {
    let mut src = Source {
        format: F::I40A,
        linesize: [8, 4, 4, 8],
        ..Source::default()
    };
    let dst = Destination {
        height: 5,
        linesize: [8, 4, 4, 8],
    };
    let mut rec = Recorder::default();
    copy_frame_data(&mut rec, &src, dst);
    assert_eq!(
        rec.calls,
        [
            "metadata false",
            "range",
            "plane 0 5",
            "plane 1 3",
            "plane 2 3",
            "plane 3 5"
        ]
    );

    // A full-range source writes no range, whatever the destination was.
    src.metadata.full_range = true;
    let mut rec = Recorder::default();
    copy_frame_data(&mut rec, &src, dst);
    assert_eq!(rec.calls[..2], ["metadata true", "plane 0 5"]);

    // P216 copies no plane.
    src.format = F::P216;
    let mut rec = Recorder::default();
    copy_frame_data(&mut rec, &src, dst);
    assert_eq!(rec.calls, ["metadata true"]);
}

fn rows(rows: u32, src_stride: u32, dst_stride: u32, bytes: usize) -> PlaneCopy {
    PlaneCopy {
        whole: false,
        bytes,
        rows,
        src_stride,
        dst_stride,
    }
}

#[test]
fn edge_plane_extent() {
    let whole = PlaneCopy {
        whole: true,
        bytes: 1920 * 1080,
        rows: 1080,
        src_stride: 1920,
        dst_stride: 1920,
    };
    assert_eq!(plane_extent(&whole), (1920 * 1080, 1920 * 1080));
    assert_eq!(
        plane_extent(&rows(4, 10, 12, 10)),
        (3 * 12 + 10, 3 * 10 + 10)
    );
    assert_eq!(
        plane_extent(&rows(4, 12, 10, 10)),
        (3 * 10 + 10, 3 * 12 + 10)
    );
    assert_eq!(plane_extent(&rows(1, 12, 10, 10)), (10, 10));
    assert_eq!(plane_extent(&rows(0, 12, 10, 10)), (0, 0));
    assert_eq!(plane_extent(&rows(5, 12, 10, 0)), (0, 0));
    // Row starts wrap in u32: rows 0..3 at 2^31 start at 0, 2^31, 0.
    assert_eq!(
        plane_extent(&rows(3, 0x8000_0000, 1, 1)),
        (2 + 1, 0x8000_0000 + 1)
    );
    // The furthest row is not the last one once the starts wrap.
    let stride = 0x6000_0000u32;
    let furthest = (0..4u32).map(|y| y.wrapping_mul(stride)).max().unwrap() as usize;
    assert_eq!(furthest, 0xc000_0000);
    assert_eq!(plane_extent(&rows(4, stride, 1, 7)).1, furthest + 7);
}

#[test]
fn edge_copy_plane_rows_and_whole() {
    let src: Vec<u8> = (0..40).collect();
    let mut dst = vec![0xff; 40];
    copy_plane(&mut dst, &src, &rows(3, 10, 12, 4));
    for y in 0..3 {
        assert_eq!(dst[y * 12..y * 12 + 4], src[y * 10..y * 10 + 4]);
        assert!(dst[y * 12 + 4..y * 12 + 12].iter().all(|&b| b == 0xff));
    }
    assert!(dst[28..].iter().all(|&b| b == 0xff));

    let mut dst = vec![0xff; 40];
    let whole = PlaneCopy {
        whole: true,
        bytes: 30,
        rows: 3,
        src_stride: 10,
        dst_stride: 10,
    };
    copy_plane(&mut dst, &src, &whole);
    assert_eq!(dst[..30], src[..30]);
    assert!(dst[30..].iter().all(|&b| b == 0xff));
}

#[test]
fn edge_copy_plane_short_slices_copy_what_fits() {
    let src: Vec<u8> = (0..20).collect();
    let mut dst = vec![0xff; 15];
    // Rows 0 and 1 fit; row 2 (dst 12..17) does not and is skipped.
    copy_plane(&mut dst, &src, &rows(3, 6, 6, 5));
    assert_eq!(dst[..5], src[..5]);
    assert_eq!(dst[6..11], src[6..11]);
    assert!(dst[12..].iter().all(|&b| b == 0xff));

    let mut dst = vec![0xff; 4];
    let whole = PlaneCopy {
        whole: true,
        bytes: 8,
        rows: 1,
        src_stride: 8,
        dst_stride: 8,
    };
    copy_plane(&mut dst, &src, &whole);
    assert_eq!(dst, [0xff; 4]);
}
