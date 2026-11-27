//! `obs_source_process_filter_end()` and
//! `obs_source_process_filter_tech_end()` (`obs.h`, `EXPORT`): an effect
//! filter finishes its pass, after `obs_source_process_filter_begin()`, by
//! drawing what begin prepared through its effect.
//!
//! The routing decisions are [`crate::render_route`]'s; this is the shell
//! around them. Everything it reads, writes and draws stays C and is
//! reached through [`FilterEnd`], one C step per method: the NULL check's
//! log line, the filter's bypass flag, its target and parent, its output
//! flags, linear sRGB, its texrender's texture, and the two draws.
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
    FilterCheck, TechEnd, TechName, filter_check, tech_end, tech_end_linear_srgb, tech_name,
};

/// What the end reads, writes and draws, each method one C step.
pub trait FilterEnd {
    /// A source as the C passes it (never NULL here: the shim checks it).
    type Source: Copy + PartialEq;
    /// The caller's effect, passed through unread (may be NULL).
    type Effect: Copy;
    /// A technique name as the C passes it (never NULL here).
    type Tech: Copy;
    /// A texture of the filter's texrender (never NULL here).
    type Texture: Copy;

    /// `obs_ptr_valid(filter, "obs_source_process_filter_end")` failing:
    /// its one `LOG_DEBUG` line.
    fn log_null_filter(&mut self);
    /// `"Draw"`, the name `obs_source_process_filter_end()` passes and a
    /// NULL name stands for. Not a C step.
    fn draw_tech(&self) -> Self::Tech;
    /// `filter->filter_bypass_active`.
    fn bypass_active(&mut self, filter: Self::Source) -> bool;
    /// `filter->filter_bypass_active = false`.
    fn clear_bypass(&mut self, filter: Self::Source);
    /// `obs_filter_get_target(filter)` (`None` is NULL).
    fn target(&mut self, filter: Self::Source) -> Option<Self::Source>;
    /// `obs_filter_get_parent(filter)` (`None` is NULL).
    fn parent(&mut self, filter: Self::Source) -> Option<Self::Source>;
    /// `filter->info.output_flags`.
    fn output_flags(&mut self, filter: Self::Source) -> u32;
    /// `gs_set_linear_srgb(linear_srgb)`: the previous value.
    fn set_linear_srgb(&mut self, linear_srgb: bool) -> bool;
    /// `render_filter_bypass(target, effect, tech)`: the target, straight
    /// through each pass of the technique.
    fn render_bypass(&mut self, target: Self::Source, effect: Self::Effect, tech: Self::Tech);
    /// `gs_texrender_get_texture(filter->filter_texrender)` (`None` is
    /// NULL: no texrender, or one with no texture).
    fn texture(&mut self, filter: Self::Source) -> Option<Self::Texture>;
    /// `render_filter_tex(texture, effect, width, height, tech)`.
    fn render_tex(
        &mut self,
        texture: Self::Texture,
        effect: Self::Effect,
        width: u32,
        height: u32,
        tech: Self::Tech,
    );
}

/// `obs_source_process_filter_tech_end(filter, effect, width, height,
/// tech_name)`: a NULL filter returns, without a log line. Otherwise the
/// bypass flag is read, then cleared, before anything else, so a filter
/// removed while it renders has it cleared too. The target, then the
/// parent, are read; without either nothing more is done. Then the
/// filter's output flags set linear sRGB (on for an `OBS_SOURCE_SRGB`
/// filter), the target is drawn through the effect when begin chose the
/// bypass, else the filter's texture is when there is one, with `tech_name`
/// or `"Draw"` for NULL, and linear sRGB is put back.
pub fn process_filter_tech_end<F: FilterEnd>(
    end: &mut F,
    filter: Option<F::Source>,
    effect: F::Effect,
    width: u32,
    height: u32,
    tech_name_given: Option<F::Tech>,
) {
    let Some(filter) = filter else {
        return;
    };

    let bypass_active = end.bypass_active(filter);
    end.clear_bypass(filter);

    let target = end.target(filter);
    let parent = end.parent(filter);
    if filter_check(target.is_some(), parent.is_some()) != FilterCheck::Ready {
        return;
    }
    // Ready means the target is set (the parent is only checked).
    let Some(target) = target else {
        return;
    };

    let filter_flags = end.output_flags(filter);
    let previous = end.set_linear_srgb(tech_end_linear_srgb(filter_flags));

    let tech = match (tech_name(tech_name_given.is_some()), tech_name_given) {
        (TechName::Given, Some(given)) => given,
        (TechName::Given, None) | (TechName::Draw, _) => end.draw_tech(),
    };

    match tech_end(bypass_active) {
        TechEnd::Bypass => end.render_bypass(target, effect, tech),
        TechEnd::Texture => {
            if let Some(texture) = end.texture(filter) {
                end.render_tex(texture, effect, width, height, tech);
            }
        }
    }

    end.set_linear_srgb(previous);
}

/// `obs_source_process_filter_end(filter, effect, width, height)`: a NULL
/// filter logs its line and returns; otherwise the tech end with `"Draw"`.
pub fn process_filter_end<F: FilterEnd>(
    end: &mut F,
    filter: Option<F::Source>,
    effect: F::Effect,
    width: u32,
    height: u32,
) {
    if filter.is_none() {
        end.log_null_filter();
        return;
    }
    let draw = end.draw_tech();
    process_filter_tech_end(end, filter, effect, width, height, Some(draw));
}
