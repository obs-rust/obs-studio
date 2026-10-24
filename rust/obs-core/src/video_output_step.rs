//! Safe core of `libobs/obs-video-output-step.c.h`: the per-frame video
//! output step decisions of `obs-video.c`.
//!
//! `obs-video.c` keeps every `gs_*` call in its order and under its
//! conditions, the deques, `os_sem_post()`, the texture-frame host calls,
//! the shared-planes and GPU-encode-surface host queries, the maps of
//! `download_frame()`, the mix list and its mutex, and every atomic load. It
//! reads the values, passes them here, and does what the answer says. These
//! keep no state, take no lock and touch no texture.
//!
//! - [`activation`] (`update_active_state()`): which frame data to clear
//!   when either output, raw output or GPU encoding becomes active, and the
//!   new `*_was_active` flags.
//! - [`stage`] (`stage_output_texture()`): whether this frame's copy
//!   surfaces are staged, from the output texture or the convert textures,
//!   and how many.
//! - [`queue_step`], [`copy_uv`] and [`queue_count`] (`queue_frame()`): a
//!   duplicate of the last queued frame or a new one, copied or swapped in;
//!   the separate UV copy; the count left.
//! - [`render_targets`] and [`render_steps`] (`render_video()`): whether
//!   the outputs are rendered, from which textures, then convert, encode and
//!   stage.
//! - [`ring`], [`download`] and [`output`] (`output_frame()`): the texture
//!   ring, whether to download, and whether the downloaded frame is output.
//!
//! The C asks some of these at more than one point (the host query of
//! `queue_frame()` comes between the Y copy and the UV copy; `render_video()`
//! reads `gpu_conversion` after the output texture is rendered and the host
//! is asked for the encode surfaces), so they are separate functions, called
//! where the C decided.
//!
//! No input comes from outside OBS (the flags and counts come from the
//! outputs and encoders libobs starts, the sizes from its own deques), but
//! nothing here can panic on any input.
//!
//! Intentional difference, unreachable: where the C's `int` arithmetic
//! overflows (undefined), the port wraps: [`queue_count`] of `INT_MIN`, and
//! [`ring`] of `INT_MIN` (prev) and `INT_MAX` (next). libobs passes counts
//! of 1 and more from `video_sleep()`, and `cur_texture` is only ever 0 or
//! set by the ring.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

use core::ffi::{c_int, c_long};

/// `NUM_TEXTURES`: the textures in flight between rendering and download.
pub const TEXTURES: c_int = 2;

/// `NUM_CHANNELS`: the planes of a converted frame.
pub const CHANNELS: usize = 3;

/// What `update_active_state()` looks at (`struct
/// obs_video_step_activation_input`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ActivationInput {
    /// `video->raw_was_active`.
    pub raw_was_active: bool,
    /// `video->gpu_was_active`.
    pub gpu_was_active: bool,
    /// `video->was_active`.
    pub was_active: bool,
    /// `os_atomic_load_long(&video->raw_active)`, loaded by the C.
    pub raw_active: c_long,
    /// `os_atomic_load_long(&video->gpu_encoder_active)`, loaded by the C
    /// after `raw_active`.
    pub gpu_encoder_active: c_long,
}

/// What `update_active_state()` does (`struct obs_video_step_activation`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Activation {
    /// `clear_base_frame_data()`, first.
    pub clear_base: bool,
    /// `clear_raw_frame_data()`, second.
    pub clear_raw: bool,
    /// `clear_gpu_frame_data()`, last.
    pub clear_gpu: bool,
    /// The new `video->raw_was_active`.
    pub raw_active: bool,
    /// The new `video->gpu_was_active`.
    pub gpu_active: bool,
    /// The new `video->was_active`.
    pub active: bool,
}

/// What `stage_output_texture()` stages after `unmap_last_surface()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Stage {
    /// Nothing: GPU conversion is on and the texture is not converted yet.
    /// `active_copy_surfaces` and `textures_copied` keep their values.
    Skip,
    /// No GPU conversion: copy surface 0 stages the output texture; the
    /// other channels are cleared.
    Output,
    /// Converted: copy surface `i` stages convert texture `i` for each `i`
    /// below `channels`; the channels from there to `NUM_CHANNELS` are
    /// cleared. `channels` is `channel_count` as the C passed it.
    Convert {
        /// The copy surfaces staged.
        channels: usize,
    },
}

impl Stage {
    /// Whether the copy surfaces are staged (`textures_copied` is set).
    #[must_use]
    pub const fn staged(self) -> bool {
        !matches!(self, Self::Skip)
    }

    /// Whether copy surface 0 stages the output texture.
    #[must_use]
    pub const fn from_output(self) -> bool {
        matches!(self, Self::Output)
    }

    /// The copy surfaces staged: 0 when skipped.
    #[must_use]
    pub const fn channels(self) -> usize {
        match self {
            Self::Skip => 0,
            Self::Output => 1,
            Self::Convert { channels } => channels,
        }
    }
}

