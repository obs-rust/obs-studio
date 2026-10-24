//! Safe core of `libobs/obs-source-render-route.c.h`: the source and filter
//! render routing of `obs-source.c`.
//!
//! - [`filter_check`]: the target and parent check of a filter render step,
//!   no target first (#2177's guard in `obs_source_skip_video_filter()`);
//! - `source_render_video()`: [`video_reads_flags`] and [`video_none`] (a
//!   source that is neither video nor a filter), [`async_reads_flags`],
//!   [`async_reads_rendering_filter`] and [`async_update`] (the async
//!   texture update), [`reads_enabled`] and [`inactive`] (no data or
//!   disabled), and [`draw`] then [`draw_next`]: filters, main render,
//!   filter target, deinterlace or async video;
//! - `obs_source_main_render()`: [`main_custom_draw`],
//!   [`main_srgb_aware`], [`main_reads_filters`] and
//!   [`main_default_effect`];
//! - `obs_source_process_filter_begin_with_color_space()`:
//!   [`bypass_reads_space`] and [`bypass`] (`can_bypass()`), [`size_skip`],
//!   [`texrender_stale`] and [`filter_render`];
//! - `obs_source_process_filter_tech_end()`: [`tech_end_linear_srgb`],
//!   [`tech_name`] and [`tech_end`];
//! - `obs_source_skip_video_filter()`: [`skip`] then [`draw_next`].
//!
//! `obs-source.c.h` keeps every `gs_*` call and texrender, the source refs,
//! the render calls, `get_base_width()` and `get_base_height()`, the debug
//! markers and every field read, in their order and under their conditions.
//! A `reads_` function says whether C reads a value the old code read only on
//! one side of a short circuit; a draw route that needs a value C has not read
//! yet returns a `Read*` step, and C reads that one value and passes it to
//! [`draw_next`]. Nothing here keeps state, takes a lock or calls libobs.
//!
//! Types, the direct render choice, colour spaces and formats are the C's
//! `int`: a value outside an enum takes the path it takes in the C. Output
//! flags are the `uint32_t` `OBS_SOURCE_*` bits. Characterized, not endorsed
//! (`test_source_render_route_steps`), and kept: a filter type draws whatever
//! its flags; `ASYNC` without `VIDEO` is no video; a size of 2^31 or more is
//! negative in the C's `int` and does not skip.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::as_conversions,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable
)]

/// `OBS_SOURCE_TYPE_INPUT` (`enum obs_source_type`, `obs-source.h`).
pub const TYPE_INPUT: i32 = 0;
/// `OBS_SOURCE_TYPE_FILTER`.
pub const TYPE_FILTER: i32 = 1;
/// `OBS_SOURCE_TYPE_TRANSITION`.
pub const TYPE_TRANSITION: i32 = 2;
/// `OBS_SOURCE_TYPE_SCENE`.
pub const TYPE_SCENE: i32 = 3;

/// `OBS_SOURCE_VIDEO` (`obs-source.h`).
pub const FLAG_VIDEO: u32 = 1 << 0;
/// `OBS_SOURCE_AUDIO`: not read here.
pub const FLAG_AUDIO: u32 = 1 << 1;
/// `OBS_SOURCE_ASYNC`.
pub const FLAG_ASYNC: u32 = 1 << 2;
/// `OBS_SOURCE_CUSTOM_DRAW`.
pub const FLAG_CUSTOM_DRAW: u32 = 1 << 3;
/// `OBS_SOURCE_CAP_DISABLED`: not read here.
pub const FLAG_CAP_DISABLED: u32 = 1 << 10;
/// `OBS_SOURCE_SRGB`.
pub const FLAG_SRGB: u32 = 1 << 15;

/// `OBS_NO_DIRECT_RENDERING` (`enum obs_allow_direct_render`, `obs.h`).
pub const NO_DIRECT_RENDERING: i32 = 0;
/// `OBS_ALLOW_DIRECT_RENDERING`.
pub const ALLOW_DIRECT_RENDERING: i32 = 1;

/// `GS_CS_SRGB` (`enum gs_color_space`, `graphics/graphics.h`).
pub const GS_CS_SRGB: i32 = 0;
/// `GS_CS_SRGB_16F`.
pub const GS_CS_SRGB_16F: i32 = 1;
/// `GS_CS_709_EXTENDED`.
pub const GS_CS_709_EXTENDED: i32 = 2;
/// `GS_CS_709_SCRGB`.
pub const GS_CS_709_SCRGB: i32 = 3;

/// `GS_RG16`, the last `enum gs_color_format` value.
pub const GS_RG16: i32 = 22;

/// `enum obs_source_render_route_filter`: what a filter render step finds.
/// The discriminants are the C's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FilterCheck {
    /// A target and a parent: go on.
    Ready = 0,
    /// No target (checked first).
    NoTarget = 1,
    /// A target but no parent.
    NoParent = 2,
}

