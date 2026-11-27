//! `obs_source_process_filter_begin()` and
//! `obs_source_process_filter_begin_with_color_space()` (`obs.h`,
//! `EXPORT`): an effect filter starts its pass by drawing its target into
//! its texture, or by choosing to draw it straight through its effect at
//! the end (the bypass).
//!
//! The routing decisions are [`crate::render_route`]'s; this is the shell
//! around them. Everything it reads, writes and draws stays C and is
//! reached through [`FilterBegin`], one C step per method: the log lines,
//! the filter's bypass flag, `allow_direct` and texrender, its target and
//! parent, their output flags, the target's base size, the current color
//! space, the skip, and the draw into the texrender.
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

use crate::render_route::{
    Draw, FilterCheck, GS_CS_SRGB, bypass, bypass_reads_space, filter_check, filter_render,
    size_skip, texrender_stale,
};

/// What the begin reads, writes and draws, each method one C step.
pub trait FilterBegin {
    /// A source as the C passes it (never NULL here: the shim checks it).
    type Source: Copy + PartialEq;

    /// `obs_ptr_valid(filter,
    /// "obs_source_process_filter_begin_with_color_space")` failing: its one
    /// `LOG_DEBUG` line.
    fn log_null_filter(&mut self);
    /// `filter->filter_bypass_active = false`.
    fn clear_bypass(&mut self, filter: Self::Source);
    /// `obs_filter_get_target(filter)` (`None` is NULL).
    fn target(&mut self, filter: Self::Source) -> Option<Self::Source>;
    /// `obs_filter_get_parent(filter)` (`None` is NULL).
    fn parent(&mut self, filter: Self::Source) -> Option<Self::Source>;
    /// The `LOG_INFO` line `"filter '%s' being processed with no target!"`
    /// with `filter->context.name`.
    fn log_no_target(&mut self, filter: Self::Source);
    /// The `LOG_INFO` line `"filter '%s' being processed with no parent!"`
    /// with `filter->context.name`.
    fn log_no_parent(&mut self, filter: Self::Source);
    /// `source->info.output_flags`.
    fn output_flags(&mut self, source: Self::Source) -> u32;
    /// `obs_source_base_width(target)`.
    fn base_width(&mut self, target: Self::Source) -> u32;
    /// `obs_source_base_height(target)`.
    fn base_height(&mut self, target: Self::Source) -> u32;
    /// `filter->allow_direct = allow_direct`.
    fn set_allow_direct(&mut self, filter: Self::Source, allow_direct: i32);
    /// `gs_get_color_space()`.
    fn color_space(&mut self) -> i32;
    /// `filter->filter_bypass_active = true`.
    fn set_bypass(&mut self, filter: Self::Source);
    /// `obs_source_skip_video_filter(filter)`.
    fn skip_video_filter(&mut self, filter: Self::Source);
    /// `filter->filter_texrender != NULL`.
    fn has_texrender(&mut self, filter: Self::Source) -> bool;
    /// `gs_texrender_get_format(filter->filter_texrender)`.
    fn texrender_format(&mut self, filter: Self::Source) -> i32;
    /// `gs_texrender_destroy(filter->filter_texrender)`, then
    /// `filter->filter_texrender = NULL`.
    fn destroy_texrender(&mut self, filter: Self::Source);
    /// `filter->filter_texrender = gs_texrender_create(format, GS_ZS_NONE)`.
    fn create_texrender(&mut self, filter: Self::Source, format: i32);
    /// `gs_texrender_begin_with_color_space(filter->filter_texrender, cx,
    /// cy, space)`.
    fn texrender_begin(&mut self, filter: Self::Source, cx: u32, cy: u32, space: i32) -> bool;
    /// `gs_blend_state_push()`, the separate blend function, the clear to
    /// transparent and `gs_ortho(0, cx, 0, cy, -100, 100)`.
    fn draw_setup(&mut self, cx: i32, cy: i32);
    /// `obs_source_default_render(target)`.
    fn default_render(&mut self, target: Self::Source);
    /// `obs_source_video_render(target)`.
    fn video_render(&mut self, target: Self::Source);
    /// `gs_blend_state_pop()`, then
    /// `gs_texrender_end(filter->filter_texrender)`.
    fn draw_finish(&mut self, filter: Self::Source);
}

/// `obs_source_process_filter_begin_with_color_space(filter, format, space,
/// allow_direct)`: a NULL filter logs its line and fails. Otherwise the
/// bypass flag is cleared before anything else; without a target, then a
/// parent, a line is logged and it fails. The filter's, then the parent's
/// flags, the target's base width and height are read and `allow_direct` is
/// stored. When the bypass applies (the current space read only when the
/// rest holds) the flag is set and it succeeds; a zero-size target skips
/// the filter and fails. Otherwise a texrender of another format is
/// replaced, a missing one created, and when it begins the target is drawn
/// into it; it succeeds either way.
pub fn process_filter_begin_with_color_space<F: FilterBegin>(
    begin: &mut F,
    filter: Option<F::Source>,
    format: i32,
    space: i32,
    allow_direct: i32,
) -> bool {
    let Some(filter) = filter else {
        begin.log_null_filter();
        return false;
    };

    begin.clear_bypass(filter);

    let target = begin.target(filter);
    let parent = begin.parent(filter);
    let (target, parent) = match (
        filter_check(target.is_some(), parent.is_some()),
        target,
        parent,
    ) {
        (FilterCheck::Ready, Some(target), Some(parent)) => (target, parent),
        (FilterCheck::NoParent, _, _) => {
            begin.log_no_parent(filter);
            return false;
        }
        // NoTarget, and Ready never comes without both.
        _ => {
            begin.log_no_target(filter);
            return false;
        }
    };

    let filter_flags = begin.output_flags(filter);
    let parent_flags = begin.output_flags(parent);
    let cx = begin.base_width(target).cast_signed();
    let cy = begin.base_height(target).cast_signed();

    begin.set_allow_direct(filter, allow_direct);

    // The parent draws straight through the filter's effect when it uses no
    // custom effect and this is the last filter of its chain: one pass
    // fewer than through the texture.
    let target_is_parent = target == parent;
    let current_space =
        if bypass_reads_space(target_is_parent, allow_direct, filter_flags, parent_flags) {
            begin.color_space()
        } else {
            GS_CS_SRGB
        };
    if bypass(
        target_is_parent,
        allow_direct,
        filter_flags,
        parent_flags,
        space,
        current_space,
    ) {
        begin.set_bypass(filter);
        return true;
    }

    if size_skip(cx, cy) {
        begin.skip_video_filter(filter);
        return false;
    }

    if begin.has_texrender(filter) && texrender_stale(begin.texrender_format(filter), format) {
        begin.destroy_texrender(filter);
    }

    if !begin.has_texrender(filter) {
        begin.create_texrender(filter, format);
    }

    if begin.texrender_begin(filter, cx.cast_unsigned(), cy.cast_unsigned(), space) {
        begin.draw_setup(cx, cy);
        match filter_render(target_is_parent, parent_flags) {
            Draw::Default => begin.default_render(target),
            _ => begin.video_render(target),
        }
        begin.draw_finish(filter);
    }
    true
}

/// `obs_source_process_filter_begin(filter, format, allow_direct)`: the
/// begin in [`GS_CS_SRGB`].
pub fn process_filter_begin<F: FilterBegin>(
    begin: &mut F,
    filter: Option<F::Source>,
    format: i32,
    allow_direct: i32,
) -> bool {
    process_filter_begin_with_color_space(begin, filter, format, GS_CS_SRGB, allow_direct)
}
