//! C ABI for `libobs/obs-video-output-step.c.h`, the per-frame video output
//! step decisions of `obs-video.c`, declared in
//! `libobs/obs-video-output-step.h` without `EXPORT`.
//!
//! These only decide. `obs-video.c.h` (still C) makes every `gs_*` call in
//! its order and under its conditions, works the deques, posts the
//! semaphore, calls the texture-frame host functions and asks the
//! shared-planes and GPU-encode-surface host queries (it passes their
//! answers here), maps the staging surfaces in `download_frame()`, walks
//! the mix list under its mutex and loads the atomics (it passes the loaded
//! values). The structs cross as `#[repr(C)]` mirrors whose layout Tier 3
//! checks against the C; the queue action returns as the C enum's value. No
//! texture, surface, deque or mix pointer comes here. All stay hidden
//! (`rust-hidden.txt`).
//!
//! The one intentional difference is the core's (the C's undefined `int`
//! overflows wrap; unreachable). A null `input` is not checked: libobs
//! passes a local.

use core::ffi::{c_int, c_long};

use crate::video_output_step::{
    self as step, Activation, ActivationInput, QueueStep, RenderSteps, RenderTargets, Ring, Stage,
};

/// `struct obs_video_step_activation_input` (`obs-video-output-step.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(non_camel_case_types)]
pub struct obs_video_step_activation_input {
    pub raw_was_active: bool,
    pub gpu_was_active: bool,
    pub was_active: bool,
    pub raw_active: c_long,
    pub gpu_encoder_active: c_long,
}

/// `struct obs_video_step_activation` (`obs-video-output-step.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(non_camel_case_types)]
pub struct obs_video_step_activation {
    pub clear_base: bool,
    pub clear_raw: bool,
    pub clear_gpu: bool,
    pub raw_active: bool,
    pub gpu_active: bool,
    pub active: bool,
}

/// `struct obs_video_step_stage` (`obs-video-output-step.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(non_camel_case_types)]
pub struct obs_video_step_stage {
    pub staged: bool,
    pub from_output: bool,
    pub channels: usize,
}

/// `struct obs_video_step_render_targets` (`obs-video-output-step.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(non_camel_case_types)]
pub struct obs_video_step_render_targets {
    pub outputs: bool,
    pub encode: bool,
}

/// `struct obs_video_step_render` (`obs-video-output-step.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(non_camel_case_types)]
pub struct obs_video_step_render {
    pub convert: bool,
    pub encode: bool,
    pub stage: bool,
}

/// `struct obs_video_step_ring` (`obs-video-output-step.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(non_camel_case_types)]
pub struct obs_video_step_ring {
    pub prev: c_int,
    pub next: c_int,
}

/// `enum obs_video_step_queue_action`, as the C returns it (an `unsigned
/// int`; every value fits an `int` too).
#[allow(non_camel_case_types)]
pub type obs_video_step_queue_action = u32;

/// `OBS_VIDEO_STEP_QUEUE_DUPLICATE`.
pub const OBS_VIDEO_STEP_QUEUE_DUPLICATE: obs_video_step_queue_action = 0;
/// `OBS_VIDEO_STEP_QUEUE_COPY`.
pub const OBS_VIDEO_STEP_QUEUE_COPY: obs_video_step_queue_action = 1;
/// `OBS_VIDEO_STEP_QUEUE_SWAP`.
pub const OBS_VIDEO_STEP_QUEUE_SWAP: obs_video_step_queue_action = 2;

/// The C value of a [`QueueStep`].
#[must_use]
pub const fn queue_step_to_c(step: QueueStep) -> obs_video_step_queue_action {
    match step {
        QueueStep::Duplicate => OBS_VIDEO_STEP_QUEUE_DUPLICATE,
        QueueStep::Copy => OBS_VIDEO_STEP_QUEUE_COPY,
        QueueStep::Swap => OBS_VIDEO_STEP_QUEUE_SWAP,
    }
}

