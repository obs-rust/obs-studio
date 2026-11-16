//! Tier 3 parity: the shims of `obs_core::ffi::video_frame_copy` (and the
//! safe core under them) against the original
//! `libobs/obs-video-frame-copy.c.h`, compiled as `oracle_obs_video_*`
//! (obs-c-oracle, oracle/video_frame_copy.c) over the C copy plan
//! (oracle/video_copy_plan.c).
//!
//! Both sides get the same frames: input planes of random bytes, output
//! planes of a fill byte, each with slack after what the plan reads and
//! writes. Every byte of every output plane is compared, the padding after
//! each row and the slack included, so a copy that writes too much, too
//! little or from the wrong place differs.
//!
//! Runs: every C format value (the enum, `VIDEO_FORMAT_R10L + 1`, -1, and
//! the `int` ends) over widths and heights at 0, 1, odd and even, with each
//! plane's line sizes equal to its row (one whole copy) or past it (a row at
//! a time), and NV12 and P010 also packed (input `linesize[1]` of 0, the UV
//! plane after plane 0); the RGBX copy over the same sizes; and proptests
//! over random formats, sizes up to 48 x 32, line sizes from the row to 40
//! past it, and the packed frames. Only plans the copy plan makes are fed,
//! with planes that hold what they read and write, as libobs's frames do.
//!
//! No intentional difference.

use core::ffi::{c_int, c_void};
use core::ptr;

use obs_c_oracle::video_frame_copy as oracle;
use obs_core::ffi::video_frame_copy::{
    obs_video_copy_rgbx_frame, obs_video_set_gpu_converted_data,
};
use obs_media_io::ffi::video_frame::video_frame;
use obs_media_io::ffi::video_io::{video_data, video_output_info};
use obs_media_io::video_io::{MAX_AV_PLANES, VideoFormat, video_format_from_c};
use proptest::prelude::*;

const FILL: u8 = 0xEE;
const SLACK: usize = 9;

/// Each plane's row bytes and rows of a GPU-converted frame of `format`, as
/// `gpu_copy_plan()` copies them; no planes for a format with no plan.
fn planes(format: c_int, width: u32, height: u32) -> Vec<(u32, u32)> {
    let (w, h) = (width, height);
    match video_format_from_c(format) {
        Some(VideoFormat::I420) => vec![(w, h), (w / 2, h / 2), (w / 2, h / 2)],
        Some(VideoFormat::Nv12) => vec![(w, h), (w, h / 2)],
        Some(VideoFormat::I444) => vec![(w, h), (w, h), (w, h)],
        Some(VideoFormat::I010) => vec![(2 * w, h), (w, h / 2), (w, h / 2)],
        Some(VideoFormat::P010) => vec![(2 * w, h), (2 * w, h / 2)],
        Some(VideoFormat::P216) => vec![(2 * w, h), (2 * w, h)],
        Some(VideoFormat::P416) => vec![(2 * w, h), (4 * w, h)],
        _ => Vec::new(),
    }
}

fn packable(format: c_int) -> bool {
    matches!(
        video_format_from_c(format),
        Some(VideoFormat::Nv12 | VideoFormat::P010)
    )
}

/// One frame pair: the input planes and line sizes, the output line sizes.
#[derive(Clone, Debug)]
struct Frames {
    format: c_int,
    width: u32,
    height: u32,
    in_ls: [u32; 3],
    out_ls: [u32; 3],
    input: Vec<Vec<u8>>,
}

/// Frames of `format` with each plane's line sizes `pad_in` and `pad_out`
/// past its row, and `seed` for the input bytes.
fn frames(
    format: c_int,
    width: u32,
    height: u32,
    pad_in: [u32; 3],
    pad_out: [u32; 3],
    packed: bool,
    seed: u64,
) -> Frames {
    let ps = planes(format, width, height);
    let mut in_ls = [0; 3];
    let mut out_ls = [0; 3];
    for (c, &(row, _)) in ps.iter().enumerate() {
        in_ls[c] = row + pad_in[c];
        out_ls[c] = row + pad_out[c];
    }
    let mut state = seed | 1;
    let mut bytes = |len: usize| -> Vec<u8> {
        (0..len)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state as u8
            })
            .collect()
    };
    let mut input: Vec<Vec<u8>> = vec![Vec::new(); 3];
    let packed = packed && packable(format);
    for (c, &(_, rows)) in ps.iter().enumerate() {
        input[c] = bytes(in_ls[c] as usize * rows as usize + SLACK);
    }
    if packed {
        // plane 1 is laid out at plane 0's stride, after plane 0's rows
        let (_, uv_rows) = ps[1];
        // (the UV row is plane 0's row for both formats, so it fits)
        input[0] = bytes(in_ls[0] as usize * (height + uv_rows) as usize + SLACK);
        input[1] = Vec::new();
        in_ls[1] = 0;
    }
    Frames {
        format,
        width,
        height,
        in_ls,
        out_ls,
        input,
    }
}

fn info(format: c_int, width: u32, height: u32) -> video_output_info {
    video_output_info {
        name: ptr::null(),
        format,
        fps_num: 30,
        fps_den: 1,
        width,
        height,
        cache_size: 0,
        colorspace: 0,
        range: 0,
    }
}

fn outputs(f: &Frames, planes: usize) -> Vec<Vec<u8>> {
    (0..3)
        .map(|c| {
            if c < planes {
                vec![FILL; f.out_ls[c] as usize * f.height as usize + SLACK]
            } else {
                Vec::new()
            }
        })
        .collect()
}