impl FilterCheck {
    /// The C's value.
    pub const fn value(self) -> u32 {
        let _ = (self,);
        todo!()
    }
}

/// `enum obs_source_render_route_draw`: a draw, or the value to read before
/// the draw is known. The discriminants are the C's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Draw {
    /// `obs_source_default_render(target)`.
    Default = 0,
    /// `obs_source_render_filters(source)`.
    Filters = 1,
    /// `obs_source_main_render()` of the source or the target.
    Main = 2,
    /// `obs_source_video_render()` of the filter target.
    Target = 3,
    /// `deinterlace_render()` of the source or the target.
    Deinterlace = 4,
    /// `obs_source_render_async_video()` of the source or the target.
    Async = 5,
    /// Read `source->rendering_filter`.
    ReadRenderingFilter = 6,
    /// Read `source->info.video_render != NULL`.
    ReadVideoRender = 7,
    /// Read `source->filter_target != NULL`.
    ReadFilterTarget = 8,
    /// Call `deinterlacing_enabled(source)`.
    ReadDeinterlacing = 9,
    /// Read `target->info.video_render != NULL`.
    ReadTargetVideoRender = 10,
    /// Call `deinterlacing_enabled(target)`.
    ReadTargetDeinterlacing = 11,
}

impl Draw {
    /// Every value, in the C's order.
    pub const ALL: [Self; 12] = [
        Self::Default,
        Self::Filters,
        Self::Main,
        Self::Target,
        Self::Deinterlace,
        Self::Async,
        Self::ReadRenderingFilter,
        Self::ReadVideoRender,
        Self::ReadFilterTarget,
        Self::ReadDeinterlacing,
        Self::ReadTargetVideoRender,
        Self::ReadTargetDeinterlacing,
    ];

    /// The C's value.
    pub const fn value(self) -> u32 {
        let _ = (self,);
        todo!()
    }

    /// The step of a C value, `None` outside the enum.
    pub const fn from_value(value: u32) -> Option<Self> {
        let _ = (value,);
        todo!()
    }

    /// Whether C has a value to read before the draw is known.
    pub const fn is_read(self) -> bool {
        let _ = (self,);
        todo!()
    }
}

/// `enum obs_source_render_route_tech_end`: what
/// `obs_source_process_filter_tech_end()` draws. The discriminants are the
/// C's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TechEnd {
    /// `render_filter_bypass()`: the target, straight through the effect.
    Bypass = 0,
    /// `render_filter_tex()`: the filter's texrender, if it has a texture.
    Texture = 1,
}

impl TechEnd {
    /// The C's value.
    pub const fn value(self) -> u32 {
        let _ = (self,);
        todo!()
    }
}

/// The technique `obs_source_process_filter_tech_end()` draws with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TechName {
    /// The name the filter gave.
    Given,
    /// `"Draw"`: the filter gave none.
    Draw,
}

/// `obs_source_render_route_filter()`: no target first, then no parent. A
/// filter removed while it renders has neither (#2177).
pub fn filter_check(has_target: bool, has_parent: bool) -> FilterCheck {
    let _ = (has_target, has_parent);
    todo!()
}

/// `obs_source_render_route_video_reads_flags()`: the "no video" check
/// reads the output flags for any type but a filter.
pub fn video_reads_flags(source_type: i32) -> bool {
    let _ = (source_type,);
    todo!()
}

/// `obs_source_render_route_video_none()`: a source that is neither a
/// filter nor [`FLAG_VIDEO`] draws nothing. `flags` is 0 when not read.
pub fn video_none(source_type: i32, flags: u32) -> bool {
    let _ = (source_type, flags);
    todo!()
}

/// `obs_source_render_route_async_reads_flags()`: the async check reads the
/// output flags (again) for an input.
pub fn async_reads_flags(source_type: i32) -> bool {
    let _ = (source_type,);
    todo!()
}

/// `obs_source_render_route_async_reads_rendering_filter()`: the async
/// check reads `rendering_filter` for an input with [`FLAG_ASYNC`].
pub fn async_reads_rendering_filter(source_type: i32, flags: u32) -> bool {
    let _ = (source_type, flags);
    todo!()
}

/// `obs_source_render_route_async_update()`: update the async textures of
/// an async input not being drawn by its own filter chain. `flags` is 0
/// and `rendering_filter` false when not read.
pub fn async_update(source_type: i32, flags: u32, rendering_filter: bool) -> bool {
    let _ = (source_type, flags, rendering_filter);
    todo!()
}

/// `obs_source_render_route_reads_enabled()`: `enabled` is read only with
/// data.
pub fn reads_enabled(has_data: bool) -> bool {
    let _ = (has_data,);
    todo!()
}

