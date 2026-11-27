//! `obs_source_activate()` and `obs_source_deactivate()` (`obs-internal.h`,
//! no `EXPORT`): the show and active counters of a source and of its active
//! tree, which views, canvases, active children and the four public
//! counters (`obs_source_inc_showing()` and the rest, [`crate::show_active_counters`])
//! move.
//!
//! The counters stay in the C `struct obs_source`, changed atomically
//! (`os_atomic_*_long()`), and so does the tree walk
//! (`obs_source_enum_active_tree()`); both are reached through
//! [`Counters`]. [`visit`] is the four tree callbacks of the C (`show_tree`,
//! `hide_tree`, `activate_tree`, `deactivate_tree`).
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

use core::ffi::{CStr, c_long};

pub use crate::show_active_counters::{AUX_VIEW, MAIN_VIEW};

/// The function named in [`activate`]'s NULL diagnostic.
pub const ACTIVATE_FUNCTION: &CStr = c"obs_source_activate";
/// The function named in [`deactivate`]'s NULL diagnostic.
pub const DEACTIVATE_FUNCTION: &CStr = c"obs_source_deactivate";

/// One of a source's two counters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Counter {
    /// `show_refs`.
    Show,
    /// `activate_refs`.
    Activate,
}

/// Which of the four C tree callbacks a walk runs on each child.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tree {
    /// `show_tree`: the child's `show_refs` +1.
    Show,
    /// `activate_tree`: the child's `activate_refs` +1.
    Activate,
    /// `hide_tree`: the child's `show_refs` -1.
    Hide,
    /// `deactivate_tree`: the child's `activate_refs` -1.
    Deactivate,
}

impl Tree {
    /// All four, in the C's order.
    pub const ALL: [Self; 4] = [Self::Activate, Self::Deactivate, Self::Show, Self::Hide];

    /// The counter the callback changes.
    pub fn counter(self) -> Counter {
        match self {
            Self::Show | Self::Hide => Counter::Show,
            Self::Activate | Self::Deactivate => Counter::Activate,
        }
    }

    /// True when the callback adds one, false when it subtracts one.
    pub fn raises(self) -> bool {
        matches!(self, Self::Show | Self::Activate)
    }
}

/// What activation reads and changes, each one C step.
pub trait Counters {
    /// A live source, as the C passes it.
    type Source: Copy;
    /// `os_atomic_inc_long(&source-><counter>)`.
    fn inc(&mut self, source: Self::Source, counter: Counter);
    /// `os_atomic_dec_long(&source-><counter>)`.
    fn dec(&mut self, source: Self::Source, counter: Counter);
    /// `os_atomic_load_long(&source-><counter>)`.
    fn load(&mut self, source: Self::Source, counter: Counter) -> c_long;
    /// `obs_source_enum_active_tree(source, <tree callback>, NULL)`: each
    /// child it reports goes to [`visit`] with `tree`.
    fn walk(&mut self, source: Self::Source, tree: Tree);
    /// `obs_source_valid()`'s NULL line: `"<function>: Null 'source'
    /// parameter"` at `LOG_DEBUG`.
    fn log_null(&mut self, function: &'static CStr);
}

/// `obs_source_activate(source, type)`: `None` is a NULL source, logged
/// under [`ACTIVATE_FUNCTION`], and nothing else happens. Otherwise
/// `show_refs` +1 and a [`Tree::Show`] walk; then, only for [`MAIN_VIEW`],
/// `activate_refs` +1 and a [`Tree::Activate`] walk. Any other view type
/// touches only the show counters.
pub fn activate<C: Counters>(counters: &mut C, source: Option<C::Source>, view_type: u32) {
    let _ = (counters, source, view_type);
    todo!("#5811")
}

/// `obs_source_deactivate(source, type)`: `None` is a NULL source, logged
/// under [`DEACTIVATE_FUNCTION`], and nothing else happens. Otherwise, only
/// when `show_refs` is above zero, `show_refs` -1 and a [`Tree::Hide`]
/// walk; then, only for [`MAIN_VIEW`] and only when `activate_refs` is
/// above zero, `activate_refs` -1 and a [`Tree::Deactivate`] walk. Each
/// counter is loaded once, just before its own step.
pub fn deactivate<C: Counters>(counters: &mut C, source: Option<C::Source>, view_type: u32) {
    let _ = (counters, source, view_type);
    todo!("#5811")
}

/// One tree callback on one child: its counter +1 or -1, with no check of
/// the value (a child can go below zero).
pub fn visit<C: Counters>(counters: &mut C, child: C::Source, tree: Tree) {
    let _ = (counters, child, tree);
    todo!("#5811")
}
