//! obs-core: the output frame copy (`libobs/obs-video-frame-copy.c.h`). The
//! core video thread runs one copy per output frame of each mix with raw
//! output (`output_video_data()`): a GPU-converted frame's planes
//! (`set_gpu_converted_data()`) or an RGBA one (`copy_rgbx_frame()`), at
//! 60 fps. Each side copies a 1280x720 frame: I420 in whole planes, NV12
//! with padded staging lines and its UV plane after plane 0 (a row at a
//! time), and BGRA with padded lines. The result is a checksum of the
//! output planes, which both must match.

use super::pair;

use core::ffi::{c_int, c_void};
use core::ptr;

use criterion::Criterion;
use obs_c_oracle::video_frame_copy::{
    oracle_obs_video_copy_rgbx_frame, oracle_obs_video_set_gpu_converted_data,
};
use obs_core::ffi::video_frame_copy::{
    obs_video_copy_rgbx_frame, obs_video_set_gpu_converted_data,
};
use obs_media_io::ffi::video_frame::video_frame;
use obs_media_io::ffi::video_io::{video_data, video_output_info};
use std::hint::black_box;

const W: u32 = 1280;
const H: u32 = 720;

/// `VIDEO_FORMAT_I420`, `VIDEO_FORMAT_NV12`, `VIDEO_FORMAT_BGRA`.
const I420: c_int = 1;
const NV12: c_int = 2;
const BGRA: c_int = 7;

/// One frame pair: the input planes and line sizes, the output line sizes.
struct Fixture {
    format: c_int,
    input: Vec<Vec<u8>>,
    in_ls: [u32; 3],
    out_ls: [u32; 3],
    out_rows: [u32; 3],
}

fn bytes(len: usize, seed: u8) -> Vec<u8> {
    (0..len)
        .map(|i| (i as u8).wrapping_mul(31).wrapping_add(seed))
        .collect()
}

fn fixtures() -> [Fixture; 3] {
    [
        Fixture {
            format: I420,
            input: vec![
                bytes((W * H) as usize, 1),
                bytes((W / 2 * H / 2) as usize, 2),
                bytes((W / 2 * H / 2) as usize, 3),
            ],
            in_ls: [W, W / 2, W / 2],
            out_ls: [W, W / 2, W / 2],
            out_rows: [H, H / 2, H / 2],
        },
        Fixture {
            format: NV12,
            // Y then UV at a padded stride, in one surface
            input: vec![bytes(((W + 256) * (H + H / 2)) as usize, 4), Vec::new(), Vec::new()],
            in_ls: [W + 256, 0, 0],
            out_ls: [W, W, 0],
            out_rows: [H, H / 2, 0],
        },
        Fixture {
            format: BGRA,
            input: vec![bytes(((W * 4 + 64) * H) as usize, 5), Vec::new(), Vec::new()],
            in_ls: [W * 4 + 64, 0, 0],
            out_ls: [W * 4, 0, 0],
            out_rows: [H, 0, 0],
        },
    ]
}

type Copy = unsafe extern "C" fn(*mut video_frame, *const video_data, *const video_output_info);

/// Copies `f` into `out` with `copy` and checksums the output.
fn run(f: &Fixture, out: &mut [Vec<u8>; 3], copy: impl Fn(*mut video_frame, *const video_data, *const video_output_info)) -> u64 {
    let mut data = video_data {
        data: [ptr::null_mut(); 8],
        linesize: [0; 8],
        timestamp: 0,
    };
    let mut frame = video_frame {
        data: [ptr::null_mut(); 8],
        linesize: [0; 8],
    };
    for (c, plane) in out.iter_mut().enumerate() {
        if !f.input[c].is_empty() {
            data.data[c] = f.input[c].as_ptr().cast_mut();
        }
        data.linesize[c] = f.in_ls[c];
        if !plane.is_empty() {
            frame.data[c] = plane.as_mut_ptr();
        }
        frame.linesize[c] = f.out_ls[c];
    }
    let info = video_output_info {
        name: ptr::null(),
        format: f.format,
        fps_num: 60,
        fps_den: 1,
        width: W,
        height: H,
        cache_size: 0,
        colorspace: 0,
        range: 0,
    };
    copy(black_box(&raw mut frame), &raw const data, &raw const info);
    out.iter().fold(0_u64, |acc, plane| {
        plane
            .iter()
            .step_by(61)
            .fold(acc, |acc, &b| acc.wrapping_mul(31).wrapping_add(u64::from(b)))
    })
}

fn outputs(f: &Fixture) -> [Vec<u8>; 3] {
    std::array::from_fn(|c| vec![0; (f.out_ls[c] * f.out_rows[c]) as usize])
}

fn shim(rgbx: bool) -> Copy {
    if rgbx {
        obs_video_copy_rgbx_frame
    } else {
        obs_video_set_gpu_converted_data
    }
}

fn oracle(rgbx: bool) -> unsafe extern "C" fn(*mut c_void, *const c_void, *const c_void) {
    if rgbx {
        oracle_obs_video_copy_rgbx_frame
    } else {
        oracle_obs_video_set_gpu_converted_data
    }
}

pub fn benchmarks(criterion: &mut Criterion) {
    let fx = fixtures();
    for (f, name) in fx.iter().zip([
        "video_frame_copy/i420_whole/720p",
        "video_frame_copy/nv12_packed_rows/720p",
        "video_frame_copy/bgra_rows/720p",
    ]) {
        let rgbx = f.format == BGRA;
        let mut a = outputs(f);
        let mut b = outputs(f);
        let rust = shim(rgbx);
        let c = oracle(rgbx);
        pair(
            criterion,
            name,
            // SAFETY: the planes hold what the plan reads and writes.
            || run(f, &mut a, |o, i, n| unsafe { rust(o, i, n) }),
            // SAFETY: as above; the oracle reads them as the C structs.
            || run(f, &mut b, |o, i, n| unsafe { c(o.cast(), i.cast(), n.cast()) }),
        );
    }
}
