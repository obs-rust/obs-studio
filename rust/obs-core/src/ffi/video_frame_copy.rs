//! C ABI for `libobs/obs-video-frame-copy.c.h`, the output frame copy of
//! `obs-video.c`, declared in `libobs/obs-video-frame-copy.h` without
//! `EXPORT` (hidden, `rust-hidden.d`).
//!
//! `output_video_data()` (still C) locks the output frame, picks the copy
//! (GPU conversion or not) and unlocks the frame; these run the copy. Each
//! reads only the fields the C reads, one at a time, where and under the
//! condition the C reads them (the output info's format and size and the
//! line sizes the format's plan uses, [`line_sizes_read`], then each copy's
//! plane pointers in its turn; never `input->data[1]` of a packed frame),
//! makes the plan in Rust ([`gpu_copy_plan`], [`rgbx_copy_plan`]), and runs
//! every copy of it ([`video_frame_copy`]) over slices made from the planes
//! for exactly the bytes [`src_len`] and [`dst_len`] say that copy reads and
//! writes, one copy at a time: no two slices of an output frame are alive at
//! once. The source of a copy after plane 0 starts where copy 0's source
//! ended, in input plane 0, as the C's pointer does.
//!
//! The structs are `obs-media-io`'s mirrors (`video_frame`, `video_data`,
//! `video_output_info`), whose layouts its Tier 2 tests check against the
//! C. A null `output`, `input` or `info` is not checked: libobs passes its
//! locked frame, the mix's mapped frame and the output's info. A plane
//! pointer the plan reads that is null (the C would crash) ends the copy
//! there; libobs never passes one.
//!
//! [`gpu_copy_plan`]: crate::video_copy_plan::gpu_copy_plan
//! [`rgbx_copy_plan`]: crate::video_copy_plan::rgbx_copy_plan
//! [`video_frame_copy`]: crate::video_frame_copy
//! [`src_len`]: crate::video_frame_copy::src_len
//! [`dst_len`]: crate::video_frame_copy::dst_len
//! [`line_sizes_read`]: crate::video_frame_copy::line_sizes_read

use core::slice;

use obs_media_io::ffi::video_frame::video_frame;
use obs_media_io::ffi::video_io::{video_data, video_output_info};
use obs_media_io::video_io::{MAX_AV_PLANES, video_format_from_c};

use crate::video_copy_plan::{FrameCopy, LineSizes, gpu_copy_plan, rgbx_copy_plan};
use crate::video_frame_copy::{self as frame_copy, Source};

/// Runs `copy` from `src` into `dst`: slices of exactly the bytes it reads
/// and writes. `false` when a plane it touches is null, or its lengths
/// overflow.
///
/// # Safety
///
/// As the C's `memcpy()` calls: `src` is readable for [`src_len`] bytes and
/// `dst` writable for [`dst_len`] bytes, and the two do not overlap.
///
/// [`src_len`]: frame_copy::src_len
/// [`dst_len`]: frame_copy::dst_len
unsafe fn run(copy: &FrameCopy, src: *const u8, dst: *mut u8) -> bool {
    let (Some(src_len), Some(dst_len)) = (frame_copy::src_len(copy), frame_copy::dst_len(copy))
    else {
        return false;
    };
    let src: &[u8] = if src_len == 0 {
        &[]
    } else if src.is_null() {
        return false;
    } else {
        // SAFETY: readable for src_len bytes (caller contract).
        unsafe { slice::from_raw_parts(src, src_len) }
    };
    let dst: &mut [u8] = if dst_len == 0 {
        &mut []
    } else if dst.is_null() {
        return false;
    } else {
        // SAFETY: writable for dst_len bytes and apart from src (caller
        // contract); the only slice of this plane alive.
        unsafe { slice::from_raw_parts_mut(dst, dst_len) }
    };
    frame_copy::copy_plane(copy, src, dst).is_some()
}

/// `input->data[i]`, read alone; `None` past `MAX_AV_PLANES`.
///
/// # Safety
///
/// `input` points to a readable `struct video_data`.
unsafe fn input_plane(input: *const video_data, i: usize) -> Option<*const u8> {
    if i >= MAX_AV_PLANES {
        return None;
    }
    // SAFETY: a readable struct (caller contract) and i < MAX_AV_PLANES;
    // only this element is read.
    Some(unsafe { (&raw const (*input).data).cast::<*mut u8>().add(i).read() }.cast_const())
}