/// What `queue_frame()` does with a frame for the GPU encoders (`enum
/// obs_video_step_queue_action`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum QueueStep {
    /// Count the last queued frame once more. The C stops instead when the
    /// queue holds no frame (`deque_data()` gives `NULL`).
    Duplicate,
    /// Take a free frame and copy the encode textures into it.
    Copy,
    /// Take a free frame and swap its textures with the encode textures.
    Swap,
}

/// What `render_video()` renders once the main texture is drawn (`struct
/// obs_video_step_render_targets`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct RenderTargets {
    /// Render the output texture and run the [`RenderSteps`].
    pub outputs: bool,
    /// From the encode textures, with the copy surfaces and channel count
    /// of `obs_host_gpu_encode_surfaces()`.
    pub encode: bool,
}

/// What `render_video()` does with the output texture, in this order
/// (`struct obs_video_step_render`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct RenderSteps {
    /// `render_convert_texture()`.
    pub convert: bool,
    /// `output_gpu_encoders()`.
    pub encode: bool,
    /// `stage_output_texture()`.
    pub stage: bool,
}

/// The texture ring of `output_frame()` (`struct obs_video_step_ring`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Ring {
    /// The texture before the current one, downloaded this frame.
    pub prev: c_int,
    /// The texture after the current one, the next frame's.
    pub next: c_int,
}

/// `update_active_state()`: a counter above 0 is active; each of either
/// output, raw output and GPU encoding that was not active and is now
/// clears its frame data.
#[must_use]
pub fn activation(input: &ActivationInput) -> Activation {
    let raw_active = input.raw_active > 0;
    let gpu_active = input.gpu_encoder_active > 0;
    let active = raw_active || gpu_active;

    Activation {
        clear_base: !input.was_active && active,
        clear_raw: !input.raw_was_active && raw_active,
        clear_gpu: !input.gpu_was_active && gpu_active,
        raw_active,
        gpu_active,
        active,
    }
}

/// `stage_output_texture()`: without GPU conversion the output texture into
/// copy surface 0; with it, once converted, `channel_count` copy surfaces
/// from the convert textures; else nothing.
#[must_use]
pub fn stage(gpu_conversion: bool, texture_converted: bool, channel_count: usize) -> Stage {
    if !gpu_conversion {
        Stage::Output
    } else if texture_converted {
        Stage::Convert {
            channels: channel_count,
        }
    } else {
        Stage::Skip
    }
}

/// `queue_frame()`: a duplicate when no frame is free (`avail_size` 0), or
/// when one is queued (`queue_size` not 0) and this frame repeats (`count`
/// above 1); else a copy for raw output or a repeat, or a swap.
#[must_use]
pub fn queue_step(
    avail_size: usize,
    queue_size: usize,
    count: c_int,
    raw_active: bool,
) -> QueueStep {
    let repeat = count > 1;
    if avail_size == 0 || (queue_size != 0 && repeat) {
        QueueStep::Duplicate
    } else if raw_active || repeat {
        // A repeat is copied only when nothing is queued yet (the very
        // first frame repeated); otherwise it is a duplicate above.
        QueueStep::Copy
    } else {
        QueueStep::Swap
    }
}

/// `queue_frame()`: after the Y copy, whether the UV texture is copied on
/// its own: unless `obs_host_nv12_planes_share_texture()`.
#[must_use]
pub fn copy_uv(planes_share_texture: bool) -> bool {
    !planes_share_texture
}

/// `queue_frame()`: the count left after a frame is queued or duplicated
/// (`--vframe_info->count`); `encode_gpu()` goes on while it is not 0.
#[must_use]
pub fn queue_count(count: c_int) -> c_int {
    count.wrapping_sub(1)
}

/// `render_video()`: the outputs are rendered when raw output or GPU
/// encoding is active, from the encode textures for GPU encoding.
#[must_use]
pub fn render_targets(raw_active: bool, gpu_active: bool) -> RenderTargets {
    let outputs = raw_active || gpu_active;
    RenderTargets {
        outputs,
        encode: outputs && gpu_active,
    }
}

/// `render_video()`: convert with GPU conversion, feed the GPU encoders for
/// GPU encoding, stage for raw output.
#[must_use]
pub fn render_steps(raw_active: bool, gpu_active: bool, gpu_conversion: bool) -> RenderSteps {
    RenderSteps {
        convert: gpu_conversion,
        encode: gpu_active,
        stage: raw_active,
    }
}

/// `output_frame()`: the textures before and after `cur_texture` in the
/// ring of [`TEXTURES`].
#[must_use]
pub fn ring(cur_texture: c_int) -> Ring {
    let prev = if cur_texture == 0 {
        TEXTURES.wrapping_sub(1)
    } else {
        cur_texture.wrapping_sub(1)
    };
    let next = match cur_texture.wrapping_add(1) {
        TEXTURES => 0,
        next => next,
    };
    Ring { prev, next }
}

/// `output_frame()`: the previous texture is downloaded for raw output.
#[must_use]
pub fn download(raw_active: bool) -> bool {
    raw_active
}

/// `output_frame()`: the downloaded frame is output for raw output, once
/// it is ready.
#[must_use]
pub fn output(raw_active: bool, frame_ready: bool) -> bool {
    raw_active && frame_ready
}