impl From<obs_video_step_activation_input> for ActivationInput {
    fn from(i: obs_video_step_activation_input) -> Self {
        Self {
            raw_was_active: i.raw_was_active,
            gpu_was_active: i.gpu_was_active,
            was_active: i.was_active,
            raw_active: i.raw_active,
            gpu_encoder_active: i.gpu_encoder_active,
        }
    }
}

impl From<Activation> for obs_video_step_activation {
    fn from(a: Activation) -> Self {
        Self {
            clear_base: a.clear_base,
            clear_raw: a.clear_raw,
            clear_gpu: a.clear_gpu,
            raw_active: a.raw_active,
            gpu_active: a.gpu_active,
            active: a.active,
        }
    }
}

impl From<Stage> for obs_video_step_stage {
    fn from(s: Stage) -> Self {
        Self {
            staged: s.staged(),
            from_output: s.from_output(),
            channels: s.channels(),
        }
    }
}

impl From<RenderTargets> for obs_video_step_render_targets {
    fn from(t: RenderTargets) -> Self {
        Self {
            outputs: t.outputs,
            encode: t.encode,
        }
    }
}

impl From<RenderSteps> for obs_video_step_render {
    fn from(r: RenderSteps) -> Self {
        Self {
            convert: r.convert,
            encode: r.encode,
            stage: r.stage,
        }
    }
}

impl From<Ring> for obs_video_step_ring {
    fn from(r: Ring) -> Self {
        Self {
            prev: r.prev,
            next: r.next,
        }
    }
}

/// `struct obs_video_step_activation obs_video_step_activation(const struct
/// obs_video_step_activation_input *input)`.
///
/// # Safety
///
/// As the C: `input` points to a readable `struct
/// obs_video_step_activation_input`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_video_step_activation(
    input: *const obs_video_step_activation_input,
) -> obs_video_step_activation {
    // SAFETY: a readable input (caller contract). Copied, not borrowed.
    let input = unsafe { input.read() };
    step::activation(&input.into()).into()
}

/// `struct obs_video_step_stage obs_video_step_stage(bool gpu_conversion,
/// bool texture_converted, size_t channel_count)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_video_step_stage(
    gpu_conversion: bool,
    texture_converted: bool,
    channel_count: usize,
) -> obs_video_step_stage {
    step::stage(gpu_conversion, texture_converted, channel_count).into()
}

/// `enum obs_video_step_queue_action obs_video_step_queue(size_t
/// avail_size, size_t queue_size, int count, bool raw_active)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_video_step_queue(
    avail_size: usize,
    queue_size: usize,
    count: c_int,
    raw_active: bool,
) -> obs_video_step_queue_action {
    queue_step_to_c(step::queue_step(avail_size, queue_size, count, raw_active))
}

/// `bool obs_video_step_queue_copy_uv(bool planes_share_texture)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_video_step_queue_copy_uv(planes_share_texture: bool) -> bool {
    step::copy_uv(planes_share_texture)
}

/// `int obs_video_step_queue_count(int count)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_video_step_queue_count(count: c_int) -> c_int {
    step::queue_count(count)
}

/// `struct obs_video_step_render_targets obs_video_step_render_targets(bool
/// raw_active, bool gpu_active)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_video_step_render_targets(
    raw_active: bool,
    gpu_active: bool,
) -> obs_video_step_render_targets {
    step::render_targets(raw_active, gpu_active).into()
}

/// `struct obs_video_step_render obs_video_step_render(bool raw_active, bool
/// gpu_active, bool gpu_conversion)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_video_step_render(
    raw_active: bool,
    gpu_active: bool,
    gpu_conversion: bool,
) -> obs_video_step_render {
    step::render_steps(raw_active, gpu_active, gpu_conversion).into()
}

/// `struct obs_video_step_ring obs_video_step_ring(int cur_texture)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_video_step_ring(cur_texture: c_int) -> obs_video_step_ring {
    step::ring(cur_texture).into()
}

/// `bool obs_video_step_download(bool raw_active)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_video_step_download(raw_active: bool) -> bool {
    step::download(raw_active)
}

/// `bool obs_video_step_output(bool raw_active, bool frame_ready)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_video_step_output(raw_active: bool, frame_ready: bool) -> bool {
    step::output(raw_active, frame_ready)
}
