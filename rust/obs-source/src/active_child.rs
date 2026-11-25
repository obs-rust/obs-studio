//! `obs_source_add_active_child()` and `obs_source_remove_active_child()`,
//! which scenes, groups and transitions call when a child gains or loses a
//! parent that is showing. Add refuses a NULL source, the parent itself and
//! a parent already in the child's full tree; it then activates the child
//! once for each of the parent's show references, `MAIN_VIEW` for the first
//! `activate_refs` of them and `AUX_VIEW` for the rest. Remove mirrors the
//! activations with deactivations, without the self or cycle checks. The
//! full-tree walk, the two counter loads and `obs_source_activate()` /
//! `obs_source_deactivate()` stay C; the core reaches them through
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

pub use crate::show_active_counters::{AUX_VIEW, MAIN_VIEW};
use core::ffi::{CStr, c_long};

/// `obs_source_add_active_child`, named in its NULL diagnostic.
pub const ADD_FUNCTION: &CStr = c"obs_source_add_active_child";
/// `obs_source_remove_active_child`, named in its NULL diagnostic.
pub const REMOVE_FUNCTION: &CStr = c"obs_source_remove_active_child";
/// The parameter a NULL parent's diagnostic names.
pub const PARENT: &CStr = c"parent";
/// The parameter a NULL child's diagnostic names.
pub const CHILD: &CStr = c"child";
/// The `LOG_WARNING` line of `obs_source_add_active_child(source, source)`.
pub const SELF_WARNING: &CStr = c"obs_source_add_active_child: parent == child";

/// What the two calls reach in libobs. `Source` is a non-NULL source; an
/// edge of the walk may name NULL, which arrives as `None`.
pub trait Libobs {
    /// A source handle, compared by identity.
    type Source: Copy + PartialEq;
    /// `enum_source_full_tree(root, visit, ..., stop)`: calls `visit(parent,
    /// child)` for each edge of `root`'s full tree, reporting an edge into
    /// `stop` without descending into it.
    fn enum_full_tree(
        &mut self,
        root: Self::Source,
        stop: Self::Source,
        visit: &mut dyn FnMut(Option<Self::Source>, Option<Self::Source>),
    );
    /// `os_atomic_load_long(&source->show_refs)`.
    fn show_refs(&mut self, source: Self::Source) -> c_long;
    /// `os_atomic_load_long(&source->activate_refs)`.
    fn activate_refs(&mut self, source: Self::Source) -> c_long;
    /// `obs_source_activate(source, view_type)`.
    fn activate(&mut self, source: Self::Source, view_type: u32);
    /// `obs_source_deactivate(source, view_type)`.
    fn deactivate(&mut self, source: Self::Source, view_type: u32);
    /// `LOG_DEBUG` `"<function>: Null '<parameter>' parameter"`.
    fn log_null(&mut self, function: &'static CStr, parameter: &'static CStr);
    /// `LOG_WARNING` [`SELF_WARNING`].
    fn warn_self(&mut self);
}

/// The view type of activation `index` (0-based) when the parent has
/// `activate_refs` active references: `MAIN_VIEW` below it, else `AUX_VIEW`.
#[expect(clippy::todo, reason = "RED: the port lands in the next commit")]
pub fn view_type(_index: c_long, _activate_refs: c_long) -> u32 {
    todo!("#5622")
}

/// Whether a walk edge names `target`, as its parent or as its child
/// (`check_descendant()`).
#[expect(clippy::todo, reason = "RED: the port lands in the next commit")]
pub fn names<S: PartialEq>(_parent: Option<S>, _child: Option<S>, _target: S) -> bool {
    todo!("#5622")
}

/// `obs_source_add_active_child(parent, child)`.
#[expect(clippy::todo, reason = "RED: the port lands in the next commit")]
pub fn add<L: Libobs>(
    _libobs: &mut L,
    _parent: Option<L::Source>,
    _child: Option<L::Source>,
) -> bool {
    todo!("#5622")
}

/// `obs_source_remove_active_child(parent, child)`.
#[expect(clippy::todo, reason = "RED: the port lands in the next commit")]
pub fn remove<L: Libobs>(_libobs: &mut L, _parent: Option<L::Source>, _child: Option<L::Source>) {
    todo!("#5622")
}
