//! Safe core of the rest of `libobs/obs-source-deinterlace.c.h`: the frame
//! take, the texture update, the render and the mode and field-order
//! setters of a deinterlaced async source.
//!
//! - [`process_last_frame`]: `deinterlace_process_last_frame()`, under
//!   `async_mutex` (held by `async_tick()`): the current and previous frames
//!   back to the cache, then the take (`deinterlace_get_closest_frames()`,
//!   with `first_frame()` and `ready_deinterlace_frames()` folded in): the
//!   unbuffered hold, the walk over a copy of the queue's timestamps
//!   ([`crate::deinterlace_select::walk`]) and its drops, the unbuffered
//!   pair, the take of one frame or a pair, the half duration and the
//!   offset ([`crate::deinterlace_pacing`]);
//! - [`update_async_video`]: `deinterlace_update_async_video()`, once a
//!   frame: the slots read and the previous frame taken under `async_mutex`,
//!   then, outside it, the upload of that frame into the previous textures
//!   or the swap of the two texture sets
//!   ([`crate::deinterlace_select::update`]);
//! - [`render`]: `deinterlace_render()`, the draw with the source's effect
//!   ([`crate::deinterlace_draw`]: the skip, the linear rule, the technique,
//!   the multiplier and the second field);
//! - [`set_mode`]: `obs_source_set_deinterlace_mode()`, with
//!   `enable_deinterlacing()`, `disable_deinterlacing()` and `get_effect()`
//!   folded in ([`crate::deinterlace_draw::mode_change`]);
//! - [`set_field_order`]: `obs_source_set_deinterlace_field_order()`, top
//!   first only for the top field order.
//!
//! Each is generic over a [`Source`]: one method per field read or write,
//! lock call and call into the rest of libobs (the frame cache, the async
//! texture upload, the texture size, the effect load, the graphics
//! context and the `gs_*` calls), so a function makes the C's calls in the
//! C's order, under the C's locks and conditions.
//!
//! The C shims live in the `obs-source-seam` crate, which implements
//! [`Source`] over `libobs/obs-source-deinterlace-seam.h` and the libobs
//! calls; only libobs-rust links it, so this crate calls nothing in libobs.
//!
//! Where the C reads `cur_async_frame` or `prev_async_frame` again right
//! after it wrote it, under `async_mutex`, the value written is used: no
//! other thread writes the slots without the lock. Every other field is
//! read where the C reads it, as often as the C reads it. The timestamps
//! the walk reads are copied into a `Vec`, where the C uses a stack array
//! up to `OBS_SOURCE_FRAME_COPY_MAX_ASYNC_FRAMES` frames and `bmalloc()`
//! past it; the values and the reads are the same.
//!
//! No intentional differences from C.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::as_conversions,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable,
    clippy::todo
)]

use std::ffi::CStr;

use crate::deinterlace_draw::Effect;
use crate::draw_technique::Technique;

/// `MAX_AV_PLANES`: the texture slots of a source.
pub const MAX_AV_PLANES: usize = 8;

/// The previous texture planes [`set_mode`]'s disable destroys and clears.
pub const PREV_PLANES_DESTROYED: usize = 3;

/// `VIDEO_FORMAT_NONE`.
pub const VIDEO_FORMAT_NONE: i32 = 0;

/// `OBS_DEINTERLACE_FIELD_ORDER_TOP`.
pub const FIELD_ORDER_TOP: i32 = 0;

/// `GS_FLIP_V`, the sprite flag of a flipped source.
pub const FLIP_V: u32 = 1 << 1;

/// The name [`set_mode`] hands `obs_source_valid()`.
pub const SET_MODE_LOG_NAME: &CStr = c"obs_source_set_deinterlace_mode";

/// The name [`set_field_order`] hands `obs_source_valid()`.
pub const SET_FIELD_ORDER_LOG_NAME: &CStr = c"obs_source_set_deinterlace_field_order";

