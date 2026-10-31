//! C ABI for `libobs/obs-encoder-gpu-rescale.c.h`, the GPU rescale mix
//! decisions of `obs-encoder.c`, declared in
//! `libobs/obs-encoder-gpu-rescale.h` without `EXPORT`.
//!
//! `obs-encoder.c.h` (still C) reads the encoder's rescale fields and each
//! mix's view, scale type and `video_output_get_info()`, passes them here
//! through [`obs_encoder_gpu_rescale_request`] and
//! [`obs_encoder_gpu_rescale_output`] (Tier 3 checks their layout against
//! the C), and does every store: `encoder_refs`, `original_video`, the mix
//! creation, the mixes array under `mixes_mutex`, `obs_encoder_set_video()`.
//! So no libobs struct is mirrored, and these shims take no lock and call
//! nothing in libobs. All stay hidden (`rust-hidden.txt`).
//!
//! No intentional differences. A null pointer is not checked: libobs passes
//! locals.

use std::os::raw::{c_int, c_long};

use crate::encoder_gpu_rescale::{self as core, Output, Release, Request};

/// `struct obs_encoder_gpu_rescale_request` (`obs-encoder-gpu-rescale.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(non_camel_case_types)]
pub struct obs_encoder_gpu_rescale_request {
    /// An `enum obs_scale_type` value.
    pub scale_type: c_int,
    pub scaled_width: u32,
    pub scaled_height: u32,
    /// An `enum video_format` value.
    pub format: c_int,
    /// An `enum video_colorspace` value.
    pub colorspace: c_int,
    /// An `enum video_range_type` value.
    pub range: c_int,
}

/// `struct obs_encoder_gpu_rescale_output` (`obs-encoder-gpu-rescale.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(non_camel_case_types)]
pub struct obs_encoder_gpu_rescale_output {
    pub width: u32,
    pub height: u32,
    /// An `enum video_format` value.
    pub format: c_int,
    /// An `enum video_colorspace` value.
    pub colorspace: c_int,
    /// An `enum video_range_type` value.
    pub range: c_int,
}

/// `enum obs_encoder_gpu_rescale_release`, as the C's enum is returned (an
/// `int` or `unsigned int`; every value fits both).
#[allow(non_camel_case_types)]
pub type obs_encoder_gpu_rescale_release = u32;

/// `OBS_ENCODER_GPU_RESCALE_RELEASE_SKIP`.
pub const OBS_ENCODER_GPU_RESCALE_RELEASE_SKIP: obs_encoder_gpu_rescale_release = 0;
/// `OBS_ENCODER_GPU_RESCALE_RELEASE_STOP`.
pub const OBS_ENCODER_GPU_RESCALE_RELEASE_STOP: obs_encoder_gpu_rescale_release = 1;
/// `OBS_ENCODER_GPU_RESCALE_RELEASE_DROP`.
pub const OBS_ENCODER_GPU_RESCALE_RELEASE_DROP: obs_encoder_gpu_rescale_release = 2;

fn request(r: obs_encoder_gpu_rescale_request) -> Request {
    Request {
        scale_type: r.scale_type,
        scaled_width: r.scaled_width,
        scaled_height: r.scaled_height,
        format: r.format,
        colorspace: r.colorspace,
        range: r.range,
    }
}

fn output(o: obs_encoder_gpu_rescale_output) -> Output {
    Output {
        width: o.width,
        height: o.height,
        format: o.format,
        colorspace: o.colorspace,
        range: o.range,
    }
}

/// `bool obs_encoder_gpu_rescale_wanted(bool has_media, const struct
/// obs_encoder_gpu_rescale_request *request)`.
///
/// # Safety
///
/// As the C: `request_ptr` points to a readable `struct
/// obs_encoder_gpu_rescale_request`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_gpu_rescale_wanted(
    has_media: bool,
    request_ptr: *const obs_encoder_gpu_rescale_request,
) -> bool {
    // SAFETY: a readable request (caller contract). Copied, not borrowed.
    let r = unsafe { request_ptr.read() };
    core::wanted(has_media, &request(r))
}

/// `struct obs_encoder_gpu_rescale_output obs_encoder_gpu_rescale_target(const
/// struct obs_encoder_gpu_rescale_request *request, const struct
/// obs_encoder_gpu_rescale_output *video)`.
///
/// # Safety
///
/// As the C: `request_ptr` and `video` point to readable structs.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_gpu_rescale_target(
    request_ptr: *const obs_encoder_gpu_rescale_request,
    video: *const obs_encoder_gpu_rescale_output,
) -> obs_encoder_gpu_rescale_output {
    // SAFETY: readable structs (caller contract). Copied, not borrowed.
    let (r, v) = unsafe { (request_ptr.read(), video.read()) };
    let t = core::target(&request(r), &output(v));
    obs_encoder_gpu_rescale_output {
        width: t.width,
        height: t.height,
        format: t.format,
        colorspace: t.colorspace,
        range: t.range,
    }
}

/// `bool obs_encoder_gpu_rescale_matches(bool same_view, int mix_scale_type,
/// const struct obs_encoder_gpu_rescale_output *mix, int scale_type, const
/// struct obs_encoder_gpu_rescale_output *target)`.
///
/// # Safety
///
/// As the C: `mix` and `target` point to readable structs.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_gpu_rescale_matches(
    same_view: bool,
    mix_scale_type: c_int,
    mix: *const obs_encoder_gpu_rescale_output,
    scale_type: c_int,
    target: *const obs_encoder_gpu_rescale_output,
) -> bool {
    // SAFETY: readable structs (caller contract). Copied, not borrowed.
    let (m, t) = unsafe { (mix.read(), target.read()) };
    core::matches(
        same_view,
        mix_scale_type,
        &output(m),
        scale_type,
        &output(t),
    )
}

/// `enum obs_encoder_gpu_rescale_release
/// obs_encoder_gpu_rescale_release_step(bool is_media, bool encoder_only)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_gpu_rescale_release_step(
    is_media: bool,
    encoder_only: bool,
) -> obs_encoder_gpu_rescale_release {
    match core::release_step(is_media, encoder_only) {
        Release::Skip => OBS_ENCODER_GPU_RESCALE_RELEASE_SKIP,
        Release::Stop => OBS_ENCODER_GPU_RESCALE_RELEASE_STOP,
        Release::Drop => OBS_ENCODER_GPU_RESCALE_RELEASE_DROP,
    }
}

/// `long obs_encoder_gpu_rescale_release_refs(long encoder_refs, bool
/// *last)`.
///
/// `long` is 64-bit on LP64 and 32-bit on Windows; the core works in `i64`.
/// The one count that does not fit back, `LONG_MIN` on Windows (undefined in
/// the C), gives `LONG_MAX`, as a two's complement wrap would.
///
/// # Safety
///
/// As the C: `last` points to a writable `bool`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_gpu_rescale_release_refs(
    encoder_refs: c_long,
    last: *mut bool,
) -> c_long {
    #[cfg_attr(
        not(windows),
        expect(
            clippy::useless_conversion,
            reason = "c_long is i64 on LP64; i32 on Windows needs the widening"
        )
    )]
    let refs = i64::from(encoder_refs);
    let (after, is_last) = core::release_refs(refs);
    // SAFETY: a writable bool (caller contract).
    unsafe { last.write(is_last) };
    c_long::try_from(after).unwrap_or(c_long::MAX)
}