/// `output->data[i]`, read alone; `None` past `MAX_AV_PLANES`.
///
/// # Safety
///
/// `output` points to a readable `struct video_frame`.
unsafe fn output_plane(output: *const video_frame, i: usize) -> Option<*mut u8> {
    if i >= MAX_AV_PLANES {
        return None;
    }
    // SAFETY: as input_plane().
    Some(unsafe { (&raw const (*output).data).cast::<*mut u8>().add(i).read() })
}

/// `void obs_video_set_gpu_converted_data(struct video_frame *output, const
/// struct video_data *input, const struct video_output_info *info)`.
///
/// Reads what the C reads, in its order: `info->format`, `->width` and
/// `->height` and the leading line sizes of both frames the format's plan
/// uses ([`line_sizes_read`](frame_copy::line_sizes_read): three, two or
/// none); then,
/// copy by copy, `input->data[i]` (not for the copy after plane 0, which
/// continues from plane 0's pointer) and `output->data[i]`. A plan with no
/// copies reads no plane pointer.
///
/// # Safety
///
/// As the C: the three point to readable structs; each plane of `input`
/// the plan reads is readable for what it reads (plane 0 also for the copy
/// after it, packed), and each plane of `output` it writes is writable for
/// what it writes and apart from the input.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_video_set_gpu_converted_data(
    output: *mut video_frame,
    input: *const video_data,
    info: *const video_output_info,
) {
    // SAFETY: three readable structs (caller contract); each field is read
    // on its own, as the C reads it, and only the line sizes the format's
    // plan reads (the rest stay 0).
    let plan = unsafe {
        let format = video_format_from_c((*info).format);
        let (width, height) = ((*info).width, (*info).height);
        let n = frame_copy::line_sizes_read(format);
        let mut in_lines: LineSizes = [0; 3];
        let mut out_lines: LineSizes = [0; 3];
        let in_ls = (&raw const (*input).linesize).cast::<u32>();
        let out_ls = (&raw const (*output).linesize).cast::<u32>();
        for (i, (a, b)) in in_lines
            .iter_mut()
            .zip(out_lines.iter_mut())
            .enumerate()
            .take(n)
        {
            *a = in_ls.add(i).read();
            *b = out_ls.add(i).read();
        }
        gpu_copy_plan(format, width, height, &in_lines, &out_lines)
    };
    // input->data[0], as copy 0 read it: the copy after plane 0 continues
    // from it
    let mut plane0: Option<*const u8> = None;

    for step in frame_copy::steps(&plan) {
        let src = match step.source {
            // the C reads from NULL here; no plan has it
            None => continue,
            Some(Source::Plane(i)) => {
                // SAFETY: a readable struct (caller contract).
                let src = unsafe { input_plane(input, i) };
                if i == 0 {
                    plane0 = src;
                }
                src
            }
            // where copy 0's source ended, as the C's pointer moved on
            Some(Source::AfterPlane0(at)) => plane0.map(|p| p.wrapping_add(at)),
        };
        let Some(src) = src else {
            return;
        };
        // SAFETY: a readable struct (caller contract).
        let Some(dst) = (unsafe { output_plane(output, step.plane) }) else {
            return;
        };
        // SAFETY: the planes hold what the plan reads and writes (caller
        // contract).
        if !unsafe { run(&step.copy, src, dst) } {
            return;
        }
    }
}

/// `void obs_video_copy_rgbx_frame(struct video_frame *output, const struct
/// video_data *input, const struct video_output_info *info)`.
///
/// Reads what the C reads, in its order: `info->width`, `info->height`,
/// `input->linesize[0]` and `output->linesize[0]`, then `input->data[0]`
/// and `output->data[0]`.
///
/// # Safety
///
/// As the C: the three point to readable structs; plane 0 of `input` is
/// readable and plane 0 of `output` writable for what the copy reads and
/// writes, apart from each other.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_video_copy_rgbx_frame(
    output: *mut video_frame,
    input: *const video_data,
    info: *const video_output_info,
) {
    // SAFETY: three readable structs (caller contract); each field is read
    // on its own.
    let copy = unsafe {
        rgbx_copy_plan(
            (*info).width,
            (*info).height,
            (&raw const (*input).linesize).cast::<u32>().read(),
            (&raw const (*output).linesize).cast::<u32>().read(),
        )
    };
    // SAFETY: as above.
    let (Some(src), Some(dst)) = (unsafe { input_plane(input, 0) }, unsafe {
        output_plane(output, 0)
    }) else {
        return;
    };
    // SAFETY: plane 0 of each holds what the copy reads and writes (caller
    // contract).
    unsafe { run(&copy, src, dst) };
}
