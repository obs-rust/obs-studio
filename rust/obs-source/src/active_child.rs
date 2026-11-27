//! `obs_source_add_active_child()` and `obs_source_remove_active_child()`,
//! which scenes, groups and transitions call when a child gains or loses a
//! parent that is showing. Add refuses a NULL source, the parent itself and
//! a parent already in the child's full tree; it then activates the child
//! once for each of the parent's show references, `MAIN_VIEW` for the first
//! `activate_refs` of them and `AUX_VIEW` for the rest. Remove mirrors the
//! activations with deactivations, without the self or cycle checks. The
//! full-tree walk and the two counter loads stay C, and
//! `obs_source_activate()` / `obs_source_deactivate()` are called through
//! the C ABI ([`crate::activate`], #5811); the core reaches them through
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
pub fn view_type(index: c_long, activate_refs: c_long) -> u32 {
    if index < activate_refs {
        MAIN_VIEW
    } else {
        AUX_VIEW
    }
}

/// Whether a walk edge names `target`, as its parent or as its child
/// (`check_descendant()`).
pub fn names<S: PartialEq>(parent: Option<S>, child: Option<S>, target: S) -> bool {
    child.is_some_and(|child| child == target) || parent.is_some_and(|parent| parent == target)
}

/// The NULL checks both calls start with: the parent first, then the
/// child. `None` after logging the first one missing.
fn both<L: Libobs>(
    libobs: &mut L,
    function: &'static CStr,
    parent: Option<L::Source>,
    child: Option<L::Source>,
) -> Option<(L::Source, L::Source)> {
    let Some(parent) = parent else {
        libobs.log_null(function, PARENT);
        return None;
    };
    let Some(child) = child else {
        libobs.log_null(function, CHILD);
        return None;
    };
    Some((parent, child))
}

/// Loads the parent's two counters once each, show first (other threads
/// change them atomically, #2081), then calls `step` once per show
/// reference with its view type.
fn each_view<L: Libobs>(libobs: &mut L, parent: L::Source, mut step: impl FnMut(&mut L, u32)) {
    let show_refs = libobs.show_refs(parent);
    let activate_refs = libobs.activate_refs(parent);
    for index in 0..show_refs {
        step(libobs, view_type(index, activate_refs));
    }
}

/// `obs_source_add_active_child(parent, child)`.
pub fn add<L: Libobs>(libobs: &mut L, parent: Option<L::Source>, child: Option<L::Source>) -> bool {
    let Some((parent, child)) = both(libobs, ADD_FUNCTION, parent, child) else {
        return false;
    };
    if parent == child {
        libobs.warn_self();
        return false;
    }
    let mut exists = false;
    libobs.enum_full_tree(child, parent, &mut |edge_parent, edge_child| {
        if names(edge_parent, edge_child, parent) {
            exists = true;
        }
    });
    if exists {
        return false;
    }
    each_view(libobs, parent, |libobs, view_type| {
        libobs.activate(child, view_type)
    });
    true
}

/// `obs_source_remove_active_child(parent, child)`.
pub fn remove<L: Libobs>(libobs: &mut L, parent: Option<L::Source>, child: Option<L::Source>) {
    let Some((parent, child)) = both(libobs, REMOVE_FUNCTION, parent, child) else {
        return;
    };
    each_view(libobs, parent, |libobs, view_type| {
        libobs.deactivate(child, view_type)
    });
}