fn data_of(f: &Frames) -> video_data {
    let mut d = video_data {
        data: [ptr::null_mut(); MAX_AV_PLANES],
        linesize: [0; MAX_AV_PLANES],
        timestamp: 0,
    };
    for c in 0..3 {
        if !f.input[c].is_empty() {
            d.data[c] = f.input[c].as_ptr().cast_mut();
        }
        d.linesize[c] = f.in_ls[c];
    }
    d
}

fn frame_of(out: &mut [Vec<u8>], ls: [u32; 3]) -> video_frame {
    let mut v = video_frame {
        data: [ptr::null_mut(); MAX_AV_PLANES],
        linesize: [0; MAX_AV_PLANES],
    };
    for (c, plane) in out.iter_mut().enumerate() {
        if !plane.is_empty() {
            v.data[c] = plane.as_mut_ptr();
        }
        v.linesize[c] = ls[c];
    }
    v
}

type Copy = unsafe extern "C" fn(*mut video_frame, *const video_data, *const video_output_info);
type OracleCopy = unsafe extern "C" fn(*mut c_void, *const c_void, *const c_void);

/// Runs the C and the shim on copies of `f` and compares every output byte.
fn check(f: &Frames, planes_out: usize, shim: Copy, c: OracleCopy, what: &str) {
    let data = data_of(f);
    let info = info(f.format, f.width, f.height);
    let mut want = outputs(f, planes_out);
    let mut got = want.clone();
    let mut want_frame = frame_of(&mut want, f.out_ls);
    let mut got_frame = frame_of(&mut got, f.out_ls);
    // SAFETY: the planes hold what the plan reads and writes (frames()).
    unsafe {
        c(
            (&raw mut want_frame).cast(),
            (&raw const data).cast(),
            (&raw const info).cast(),
        );
        shim(&raw mut got_frame, &raw const data, &raw const info);
    }
    assert_eq!(got, want, "{what}: {f:?}");
}

fn check_gpu(f: &Frames) {
    let n = planes(f.format, f.width, f.height).len();
    check(
        f,
        n,
        obs_video_set_gpu_converted_data,
        oracle::oracle_obs_video_set_gpu_converted_data,
        "gpu",
    );
}

fn rgbx(width: u32, height: u32, pad_in: u32, pad_out: u32, seed: u64) -> Frames {
    let row = width * 4;
    let mut f = frames(-1, width, height, [0; 3], [0; 3], false, seed);
    f.in_ls = [row + pad_in, 0, 0];
    f.out_ls = [row + pad_out, 0, 0];
    let mut state = seed | 1;
    f.input[0] = (0..f.in_ls[0] as usize * height as usize + SLACK)
        .map(|_| {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (state >> 56) as u8
        })
        .collect();
    f
}

fn check_rgbx(f: &Frames) {
    check(
        f,
        1,
        obs_video_copy_rgbx_frame,
        oracle::oracle_obs_video_copy_rgbx_frame,
        "rgbx",
    );
}

/// Every C format value worth feeding: the enum, one past it, and outside.
fn formats() -> Vec<c_int> {
    let mut v: Vec<c_int> = (0..=VideoFormat::R10l as c_int + 1).collect();
    v.extend([-1, c_int::MIN, c_int::MAX]);
    v
}

const SIZES: [u32; 6] = [0, 1, 2, 3, 8, 17];

#[test]
fn every_format_and_layout_matches_c() {
    let mut seed = 1;
    for format in formats() {
        for width in SIZES {
            for height in SIZES {
                for (pad_in, pad_out) in [
                    ([0; 3], [0; 3]),
                    ([5, 3, 1], [0; 3]),
                    ([0; 3], [4, 2, 6]),
                    ([7, 0, 3], [1, 9, 0]),
                ] {
                    for packed in [false, true] {
                        seed += 1;
                        check_gpu(&frames(
                            format, width, height, pad_in, pad_out, packed, seed,
                        ));
                    }
                }
            }
        }
    }
}

#[test]
fn the_test_sizes_match_c() {
    // test_video_frame_copy's 64x32 and 68x34, tight and padded
    for format in formats() {
        for (width, height) in [(64, 32), (68, 34)] {
            for (pad_in, packed) in [([0; 3], false), ([64; 3], false), ([64; 3], true)] {
                check_gpu(&frames(format, width, height, pad_in, [0; 3], packed, 7));
            }
        }
    }
}

#[test]
fn rgbx_matches_c() {
    let mut seed = 1;
    for width in SIZES {
        for height in SIZES {
            for (pad_in, pad_out) in [(0, 0), (4, 0), (0, 12), (3, 5)] {
                seed += 1;
                check_rgbx(&rgbx(width, height, pad_in, pad_out, seed));
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(512)))]

    #[test]
    fn random_frames_match_c(
        format in prop::sample::select(formats()),
        width in 0..48_u32,
        height in 0..32_u32,
        pad_in in prop::array::uniform3(prop_oneof![Just(0_u32), 0..40_u32]),
        pad_out in prop::array::uniform3(prop_oneof![Just(0_u32), 0..40_u32]),
        packed in any::<bool>(),
        seed in any::<u64>(),
    ) {
        check_gpu(&frames(format, width, height, pad_in, pad_out, packed, seed));
    }

    #[test]
    fn random_rgbx_frames_match_c(
        width in 0..48_u32,
        height in 0..32_u32,
        pad_in in prop_oneof![Just(0_u32), 0..40_u32],
        pad_out in prop_oneof![Just(0_u32), 0..40_u32],
        seed in any::<u64>(),
    ) {
        check_rgbx(&rgbx(width, height, pad_in, pad_out, seed));
    }
}
