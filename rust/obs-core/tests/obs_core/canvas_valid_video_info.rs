//! Tier 1: `obs_core::canvas_valid_video_info`, the safe core of
//! `obs_canvas_has_valid_video_info()`.
//!
//! The `characterization_*` cases port every assertion of the unchanged
//! `test/cmocka/test_canvas_valid_video_info.c`: each dimension's zero and
//! nonzero boundaries on its own, the format's NONE, in-enum and
//! out-of-enum signed values, and all 32 combinations of the five
//! conjuncts, with no change to the fields and no allocation. The C
//! fixture's whole-canvas byte comparison is the unchanged field values
//! here (the core sees only the five fields); Tier 3 compares the real
//! canvas bytes through the shim.
//!
//! The `edge_*` cases are new: the read order and short circuit (a field
//! is read only once every earlier conjunct held, and each at most once),
//! and a proptest over any five values against the C expression.

use core::ffi::c_int;

use obs_core::canvas_valid_video_info::{VideoInfoFields, has_valid_video_info};
use proptest::prelude::*;

use super::alloc_count::allocs_during;

/// `VIDEO_FORMAT_NONE`, `_I420` and `_RGBA` (`media-io/video-io.h`).
const VIDEO_FORMAT_NONE: c_int = 0;
const VIDEO_FORMAT_I420: c_int = 1;
const VIDEO_FORMAT_RGBA: c_int = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Field {
    BaseWidth,
    BaseHeight,
    OutputWidth,
    OutputHeight,
    OutputFormat,
}

const ORDER: [Field; 5] = [
    Field::BaseWidth,
    Field::BaseHeight,
    Field::OutputWidth,
    Field::OutputHeight,
    Field::OutputFormat,
];

/// A canvas's video info, recording each field read.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Info {
    base_width: u32,
    base_height: u32,
    output_width: u32,
    output_height: u32,
    output_format: c_int,
    reads: Vec<Field>,
}

impl Info {
    /// `valid_canvas()` of the C fixture.
    fn valid() -> Self {
        Self {
            base_width: 1920,
            base_height: 1080,
            output_width: 1280,
            output_height: 720,
            output_format: VIDEO_FORMAT_I420,
            reads: Vec::with_capacity(ORDER.len()),
        }
    }

    fn dimension(&mut self, field: Field) -> &mut u32 {
        match field {
            Field::BaseWidth => &mut self.base_width,
            Field::BaseHeight => &mut self.base_height,
            Field::OutputWidth => &mut self.output_width,
            Field::OutputHeight => &mut self.output_height,
            Field::OutputFormat => unreachable!("not a dimension"),
        }
    }
}

impl VideoInfoFields for Info {
    fn base_width(&mut self) -> u32 {
        self.reads.push(Field::BaseWidth);
        self.base_width
    }
    fn base_height(&mut self) -> u32 {
        self.reads.push(Field::BaseHeight);
        self.base_height
    }
    fn output_width(&mut self) -> u32 {
        self.reads.push(Field::OutputWidth);
        self.output_width
    }
    fn output_height(&mut self) -> u32 {
        self.reads.push(Field::OutputHeight);
        self.output_height
    }
    fn output_format(&mut self) -> c_int {
        self.reads.push(Field::OutputFormat);
        self.output_format
    }
}

/// `check_validity()`: the result, no field changed, no allocation.
fn check_validity(info: &mut Info, expected: bool) {
    let mut before = info.clone();
    info.reads.clear();
    before.reads.clear();
    let mut actual = None;
    let allocations = allocs_during(|| actual = Some(has_valid_video_info(info)));
    assert_eq!(actual, Some(expected), "{info:?}");
    assert_eq!(allocations, 0);
    // The recorder's capacity was reserved up front, so its pushes do not
    // allocate; keep it for the next case.
    let reads = info.reads.clone();
    info.reads.clear();
    assert_eq!(*info, before);
    // Only the C's left-to-right prefix is read, each field once.
    assert!(reads.len() <= ORDER.len());
    assert_eq!(reads, ORDER[..reads.len()]);
}

