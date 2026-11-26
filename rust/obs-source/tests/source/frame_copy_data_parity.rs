//! Tier 3: the Rust `obs_source_frame_copy()` against the original C
//! (`obs-c-oracle`, `oracle/source_frame_copy_data.c`: the unmodified
//! `libobs/obs-source-frame-copy-data.c.h` with `obs_source_frame_copy`
//! renamed). Both ask the same `obs_source_frame_copy_plan()`.
//!
//! Every case builds the same destination twice and one source, runs the C
//! on one destination and the Rust on the other, and compares every field
//! of the two destinations (the colour values as bits), every byte of their
//! planes and of a guard after each plane, and the source, which neither
//! may change.
//!
//! The frames are what libobs's callers pass: a source of any format that
//! has planes, laid out as `obs_source_frame_create()` lays it out, and a
//! destination of the same format, width and height whose line sizes are
//! the source's or differ per plane (longer or shorter, never 0), each of
//! its planes line size x rows.
//!
//! Runs:
//! - the layout of `struct obs_source_frame` against the C header;
//! - every format at seven sizes against the C test's five line-size
//!   patterns and equal line sizes, both ranges;
//! - a proptest (512 cases) over the format, the size up to 48 x 24, each
//!   plane's line size, every metadata field and the plane bytes.

use core::ffi::c_long;
use std::mem::{align_of, offset_of, size_of};

use obs_media_io::video_frame::{linesizes, plane_heights};
use obs_media_io::video_io::video_format_from_c;
use obs_source::ffi::source_frame_copy_data::{
    MAX_AV_PLANES, obs_source_frame, obs_source_frame_copy,
};
use obs_source::frame_copy::{PLANES, video_format as F};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_obs_source_frame_copy(dst: *mut obs_source_frame, src: *const obs_source_frame);
    fn oracle_source_frame_copy_data_layout(which: usize) -> usize;
}

#[test]
fn layout_matches_c() {
    let rust = [
        size_of::<obs_source_frame>(),
        align_of::<obs_source_frame>(),
        offset_of!(obs_source_frame, data),
        offset_of!(obs_source_frame, linesize),
        offset_of!(obs_source_frame, width),
        offset_of!(obs_source_frame, height),
        offset_of!(obs_source_frame, timestamp),
        offset_of!(obs_source_frame, format),
        offset_of!(obs_source_frame, color_matrix),
        offset_of!(obs_source_frame, full_range),
        offset_of!(obs_source_frame, max_luminance),
        offset_of!(obs_source_frame, color_range_min),
        offset_of!(obs_source_frame, color_range_max),
        offset_of!(obs_source_frame, flip),
        offset_of!(obs_source_frame, flags),
        offset_of!(obs_source_frame, trc),
        offset_of!(obs_source_frame, refs),
        offset_of!(obs_source_frame, prev_frame),
        size_of::<core::ffi::c_int>(),
        MAX_AV_PLANES,
    ];
    for (i, &value) in rust.iter().enumerate() {
        // SAFETY: a pure query of constants.
        assert_eq!(
            unsafe { oracle_source_frame_copy_data_layout(i) },
            value,
            "entry {i}"
        );
    }
    // SAFETY: as above.
    assert_eq!(
        unsafe { oracle_source_frame_copy_data_layout(rust.len()) },
        usize::MAX
    );
}

/// Every format that has planes.
const FORMATS: [i32; 25] = [
    F::I420,
    F::NV12,
    F::YVYU,
    F::YUY2,
    F::UYVY,
    F::RGBA,
    F::BGRA,
    F::BGRX,
    F::Y800,
    F::I444,
    F::BGR3,
    F::I422,
    F::I40A,
    F::I42A,
    F::YUVA,
    F::AYUV,
    F::I010,
    F::P010,
    F::I210,
    F::I412,
    F::YA2L,
    F::P216,
    F::P416,
    F::V210,
    F::R10L,
];

/// Bytes after each plane that no copy may touch.
const GUARD: usize = 32;

/// The fields a frame carries besides its planes, as bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Fields {
    flip: bool,
    flags: u8,
    trc: u8,
    full_range: bool,
    max_luminance: u16,
    timestamp: u64,
    color_matrix: [u32; 16],
    color_range_min: [u32; 3],
    color_range_max: [u32; 3],
    refs: c_long,
    prev_frame: bool,
}

