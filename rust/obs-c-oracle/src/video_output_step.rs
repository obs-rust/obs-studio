//! `oracle_*` build of `libobs/obs-video-output-step.c.h`
//! (oracle/video_output_step.c): the per-frame video output step decisions,
//! renamed.
//!
//! The structs come back as the `Oracle*` mirrors below; the activation
//! input is an untyped pointer that callers lay out themselves.
//! [`oracle_video_output_step_layout`] says how the C sees all six structs.
//! The queue action comes back as the C enum's value.

use core::ffi::{c_int, c_void};

/// Index of each entry in `oracle_video_output_step_constant`, in the C's
/// order.
pub const CONSTANT_NAMES: [&str; 5] = [
    "OBS_VIDEO_STEP_TEXTURES",
    "OBS_VIDEO_STEP_CHANNELS",
    "OBS_VIDEO_STEP_QUEUE_DUPLICATE",
    "OBS_VIDEO_STEP_QUEUE_COPY",
    "OBS_VIDEO_STEP_QUEUE_SWAP",
];

/// Index of each entry in `oracle_video_output_step_layout`, in the C's
/// order.
pub const LAYOUT_NAMES: [&str; 35] = [
    "sizeof(struct obs_video_step_activation_input)",
    "alignof(struct obs_video_step_activation_input)",
    "offsetof(struct obs_video_step_activation_input, raw_was_active)",
    "offsetof(struct obs_video_step_activation_input, gpu_was_active)",
    "offsetof(struct obs_video_step_activation_input, was_active)",
    "offsetof(struct obs_video_step_activation_input, raw_active)",
    "offsetof(struct obs_video_step_activation_input, gpu_encoder_active)",
    "sizeof(struct obs_video_step_activation)",
    "alignof(struct obs_video_step_activation)",
    "offsetof(struct obs_video_step_activation, clear_base)",
    "offsetof(struct obs_video_step_activation, clear_raw)",
    "offsetof(struct obs_video_step_activation, clear_gpu)",
    "offsetof(struct obs_video_step_activation, raw_active)",
    "offsetof(struct obs_video_step_activation, gpu_active)",
    "offsetof(struct obs_video_step_activation, active)",
    "sizeof(struct obs_video_step_stage)",
    "alignof(struct obs_video_step_stage)",
    "offsetof(struct obs_video_step_stage, staged)",
    "offsetof(struct obs_video_step_stage, from_output)",
    "offsetof(struct obs_video_step_stage, channels)",
    "sizeof(struct obs_video_step_render_targets)",
    "alignof(struct obs_video_step_render_targets)",
    "offsetof(struct obs_video_step_render_targets, outputs)",
    "offsetof(struct obs_video_step_render_targets, encode)",
    "sizeof(struct obs_video_step_render)",
    "alignof(struct obs_video_step_render)",
    "offsetof(struct obs_video_step_render, convert)",
    "offsetof(struct obs_video_step_render, encode)",
    "offsetof(struct obs_video_step_render, stage)",
    "sizeof(struct obs_video_step_ring)",
    "alignof(struct obs_video_step_ring)",
    "offsetof(struct obs_video_step_ring, prev)",
    "offsetof(struct obs_video_step_ring, next)",
    "sizeof(enum obs_video_step_queue_action)",
    "sizeof(long)",
];

/// `struct obs_video_step_activation`, as the C returns it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct OracleActivation {
    pub clear_base: bool,
    pub clear_raw: bool,
    pub clear_gpu: bool,
    pub raw_active: bool,
    pub gpu_active: bool,
    pub active: bool,
}

/// `struct obs_video_step_stage`, as the C returns it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OracleStage {
    pub staged: bool,
    pub from_output: bool,
    pub channels: usize,
}

/// `struct obs_video_step_render_targets`, as the C returns it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OracleRenderTargets {
    pub outputs: bool,
    pub encode: bool,
}

/// `struct obs_video_step_render`, as the C returns it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OracleRender {
    pub convert: bool,
    pub encode: bool,
    pub stage: bool,
}

/// `struct obs_video_step_ring`, as the C returns it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OracleRing {
    pub prev: c_int,
    pub next: c_int,
}

unsafe extern "C" {
    /// `input` points to a `struct obs_video_step_activation_input`.
    pub fn oracle_obs_video_step_activation(input: *const c_void) -> OracleActivation;

    pub fn oracle_obs_video_step_stage(
        gpu_conversion: bool,
        texture_converted: bool,
        channel_count: usize,
    ) -> OracleStage;

    /// Returns an `enum obs_video_step_queue_action` value.
    pub fn oracle_obs_video_step_queue(
        avail_size: usize,
        queue_size: usize,
        count: c_int,
        raw_active: bool,
    ) -> u32;

    pub fn oracle_obs_video_step_queue_copy_uv(planes_share_texture: bool) -> bool;

    /// Undefined for `INT_MIN` (the C's `--count` overflows).
    pub fn oracle_obs_video_step_queue_count(count: c_int) -> c_int;

    pub fn oracle_obs_video_step_render_targets(
        raw_active: bool,
        gpu_active: bool,
    ) -> OracleRenderTargets;

    pub fn oracle_obs_video_step_render(
        raw_active: bool,
        gpu_active: bool,
        gpu_conversion: bool,
    ) -> OracleRender;

    /// Undefined for `INT_MIN` and `INT_MAX` (the C's `cur_texture - 1`
    /// and `cur_texture + 1` overflow).
    pub fn oracle_obs_video_step_ring(cur_texture: c_int) -> OracleRing;

    pub fn oracle_obs_video_step_download(raw_active: bool) -> bool;

    pub fn oracle_obs_video_step_output(raw_active: bool, frame_ready: bool) -> bool;

    /// The entry named `CONSTANT_NAMES[which]`; -1 out of range.
    pub fn oracle_video_output_step_constant(which: usize) -> i64;

    /// The entry named `LAYOUT_NAMES[which]`; `usize::MAX` out of range.
    pub fn oracle_video_output_step_layout(which: usize) -> usize;
}