/// The effect parameters [`render`] looks up, in its order.
pub const PARAM_IMAGE: &CStr = c"image";
/// See [`PARAM_IMAGE`].
pub const PARAM_PREVIOUS_IMAGE: &CStr = c"previous_image";
/// See [`PARAM_IMAGE`].
pub const PARAM_MULTIPLIER: &CStr = c"multiplier";
/// See [`PARAM_IMAGE`].
pub const PARAM_FIELD_ORDER: &CStr = c"field_order";
/// See [`PARAM_IMAGE`].
pub const PARAM_FRAME2: &CStr = c"frame2";
/// See [`PARAM_IMAGE`].
pub const PARAM_DIMENSIONS: &CStr = c"dimensions";

/// `obs_source_draw_technique_name()`, a `static inline` of
/// `obs-source-draw-technique.h`, for the techniques the deinterlace draws
/// with.
#[expect(clippy::todo, reason = "RED: the port follows")]
pub const fn technique_name(technique: Technique) -> &'static CStr {
    let _ = technique;
    todo!()
}

/// The fields, locks and libobs calls of an async source that its
/// deinterlace uses: in libobs, the deinterlace seam
/// (`obs-source-deinterlace-seam.h`), `obs_source_deinterlace_fields()` and
/// the libobs functions, one call per method. Every method takes `&self`,
/// as the C writes through its `obs_source_t *`; none caches a field.
pub trait Source {
    /// A `struct obs_source_frame *` that is not NULL.
    type Frame: Copy + PartialEq;
    /// A `gs_texture_t *` that is not NULL.
    type Texture: Copy + PartialEq;
    /// A `gs_texrender_t *` that is not NULL.
    type Texrender: Copy + PartialEq;
    /// A `gs_effect_t *` that is not NULL.
    type Effect: Copy + PartialEq;
    /// A `gs_eparam_t *` that is not NULL.
    type Param: Copy + PartialEq;