/// A frame and the buffers its planes point into.
struct Owned {
    frame: obs_source_frame,
    planes: Vec<Vec<u8>>,
}

impl Owned {
    fn new(format: i32, width: u32, height: u32, linesize: [u32; PLANES], fill: &mut Fill) -> Self {
        let heights = plane_heights(video_format_from_c(format).unwrap(), height);
        let mut planes: Vec<Vec<u8>> = (0..PLANES)
            .filter(|&p| heights[p] != 0 && linesize[p] != 0)
            .map(|p| {
                let len = linesize[p] as usize * heights[p] as usize + GUARD;
                (0..len).map(|_| fill.byte()).collect()
            })
            .collect();
        let mut data = [core::ptr::null_mut(); MAX_AV_PLANES];
        let mut lines = [0; MAX_AV_PLANES];
        for (p, plane) in planes.iter_mut().enumerate() {
            data[p] = plane.as_mut_ptr();
            lines[p] = linesize[p];
        }
        let frame = obs_source_frame {
            data,
            linesize: lines,
            width,
            height,
            timestamp: 0,
            format,
            color_matrix: [0.0; 16],
            full_range: false,
            max_luminance: 0,
            color_range_min: [0.0; 3],
            color_range_max: [0.0; 3],
            flip: false,
            flags: 0,
            trc: 0,
            refs: 0,
            prev_frame: false,
        };
        Self { frame, planes }
    }

    fn set(&mut self, f: &Fields) {
        let frame = &mut self.frame;
        frame.flip = f.flip;
        frame.flags = f.flags;
        frame.trc = f.trc;
        frame.full_range = f.full_range;
        frame.max_luminance = f.max_luminance;
        frame.timestamp = f.timestamp;
        frame.color_matrix = f.color_matrix.map(f32::from_bits);
        frame.color_range_min = f.color_range_min.map(f32::from_bits);
        frame.color_range_max = f.color_range_max.map(f32::from_bits);
        frame.refs = f.refs;
        frame.prev_frame = f.prev_frame;
    }

    fn fields(&self) -> Fields {
        let frame = &self.frame;
        Fields {
            flip: frame.flip,
            flags: frame.flags,
            trc: frame.trc,
            full_range: frame.full_range,
            max_luminance: frame.max_luminance,
            timestamp: frame.timestamp,
            color_matrix: frame.color_matrix.map(f32::to_bits),
            color_range_min: frame.color_range_min.map(f32::to_bits),
            color_range_max: frame.color_range_max.map(f32::to_bits),
            refs: frame.refs,
            prev_frame: frame.prev_frame,
        }
    }

    /// Everything but the plane bytes: the fields, and the shape and data
    /// pointers, which must still point at this frame's own planes.
    fn check_shape(&self, format: i32, width: u32, height: u32) {
        let frame = &self.frame;
        assert_eq!(
            (frame.format, frame.width, frame.height),
            (format, width, height)
        );
        for (p, plane) in self.planes.iter().enumerate() {
            assert_eq!(frame.data[p], plane.as_ptr().cast_mut());
        }
        assert!(frame.data[self.planes.len()..].iter().all(|d| d.is_null()));
    }
}

/// xorshift64: the plane bytes of a case from its seed.
struct Fill(u64);

impl Fill {
    fn byte(&mut self) -> u8 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 24) as u8
    }
}

struct Case {
    format: i32,
    width: u32,
    height: u32,
    /// Added to each plane's source line size for the destination's (at
    /// least 1).
    delta: [i64; PLANES],
    src: Fields,
    dst: Fields,
    seed: u64,
}

