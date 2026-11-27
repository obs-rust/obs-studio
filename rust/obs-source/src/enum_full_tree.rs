//! `obs_source_enum_full_tree()` and the hidden
//! `obs_source_active_child_enum_full_tree()`, the walk over every child of
//! a source, active or not, that the scene item and transition recursion
//! checks and `obs_source_add_active_child()` use. The walk refuses a NULL
//! source (logged) or one without context data (silently), returns early for
//! a non-transition type with neither `enum_all_sources` nor
//! `enum_active_sources`, then holds a reference for the call: a transition
//! reports its slots first, then the type's `enum_all_sources` runs, or its
//! `enum_active_sources` when it has none. The walk wraps the caller's
//! callback so each child's own subtree is reported before the edge into it
//! ([`tree_edge`]), except the stop source's ([`descends`]). The plugin
//! callbacks, `obs_transition_enum_sources()` and the reference calls stay
//! C; the core reaches them through [`Libobs`].

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

use core::ffi::CStr;

/// `obs_source_enum_full_tree`, named in the NULL diagnostic of both entries.
pub const FUNCTION: &CStr = c"obs_source_enum_full_tree";

/// What the walk reaches in libobs. `Source` is a non-NULL source; `Sink`
/// is the walk's callback and parameter handed on to the C.
pub trait Libobs {
    /// A source handle.
    type Source: Copy;
    /// A callback with its parameter.
    type Sink: Copy;
    /// `source->context.data != NULL`.
    fn has_data(&mut self, source: Self::Source) -> bool;
    /// `source->info.type == OBS_SOURCE_TYPE_TRANSITION`.
    fn is_transition(&mut self, source: Self::Source) -> bool;
    /// `source->info.enum_all_sources != NULL`.
    fn has_enum_all(&mut self, source: Self::Source) -> bool;
    /// `source->info.enum_active_sources != NULL`.
    fn has_enum_active(&mut self, source: Self::Source) -> bool;
    /// `obs_source_get_ref(source)`; `None` while it is being destroyed.
    fn get_ref(&mut self, source: Self::Source) -> Option<Self::Source>;
    /// `obs_source_release(source)`.
    fn release(&mut self, source: Self::Source);
    /// `obs_transition_enum_sources(source, sink)`.
    fn transition_enum(&mut self, source: Self::Source, sink: Self::Sink);
    /// `source->info.enum_all_sources(source->context.data, sink)`.
    fn enum_all(&mut self, source: Self::Source, sink: Self::Sink);
    /// `source->info.enum_active_sources(source->context.data, sink)`.
    fn enum_active(&mut self, source: Self::Source, sink: Self::Sink);
    /// `LOG_DEBUG` `"obs_source_enum_full_tree: Null 'source' parameter"`.
    fn log_null(&mut self, function: &'static CStr);
}

/// `enum_source_full_tree()` from its root, `sink` the walk's own callback.
pub fn enumerate<L: Libobs>(libobs: &mut L, source: Option<L::Source>, sink: L::Sink) {
    let _ = (libobs, source, sink);
    todo!("obs_source_enum_full_tree")
}

/// Whether the walk descends into `child` before reporting its edge: every
/// child but the stop source (`child == stop_source`, compared as pointers,
/// so with no stop source a NULL child is reported without a descent).
pub fn descends<S: PartialEq>(child: Option<S>, stop: Option<S>) -> bool {
    let _ = (child, stop);
    todo!("enum_source_full_tree_callback stop source")
}

/// `enum_source_full_tree_callback()` for a child that [`descends`], up to
/// the caller's callback: a transition child reports its slots, then a
/// child with data reports its own children through `enum_all_sources`, or
/// `enum_active_sources` when its type has no `enum_all_sources`, all
/// through the walk's `sink`. The caller then reports the edge itself.
pub fn tree_edge<L: Libobs>(libobs: &mut L, child: L::Source, sink: L::Sink) {
    let _ = (libobs, child, sink);
    todo!("enum_source_full_tree_callback")
}
