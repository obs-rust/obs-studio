//! `obs_source_skip_video_filter()` (`obs.h`, `EXPORT`): a filter with
//! nothing to draw this frame draws what is below it instead.
//!
//! The routing decisions are [`crate::render_route`]'s; this is the shell
//! around them. Everything it reads and draws stays C and is reached
//! through [`Skip`], one C step per method: the NULL check's log line, the
//! filter's target and parent, the parent's output flags, the target's
//! `video_render` and deinterlace mode, and the five draws.
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

use crate::render_route::{self, Draw, FilterCheck, draw_next, filter_check};

/// What the skip reads and draws, each method one C step.
pub trait Skip {
    /// A source as the C passes it (never NULL here: the shim checks it).
    type Source: Copy + PartialEq;

    /// `obs_ptr_valid(filter, ...)` failing: its one `LOG_DEBUG` line.
    fn log_null_filter(&mut self);
    /// `obs_filter_get_target(filter)` (`None` is NULL).
    fn target(&mut self, filter: Self::Source) -> Option<Self::Source>;
    /// `obs_filter_get_parent(filter)` (`None` is NULL).
    fn parent(&mut self, filter: Self::Source) -> Option<Self::Source>;
    /// `parent->info.output_flags`.
    fn output_flags(&mut self, parent: Self::Source) -> u32;
    /// `target->info.video_render != NULL`.
    fn has_video_render(&mut self, target: Self::Source) -> bool;
    /// `deinterlacing_enabled(target)`: its deinterlace mode is not
    /// `OBS_DEINTERLACE_MODE_DISABLE`.
    fn deinterlacing(&mut self, target: Self::Source) -> bool;
    /// `obs_source_default_render(target)`.
    fn default_render(&mut self, target: Self::Source);
    /// `obs_source_main_render(target)`.
    fn main_render(&mut self, target: Self::Source);
    /// `deinterlace_render(target)`.
    fn deinterlace_render(&mut self, target: Self::Source);
    /// `obs_source_render_async_video(target)`.
    fn async_video(&mut self, target: Self::Source);
    /// `obs_source_video_render(target)`.
    fn video_render(&mut self, target: Self::Source);
}

/// `obs_source_skip_video_filter(filter)`: a NULL filter logs its line and
/// returns. Otherwise the target, then the parent, are read; without
/// either (a filter removed while it renders, #2177) nothing is drawn.
/// Then the parent's output flags start the route, the target's
/// `video_render` and deinterlace mode are read only when it asks for
/// them, in that order, and the one draw it ends on is made.
pub fn skip_video_filter<S: Skip>(skip: &mut S, filter: Option<S::Source>) {
    let Some(filter) = filter else {
        skip.log_null_filter();
        return;
    };
    let target = skip.target(filter);
    let parent = skip.parent(filter);
    if filter_check(target.is_some(), parent.is_some()) != FilterCheck::Ready {
        return;
    }
    // Ready means both are set.
    let (Some(target), Some(parent)) = (target, parent) else {
        return;
    };

    let parent_flags = skip.output_flags(parent);
    // each Read step reads one value of the target, in the old order
    let mut route = render_route::skip(target == parent, parent_flags);
    if route == Draw::ReadTargetVideoRender {
        route = draw_next(route, skip.has_video_render(target));
    }
    if route == Draw::ReadTargetDeinterlacing {
        route = draw_next(route, skip.deinterlacing(target));
    }

    match route {
        Draw::Default => skip.default_render(target),
        Draw::Main => skip.main_render(target),
        Draw::Deinterlace => skip.deinterlace_render(target),
        Draw::Async => skip.async_video(target),
        Draw::Target => skip.video_render(target),
        // not a draw of a skipped filter
        Draw::Filters
        | Draw::ReadRenderingFilter
        | Draw::ReadVideoRender
        | Draw::ReadFilterTarget
        | Draw::ReadDeinterlacing
        | Draw::ReadTargetVideoRender
        | Draw::ReadTargetDeinterlacing => {}
    }
}
