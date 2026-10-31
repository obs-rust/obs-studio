//! `oracle_*` build of `libobs/obs-encoder-gpu-rescale.c.h`
//! (oracle/encoder_gpu_rescale.c): the GPU rescale mix decisions, renamed.
//!
//! The input structs are untyped pointers: callers lay them out themselves,
//! and [`oracle_encoder_gpu_rescale_layout`] says how the C sees them. The
//! struct returned by value comes back as [`Output`], whose layout the same
//! entries check.

use core::ffi::{c_int, c_long, c_void};

/// Index of each entry in `oracle_encoder_gpu_rescale_constant`, in the C's
/// order.
pub const CONSTANT_NAMES: [&str; 7] = [
    "OBS_ENCODER_GPU_RESCALE_SCALE_DISABLE",
    "OBS_ENCODER_GPU_RESCALE_FORMAT_NONE",
    "OBS_ENCODER_GPU_RESCALE_CS_DEFAULT",
    "OBS_ENCODER_GPU_RESCALE_RANGE_DEFAULT",
    "OBS_ENCODER_GPU_RESCALE_RELEASE_SKIP",
    "OBS_ENCODER_GPU_RESCALE_RELEASE_STOP",
    "OBS_ENCODER_GPU_RESCALE_RELEASE_DROP",
];

/// Index of each entry in `oracle_encoder_gpu_rescale_layout`, in the C's
/// order.
pub const LAYOUT_NAMES: [&str; 16] = [
    "sizeof(struct obs_encoder_gpu_rescale_request)",
    "alignof(struct obs_encoder_gpu_rescale_request)",
    "offsetof(struct obs_encoder_gpu_rescale_request, scale_type)",
    "offsetof(struct obs_encoder_gpu_rescale_request, scaled_width)",
    "offsetof(struct obs_encoder_gpu_rescale_request, scaled_height)",
    "offsetof(struct obs_encoder_gpu_rescale_request, format)",
    "offsetof(struct obs_encoder_gpu_rescale_request, colorspace)",
    "offsetof(struct obs_encoder_gpu_rescale_request, range)",
    "sizeof(struct obs_encoder_gpu_rescale_output)",
    "alignof(struct obs_encoder_gpu_rescale_output)",
    "offsetof(struct obs_encoder_gpu_rescale_output, width)",
    "offsetof(struct obs_encoder_gpu_rescale_output, height)",
    "offsetof(struct obs_encoder_gpu_rescale_output, format)",
    "offsetof(struct obs_encoder_gpu_rescale_output, colorspace)",
    "offsetof(struct obs_encoder_gpu_rescale_output, range)",
    "sizeof(enum obs_encoder_gpu_rescale_release)",
];

/// `struct obs_encoder_gpu_rescale_output`, as the C returns it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Output {
    pub width: u32,
    pub height: u32,
    pub format: c_int,
    pub colorspace: c_int,
    pub range: c_int,
}

unsafe extern "C" {
    /// `request` points to a `struct obs_encoder_gpu_rescale_request`.
    pub fn oracle_obs_encoder_gpu_rescale_wanted(has_media: bool, request: *const c_void) -> bool;

    /// `request` and `video` point to the request and output structs.
    pub fn oracle_obs_encoder_gpu_rescale_target(
        request: *const c_void,
        video: *const c_void,
    ) -> Output;

    /// `mix` and `target` point to `struct obs_encoder_gpu_rescale_output`s.
    pub fn oracle_obs_encoder_gpu_rescale_matches(
        same_view: bool,
        mix_scale_type: c_int,
        mix: *const c_void,
        scale_type: c_int,
        target: *const c_void,
    ) -> bool;

    /// Returns an `enum obs_encoder_gpu_rescale_release` value.
    pub fn oracle_obs_encoder_gpu_rescale_release_step(is_media: bool, encoder_only: bool) -> u32;

    /// `last` points to a writable `bool`.
    pub fn oracle_obs_encoder_gpu_rescale_release_refs(
        encoder_refs: c_long,
        last: *mut bool,
    ) -> c_long;

    /// The entry named `CONSTANT_NAMES[which]`; -1 out of range.
    pub fn oracle_encoder_gpu_rescale_constant(which: usize) -> i64;

    /// The entry named `LAYOUT_NAMES[which]`; `usize::MAX` out of range.
    pub fn oracle_encoder_gpu_rescale_layout(which: usize) -> usize;
}