/// `dimension_boundaries()`: 0 is unset; 1, 2, half and all of `u32` are set.
fn dimension_boundaries(field: Field) {
    let cases = [
        (0, false),
        (1, true),
        (2, true),
        (u32::MAX / 2, true),
        (u32::MAX, true),
    ];
    let mut info = Info::valid();
    for (value, expected) in cases {
        *info.dimension(field) = value;
        check_validity(&mut info, expected);
    }
}

// ---------------------------------------------------------------------------
// the C's cases

#[test]
fn characterization_base_width() {
    dimension_boundaries(Field::BaseWidth);
}

#[test]
fn characterization_base_height() {
    dimension_boundaries(Field::BaseHeight);
}

#[test]
fn characterization_output_width() {
    dimension_boundaries(Field::OutputWidth);
}

#[test]
fn characterization_output_height() {
    dimension_boundaries(Field::OutputHeight);
}

#[test]
fn characterization_output_format() {
    // Out-of-enum formats pass today: characterized, not endorsed.
    let cases = [
        (VIDEO_FORMAT_NONE, false),
        (VIDEO_FORMAT_I420, true),
        (VIDEO_FORMAT_RGBA, true),
        (c_int::MIN, true),
        (-1, true),
        (c_int::MAX, true),
    ];
    let mut info = Info::valid();
    for (value, expected) in cases {
        info.output_format = value;
        check_validity(&mut info, expected);
    }
}

#[test]
fn characterization_conjunction() {
    for mask in 0..32u32 {
        let mut info = Info::valid();
        info.base_width = u32::from(mask & 1 != 0);
        info.base_height = u32::from(mask & 2 != 0);
        info.output_width = u32::from(mask & 4 != 0);
        info.output_height = u32::from(mask & 8 != 0);
        info.output_format = if mask & 16 != 0 {
            VIDEO_FORMAT_I420
        } else {
            VIDEO_FORMAT_NONE
        };
        check_validity(&mut info, mask == 31);
    }
}

// ---------------------------------------------------------------------------
// new edges

/// The first unset conjunct stops the reads: every field up to and including
/// it is read, in order, and nothing after it.
#[test]
fn edge_short_circuit_reads() {
    for (stop, field) in ORDER.iter().enumerate() {
        let mut info = Info::valid();
        match field {
            Field::OutputFormat => info.output_format = VIDEO_FORMAT_NONE,
            dimension => *info.dimension(*dimension) = 0,
        }
        assert!(!has_valid_video_info(&mut info));
        assert_eq!(info.reads, ORDER[..=stop]);
    }
    let mut info = Info::valid();
    assert!(has_valid_video_info(&mut info));
    assert_eq!(info.reads, ORDER);
}

proptest! {
    #[test]
    fn edge_any_values_match_the_c_expression(
        base_width in any::<u32>(),
        base_height in any::<u32>(),
        output_width in any::<u32>(),
        output_height in any::<u32>(),
        output_format in any::<c_int>(),
        zeros in 0..32u32,
    ) {
        let pick = |bit: u32, value: u32| if zeros & bit != 0 { 0 } else { value };
        let mut info = Info {
            base_width: pick(1, base_width),
            base_height: pick(2, base_height),
            output_width: pick(4, output_width),
            output_height: pick(8, output_height),
            output_format: if zeros & 16 != 0 { VIDEO_FORMAT_NONE } else { output_format },
            reads: Vec::with_capacity(ORDER.len()),
        };
        let expected = info.base_width != 0
            && info.base_height != 0
            && info.output_width != 0
            && info.output_height != 0
            && info.output_format != VIDEO_FORMAT_NONE;
        check_validity(&mut info, expected);
    }
}
