//! `obs_source_enum_active_sources()` and `obs_source_enum_active_tree()`,
//! which show and activate propagation, scenes and the audio tick use to
//! walk a source's active children. Both refuse a NULL source (logged) or
//! one without context data (silently), return early for a non-transition
//! type without `enum_active_sources`, then hold a reference for the call:
//! a transition reports its slots first, then the type's
//! `enum_active_sources` runs. The tree walk wraps the caller's callback so
//! each child's own subtree is reported before the edge into it
//! ([`tree_edge`]). The plugin callbacks, `obs_transition_enum_sources()`
//! and the reference calls stay C; the core reaches them through
//! [`Libobs`].

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

/// `obs_source_enum_active_sources`, named in its NULL diagnostic.
pub const SOURCES_FUNCTION: &CStr = c"obs_source_enum_active_sources";
/// `obs_source_enum_active_tree`, named in its NULL diagnostic.
pub const TREE_FUNCTION: &CStr = c"obs_source_enum_active_tree";

/// What the two calls reach in libobs. `Source` is a non-NULL source;
/// `Sink` is the callback and parameter handed on to the C (the caller's
/// for the direct call, the tree walk's own for the tree).
pub trait Libobs {
    /// A source handle.
    type Source: Copy;
    /// A callback with its parameter.
    type Sink: Copy;
    /// `source->context.data != NULL`.
    fn has_data(&mut self, source: Self::Source) -> bool;
    /// `source->info.type == OBS_SOURCE_TYPE_TRANSITION`.
    fn is_transition(&mut self, source: Self::Source) -> bool;
    /// `source->info.enum_active_sources != NULL`.
    fn has_enum_active(&mut self, source: Self::Source) -> bool;
    /// `obs_source_get_ref(source)`; `None` while it is being destroyed.
    fn get_ref(&mut self, source: Self::Source) -> Option<Self::Source>;
    /// `obs_source_release(source)`.
    fn release(&mut self, source: Self::Source);
    /// `obs_transition_enum_sources(source, sink)`.
    fn transition_enum(&mut self, source: Self::Source, sink: Self::Sink);
    /// `source->info.enum_active_sources(source->context.data, sink)`.
    fn enum_active(&mut self, source: Self::Source, sink: Self::Sink);
    /// `LOG_DEBUG` `"<function>: Null 'source' parameter"`.
    fn log_null(&mut self, function: &'static CStr);
}

/// The body both calls share, `function` naming the call in diagnostics:
/// `sink` is the caller's callback for `obs_source_enum_active_sources()`
/// and the tree walk's for `obs_source_enum_active_tree()`.
pub fn enumerate<L: Libobs>(
    libobs: &mut L,
    function: &'static CStr,
    source: Option<L::Source>,
    sink: L::Sink,
) {
    let Some(source) = valid(libobs, function, source) else {
        return;
    };
    let is_transition = libobs.is_transition(source);
    if !is_transition && !libobs.has_enum_active(source) {
        return;
    }
    let held = libobs.get_ref(source);
    let Some(source) = valid(libobs, function, held) else {
        return;
    };
    if is_transition {
        libobs.transition_enum(source, sink);
    }
    if libobs.has_enum_active(source) {
        libobs.enum_active(source, sink);
    }
    libobs.release(source);
}

/// `data_valid()`: a NULL source is logged; one without context data is
/// refused silently.
fn valid<L: Libobs>(
    libobs: &mut L,
    function: &'static CStr,
    source: Option<L::Source>,
) -> Option<L::Source> {
    let Some(source) = source else {
        libobs.log_null(function);
        return None;
    };
    libobs.has_data(source).then_some(source)
}

/// `enum_source_active_tree_callback()` up to the caller's callback: a
/// transition child reports its slots and a child with `enum_active_sources`
/// and data reports its own children, both through the tree walk's `sink`.
/// The caller then reports the edge itself.
pub fn tree_edge<L: Libobs>(libobs: &mut L, child: L::Source, sink: L::Sink) {
    if libobs.is_transition(child) {
        libobs.transition_enum(child, sink);
    }
    if libobs.has_enum_active(child) && libobs.has_data(child) {
        libobs.enum_active(child, sink);
    }
}