fn check(case: &Case) {
    let (format, width, height) = (case.format, case.width, case.height);
    let src_lines = linesizes(video_format_from_c(format).unwrap(), width);
    let mut linesize = [0; PLANES];
    let mut dst_lines = [0; PLANES];
    for p in 0..PLANES {
        linesize[p] = src_lines[p];
        if src_lines[p] != 0 {
            dst_lines[p] = (i64::from(src_lines[p]) + case.delta[p]).max(1) as u32;
        }
    }
    let mut fill = Fill(case.seed | 1);
    let mut src = Owned::new(format, width, height, linesize, &mut fill);
    src.set(&case.src);
    let mut c = Owned::new(format, width, height, dst_lines, &mut fill);
    c.set(&case.dst);
    let mut rust = Owned::new(format, width, height, dst_lines, &mut fill);
    rust.set(&case.dst);
    rust.planes.clone_from(&c.planes);
    for (p, plane) in rust.planes.iter_mut().enumerate() {
        rust.frame.data[p] = plane.as_mut_ptr();
    }
    let source_before = (src.fields(), src.planes.clone());

    // SAFETY: both destinations and the source are valid frames of one
    // format, width and height; each plane holds line size x rows bytes of
    // its own buffer, so every planned copy fits, and no buffers overlap.
    unsafe {
        oracle_obs_source_frame_copy(&mut c.frame, &src.frame);
        obs_source_frame_copy(&mut rust.frame, &src.frame);
    }

    let what = format!(
        "format {format} {width}x{height} delta {:?} src full {}",
        case.delta, case.src.full_range
    );
    assert_eq!(rust.fields(), c.fields(), "{what}");
    assert_eq!(rust.frame.linesize, c.frame.linesize, "{what}");
    assert!(rust.planes == c.planes, "{what}: planes differ");
    c.check_shape(format, width, height);
    rust.check_shape(format, width, height);
    assert_eq!((src.fields(), src.planes.clone()), source_before, "{what}");
    src.check_shape(format, width, height);
}

fn fields(seed: u32, full_range: bool) -> Fields {
    let bits = |i: u32| {
        seed.wrapping_mul(0x9e37_79b9)
            .wrapping_add(i.wrapping_mul(0x7f4a_7c15))
    };
    Fields {
        flip: seed & 1 == 1,
        flags: seed as u8,
        trc: (seed >> 8) as u8,
        full_range,
        max_luminance: (seed >> 16) as u16,
        timestamp: u64::from(seed) << 29 | 0x1f,
        color_matrix: core::array::from_fn(|i| bits(i as u32)),
        color_range_min: [0x7fc0_0001, bits(20), 0x8000_0000],
        color_range_max: [0x7f80_0001, bits(21), 0xffff_ffff],
        refs: c_long::from(seed as u16),
        prev_frame: seed & 2 == 2,
    }
}

#[test]
fn every_format_size_and_line_size_pattern_matches_c() {
    const SIZES: [(u32, u32); 7] = [(1, 1), (2, 2), (5, 2), (13, 7), (32, 16), (47, 3), (6, 9)];
    const DELTAS: [[i64; PLANES]; 6] = [
        [0, 0, 0, 0],
        [16, 16, 16, 16],
        [-1, -1, -1, -1],
        [5, -1, 3, -1],
        [-3, 0, 7, 0],
        [0, 9, -1, 2],
    ];
    let mut seed = 1u32;
    for format in FORMATS {
        for (width, height) in SIZES {
            for delta in DELTAS {
                for full in [false, true] {
                    seed = seed.wrapping_add(1);
                    check(&Case {
                        format,
                        width,
                        height,
                        delta,
                        src: fields(seed, full),
                        dst: fields(!seed, !full),
                        seed: u64::from(seed),
                    });
                }
            }
        }
    }
}

fn any_fields() -> impl Strategy<Value = Fields> {
    (
        (
            any::<bool>(),
            any::<u8>(),
            any::<u8>(),
            any::<bool>(),
            any::<u16>(),
            any::<u64>(),
        ),
        prop::array::uniform16(any::<u32>()),
        prop::array::uniform3(any::<u32>()),
        prop::array::uniform3(any::<u32>()),
        any::<c_long>(),
        any::<bool>(),
    )
        .prop_map(
            |(
                (flip, flags, trc, full_range, max_luminance, timestamp),
                color_matrix,
                color_range_min,
                color_range_max,
                refs,
                prev_frame,
            )| Fields {
                flip,
                flags,
                trc,
                full_range,
                max_luminance,
                timestamp,
                color_matrix,
                color_range_min,
                color_range_max,
                refs,
                prev_frame,
            },
        )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(512)))]

    #[test]
    fn any_same_shape_frames_match_c(
        format in prop::sample::select(FORMATS.to_vec()),
        width in 1u32..=48,
        height in 1u32..=24,
        delta in prop::array::uniform4(prop_oneof![Just(0i64), -64i64..=64]),
        src in any_fields(),
        dst in any_fields(),
        seed in any::<u64>(),
    ) {
        check(&Case { format, width, height, delta, src, dst, seed });
    }
}