/// `obs_source_render_route_inactive()`: no data, or disabled. `enabled` is
/// false when not read.
pub fn inactive(has_data: bool, enabled: bool) -> bool {
    let _ = (has_data, enabled);
    todo!()
}

/// `obs_source_render_route_draw()`: the first step of
/// `source_render_video()`'s draw.
pub fn draw(has_filters: bool) -> Draw {
    let _ = (has_filters,);
    todo!()
}

/// `obs_source_render_route_draw_next()`: the next step after reading the
/// value a `Read*` step asks for. A draw comes back unchanged.
pub fn draw_next(step: Draw, value: bool) -> Draw {
    let _ = (step, value);
    todo!()
}

/// [`draw_next`] on the C's value: a value outside the enum comes back
/// unchanged.
pub fn draw_next_value(step: u32, value: bool) -> u32 {
    let _ = (step, value);
    todo!()
}

/// `obs_source_render_route_main_custom_draw()`: [`FLAG_CUSTOM_DRAW`], the
/// source gets no effect.
pub fn main_custom_draw(flags: u32) -> bool {
    let _ = (flags,);
    todo!()
}

/// `obs_source_render_route_main_srgb_aware()`: [`FLAG_SRGB`], the source
/// keeps the linear sRGB flag.
pub fn main_srgb_aware(flags: u32) -> bool {
    let _ = (flags,);
    todo!()
}

/// `obs_source_render_route_main_reads_filters()`: `filters.num` is read
/// only for a source that is not a filter.
pub fn main_reads_filters(has_filter_parent: bool) -> bool {
    let _ = (has_filter_parent,);
    todo!()
}

/// `obs_source_render_route_main_default_effect()`: not a filter, no
/// filters and not custom draw. `has_filters` is false when not read.
pub fn main_default_effect(flags: u32, has_filter_parent: bool, has_filters: bool) -> bool {
    let _ = (flags, has_filter_parent, has_filters);
    todo!()
}

/// `obs_source_render_route_bypass_reads_space()`: the target is the
/// parent, direct rendering is allowed, the parent is neither custom draw
/// nor async and the filter and the parent agree on [`FLAG_SRGB`].
pub fn bypass_reads_space(
    target_is_parent: bool,
    allow_direct: i32,
    filter_flags: u32,
    parent_flags: u32,
) -> bool {
    let _ = (target_is_parent, allow_direct, filter_flags, parent_flags);
    todo!()
}

/// `obs_source_render_route_bypass()` (`can_bypass()`):
/// [`bypass_reads_space`] and the requested space is the current one.
/// `current_space` is any value when not read.
pub fn bypass(
    target_is_parent: bool,
    allow_direct: i32,
    filter_flags: u32,
    parent_flags: u32,
    space: i32,
    current_space: i32,
) -> bool {
    let _ = (
        target_is_parent,
        allow_direct,
        filter_flags,
        parent_flags,
        space,
        current_space,
    );
    todo!()
}

/// `obs_source_render_route_size_skip()`: a target of zero width or height.
pub fn size_skip(cx: i32, cy: i32) -> bool {
    let _ = (cx, cy);
    todo!()
}

/// `obs_source_render_route_texrender_stale()`: the texrender has another
/// format than the one asked for.
pub fn texrender_stale(texrender_format: i32, format: i32) -> bool {
    let _ = (texrender_format, format);
    todo!()
}

/// `obs_source_render_route_filter_render()`: [`Draw::Default`] for the
/// parent itself when it is neither custom draw nor async, else
/// [`Draw::Target`].
pub fn filter_render(target_is_parent: bool, parent_flags: u32) -> Draw {
    let _ = (target_is_parent, parent_flags);
    todo!()
}

/// `obs_source_render_route_tech_end_linear_srgb()`: the filter's
/// [`FLAG_SRGB`].
pub fn tech_end_linear_srgb(filter_flags: u32) -> bool {
    let _ = (filter_flags,);
    todo!()
}

/// `obs_source_render_route_tech_name()`: the given name, or `"Draw"`.
pub fn tech_name(has_tech_name: bool) -> TechName {
    let _ = (has_tech_name,);
    todo!()
}

/// `obs_source_render_route_tech_end()`: bypass, as begin decided, or the
/// texture.
pub fn tech_end(bypass_active: bool) -> TechEnd {
    let _ = (bypass_active,);
    todo!()
}

/// `obs_source_render_route_skip()`: the first step of a skipped filter's
/// draw. [`Draw::Target`] when the target is another filter,
/// [`Draw::Default`] for a parent that is neither custom draw nor async,
/// else [`Draw::ReadTargetVideoRender`].
pub fn skip(target_is_parent: bool, parent_flags: u32) -> Draw {
    let _ = (target_is_parent, parent_flags);
    todo!()
}
