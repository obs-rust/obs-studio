//! Safe core of `obs_source_get_color_space()`
//! (`libobs/obs-source-get-color-space.c.h`, the reference copy).
//!
//! The routing itself is [`crate::color_space`]; this is the shell around
//! it that `obs-source.c.h` kept in C until #5724:
//! - `data_valid()`: a NULL source logs `obs_ptr_valid`'s one LOG_DEBUG
//!   line, and it or a source with no data answers `GS_CS_SRGB`;
//! - the source's fields are read into a [`Node`] and routed;
//! - [`Route::AskParent`] and [`Route::AskTarget`] ask the filter parent or
//!   target the same question, with the same list (the C recursed; this
//!   loops, which is the same calls in the same order);
//! - [`Route::AsyncPick`] picks from the list against
//!   `convert_video_space()` of the source's async format and transfer;
//! - [`Route::Callback`] asks `info.video_get_color_space`, or answers
//!   `GS_CS_SRGB` without one.
//!
//! The preferred list is handed on untouched (an opaque `P`): only
//! [`Libobs::pick`] and [`Libobs::callback`] read it, as in the C.

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

use crate::color_space::{Node, Route, route};

/// `GS_CS_SRGB` (`enum gs_color_space`).
pub const SRGB: i32 = 0;

/// What `obs_source_get_color_space()` reaches in libobs.
pub trait Libobs {
    /// A source pointer, possibly NULL.
    type Source: Copy;
    /// The caller's `count` and `preferred_spaces`, handed on untouched.
    type Preferred: Copy;

    /// `data_valid(source, "obs_source_get_color_space")`: a NULL source
    /// logs one LOG_DEBUG line; false for NULL or no data.
    fn data_valid(&mut self, source: Self::Source) -> bool;
    /// The source's routing fields.
    fn node(&mut self, source: Self::Source) -> Node;
    /// `source->filter_parent`.
    fn filter_parent(&mut self, source: Self::Source) -> Self::Source;
    /// `source->filter_target`.
    fn filter_target(&mut self, source: Self::Source) -> Self::Source;
    /// `convert_video_space(source->async_format, source->async_trc)`.
    fn async_space(&mut self, source: Self::Source) -> i32;
    /// `obs_source_color_space_pick(video_space, count, preferred)`.
    fn pick(&mut self, video_space: i32, preferred: Self::Preferred) -> i32;
    /// `info.video_get_color_space(data, count, preferred)`, or `None`
    /// without a callback.
    fn callback(&mut self, source: Self::Source, preferred: Self::Preferred) -> Option<i32>;
}

/// `obs_source_get_color_space(source, count, preferred_spaces)`.
pub fn get<L: Libobs>(lib: &mut L, source: L::Source, preferred: L::Preferred) -> i32 {
    let _ = (lib, source, preferred, route as fn(Node) -> Route);
    todo!("#5724")
}