    /// `obs_source_valid(source, func)`: false for no source (the C logs
    /// `func` at debug level).
    fn valid(source: Option<&Self>, func: &'static CStr) -> bool;

    /// `async_frames.num`.
    fn frames_num(&self) -> usize;
    /// `async_frames.array[idx]`, for `idx` below [`Source::frames_num`].
    fn frame(&self, idx: usize) -> Self::Frame;
    /// `da_erase(async_frames, 0)`.
    fn erase_front(&self);
    /// Locks `async_mutex`.
    fn lock(&self);
    /// Unlocks `async_mutex`.
    fn unlock(&self);

    /// `async_unbuffered`.
    fn async_unbuffered(&self) -> bool;
    /// `last_sys_timestamp`.
    fn last_sys_timestamp(&self) -> u64;
    /// `last_frame_ts`.
    fn last_frame_ts(&self) -> u64;
    /// Writes `last_frame_ts`.
    fn set_last_frame_ts(&self, value: u64);
    /// `deinterlace_offset`.
    fn offset(&self) -> u64;
    /// Writes `deinterlace_offset`.
    fn set_offset(&self, value: u64);
    /// `deinterlace_frame_ts`.
    fn frame_ts(&self) -> u64;
    /// Writes `deinterlace_frame_ts`.
    fn set_frame_ts(&self, value: u64);
    /// `deinterlace_half_duration`.
    fn half_duration(&self) -> u32;
    /// Writes `deinterlace_half_duration`.
    fn set_half_duration(&self, value: u32);

    /// `cur_async_frame`.
    fn cur_frame(&self) -> Option<Self::Frame>;
    /// Writes `cur_async_frame`.
    fn set_cur_frame(&self, value: Option<Self::Frame>);
    /// `prev_async_frame`.
    fn prev_frame(&self) -> Option<Self::Frame>;
    /// Writes `prev_async_frame`.
    fn set_prev_frame(&self, value: Option<Self::Frame>);

    /// `frame->timestamp`.
    fn frame_timestamp(&self, frame: Self::Frame) -> u64;
    /// `frame->prev_frame`.
    fn frame_prev_flag(&self, frame: Self::Frame) -> bool;
    /// Writes `frame->prev_frame`.
    fn set_frame_prev_flag(&self, frame: Self::Frame, value: bool);
    /// `os_atomic_inc_long(&frame->refs)`.
    fn frame_addref(&self, frame: Self::Frame);

    /// `async_format` (`enum video_format`).
    fn async_format(&self) -> i32;
    /// `async_trc` (`enum video_trc`).
    fn async_trc(&self) -> i32;
    /// `async_width`.
    fn async_width(&self) -> u32;
    /// `async_height`.
    fn async_height(&self) -> u32;
    /// `async_flip`.
    fn async_flip(&self) -> bool;
    /// `async_textures[idx]`, for `idx` below [`MAX_AV_PLANES`].
    fn texture(&self, idx: usize) -> Option<Self::Texture>;
    /// Writes `async_textures[idx]`.
    fn set_texture(&self, idx: usize, value: Option<Self::Texture>);
    /// `async_prev_textures[idx]`.
    fn prev_texture(&self, idx: usize) -> Option<Self::Texture>;
    /// Writes `async_prev_textures[idx]`.
    fn set_prev_texture(&self, idx: usize, value: Option<Self::Texture>);
    /// `async_texrender`.
    fn texrender(&self) -> Option<Self::Texrender>;
    /// Writes `async_texrender`.
    fn set_texrender(&self, value: Option<Self::Texrender>);
    /// `async_prev_texrender`.
    fn prev_texrender(&self) -> Option<Self::Texrender>;
    /// Writes `async_prev_texrender`.
    fn set_prev_texrender(&self, value: Option<Self::Texrender>);

    /// `deinterlace_rendered`.
    fn rendered(&self) -> bool;
    /// Writes `deinterlace_rendered`.
    fn set_rendered(&self, value: bool);
    /// `deinterlace_effect`.
    fn effect(&self) -> Option<Self::Effect>;
    /// Writes `deinterlace_effect`.
    fn set_effect(&self, value: Option<Self::Effect>);
    /// `deinterlace_mode` (`enum obs_deinterlace_mode`, as an `int`).
    fn mode(&self) -> i32;
    /// Writes `deinterlace_mode`.
    fn set_mode(&self, value: i32);
    /// `deinterlace_top_first`.
    fn top_first(&self) -> bool;
    /// Writes `deinterlace_top_first`.
    fn set_top_first(&self, value: bool);

    /// `convert_video_space(format, trc)`, the `obs-internal.h` inline
    /// (`enum gs_color_space`, as an `int`).
    fn convert_video_space(&self, format: i32, trc: i32) -> i32;
    /// `obs->video.video_frame_interval_ns`.
    fn frame_interval_ns(&self) -> u64;
    /// `obs_video_stats_frame_time(&obs->video.stats)`.
    fn video_time(&self) -> u64;
    /// `obs_load_effect(&obs->video.deinterlace_<effect>_effect,
    /// "deinterlace_<effect>.effect")`.
    fn load_effect(&self, effect: Effect) -> Option<Self::Effect>;

    /// `remove_async_frame(self, frame)`: the frame back to the cache.
    fn remove_async_frame(&self, frame: Self::Frame);
    /// `set_async_texture_size(self, frame)`.
    fn set_async_texture_size(&self, frame: Self::Frame) -> bool;
    /// `update_async_textures(self, frame, async_prev_textures,
    /// prev_texrender)`.
    fn update_prev_textures(
        &self,
        frame: Self::Frame,
        prev_texrender: Option<Self::Texrender>,
    ) -> bool;
    /// `obs_source_release_frame(self, frame)`.
    fn release_frame(&self, frame: Self::Frame);
    /// `set_deinterlace_texture_size(self)`.
    fn set_deinterlace_texture_size(&self);

    /// `obs_enter_graphics()`.
    fn enter_graphics(&self);
    /// `obs_leave_graphics()`.
    fn leave_graphics(&self);
    /// `obs_get_video_sdr_white_level()`.
    fn sdr_white_level(&self) -> f32;

    /// `gs_effect_get_param_by_name(effect, name)`.
    fn param_by_name(
        &self,
        effect: Option<Self::Effect>,
        name: &'static CStr,
    ) -> Option<Self::Param>;
    /// `gs_texrender_get_texture(texrender)`.
    fn texrender_texture(&self, texrender: Self::Texrender) -> Option<Self::Texture>;
    /// `gs_get_linear_srgb()`.
    fn linear_srgb(&self) -> bool;
    /// `gs_get_color_space()` (`enum gs_color_space`, as an `int`).
    fn color_space(&self) -> i32;
    /// `gs_framebuffer_srgb_enabled()`.
    fn framebuffer_srgb_enabled(&self) -> bool;
    /// `gs_enable_framebuffer_srgb(enable)`.
    fn enable_framebuffer_srgb(&self, enable: bool);
    /// `gs_effect_set_texture_srgb(param, texture)`.
    fn set_param_texture_srgb(&self, param: Option<Self::Param>, texture: Option<Self::Texture>);
    /// `gs_effect_set_texture(param, texture)`.
    fn set_param_texture(&self, param: Option<Self::Param>, texture: Option<Self::Texture>);
    /// `gs_effect_set_float(param, value)`.
    fn set_param_float(&self, param: Option<Self::Param>, value: f32);
    /// `gs_effect_set_int(param, value)`.
    fn set_param_int(&self, param: Option<Self::Param>, value: i32);
    /// `gs_effect_set_vec2(param, &value)`.
    fn set_param_vec2(&self, param: Option<Self::Param>, value: [f32; 2]);
    /// `gs_effect_set_bool(param, value)`.
    fn set_param_bool(&self, param: Option<Self::Param>, value: bool);
    /// `gs_effect_loop(effect, technique)`.
    fn effect_loop(&self, effect: Option<Self::Effect>, technique: &'static CStr) -> bool;
    /// `gs_draw_sprite(NULL, flip, width, height)`.
    fn draw_sprite(&self, flip: u32, width: u32, height: u32);
    /// `gs_texture_destroy(texture)`.
    fn texture_destroy(&self, texture: Option<Self::Texture>);
    /// `gs_texrender_destroy(texrender)`.
    fn texrender_destroy(&self, texrender: Option<Self::Texrender>);
}

#[expect(clippy::todo, reason = "RED: the port follows")]
/// `deinterlace_process_last_frame()`: under `async_mutex`, held by the
/// caller (`async_tick()`).
pub fn process_last_frame<S: Source>(s: &S, sys_time: u64) {
    let _ = (s, sys_time);
    todo!()
}

#[expect(clippy::todo, reason = "RED: the port follows")]
/// `deinterlace_update_async_video()`, on the graphics thread.
pub fn update_async_video<S: Source>(s: &S) {
    let _ = s;
    todo!()
}

#[expect(clippy::todo, reason = "RED: the port follows")]
/// `deinterlace_render()`, in the graphics context.
pub fn render<S: Source>(s: &S) {
    let _ = s;
    todo!()
}

#[expect(clippy::todo, reason = "RED: the port follows")]
/// `obs_source_set_deinterlace_mode()`. A mode outside the enum disables
/// (#2090), as [`crate::deinterlace_draw::mode_change`] decides.
pub fn set_mode<S: Source>(source: Option<&S>, mode: i32) {
    let _ = (source, mode);
    todo!()
}

#[expect(clippy::todo, reason = "RED: the port follows")]
/// `obs_source_set_deinterlace_field_order()`: top first only for
/// [`FIELD_ORDER_TOP`].
pub fn set_field_order<S: Source>(source: Option<&S>, field_order: i32) {
    let _ = (source, field_order);
    todo!()
}
