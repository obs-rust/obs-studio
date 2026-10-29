//! Safe core of `libobs/obs-source-duplicate.c.h`: the duplicate decisions
//! of `obs_source_duplicate()` and `get_new_filter_name()` in
//! `obs-source.c.h`, and of `obs_scene_duplicate()`, `dup_child()` and
//! `dup_group()` in `obs-scene.c.h`.
//!
//! `obs_source_duplicate()` asks one [`Step`] after each lookup or read, so
//! each lookup and read stays where the C makes it:
//! - [`type_step`]: a scene-typed source (a scene or a group) goes on to
//!   the scene lookup, any other to its output flags;
//! - [`scene_step`]: a public duplicate of a scene shares it (a new
//!   reference), a private one copies it, and no scene goes on to the group
//!   lookup;
//! - [`group_step`]: a group is copied; a scene-typed source that is
//!   neither a scene nor a group has no duplicate;
//! - [`flags_step`]: a source marked `OBS_SOURCE_DO_NOT_DUPLICATE` gives a
//!   new reference, any other a new source;
//! - [`created_step`]: the state is copied into the new source, if the
//!   create returned one.
//!
//! [`scene_type`] is the duplicate type a scene copy asks for, and
//! [`copies_filters`] whether the new source gets the filters (not for a
//! filter). [`filter_number`] is the number `get_new_filter_name()` tries
//! next ("Color", "Color 2", "Color 3", ...).
//!
//! `obs_scene_duplicate()` takes its flags from the duplicate type
//! ([`unique`], [`private`]), clears the unique flag for a public copy of a
//! group ([`keeps_unique`]: it keeps its sources) and routes each item
//! ([`item_route`]): a group is copied with the same type, a source is
//! copied when unique, else referenced. A source used by several items is
//! copied once: `dup_child()` asks [`reuse`] of each earlier item in order.
//! A copy keeps the old name only when private ([`keeps_name`]), and a group
//! is resized to its items ([`resizes`]).
//!
//! `obs-source.c.h` and `obs-scene.c.h` keep every lookup, create, reference
//! and release, `obs_data_apply()`, `obs_scene_add()`, the filter copies with
//! their add order, `filter_compatible()`, `duplicate_item_data()`,
//! `resize_scene()`, every lock, `dstr_printf()` and every log. Sources are
//! only compared here. Nothing here keeps state, takes a lock, calls libobs
//! or follows a pointer.
//!
//! Two intentional differences from the C, each a fix its caller follows:
//! - [`created_step`] stops when the create returned no source (the C
//!   wrote the copied state through it);
//! - [`reuse`] takes an earlier item's own copy, and only when that copy
//!   was added to the new scene. The C took the new scene's item at the
//!   earlier item's index, which names another item's source (or none) once
//!   an item before it was not added; a repeat now skips such an item and
//!   is copied afresh when no earlier copy was added.

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

/// `OBS_SOURCE_TYPE_FILTER` (`enum obs_source_type`).
pub const TYPE_FILTER: i32 = 1;

/// `OBS_SOURCE_TYPE_SCENE` (`enum obs_source_type`).
pub const TYPE_SCENE: i32 = 3;

/// `OBS_SOURCE_DO_NOT_DUPLICATE` (`info.output_flags`).
pub const DO_NOT_DUPLICATE: u32 = 1 << 7;

/// `OBS_SCENE_DUP_REFS` (`enum obs_scene_duplicate_type`).
pub const DUP_REFS: i32 = 0;
/// `OBS_SCENE_DUP_COPY`.
pub const DUP_COPY: i32 = 1;
/// `OBS_SCENE_DUP_PRIVATE_REFS`.
pub const DUP_PRIVATE_REFS: i32 = 2;
/// `OBS_SCENE_DUP_PRIVATE_COPY`.
pub const DUP_PRIVATE_COPY: i32 = 3;

/// `enum obs_source_duplicate_step`: what `obs_source_duplicate()` does
/// next.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Step {
    /// Return NULL.
    None,
    /// Return a new reference to the source.
    Ref,
    /// Look the scene up, then ask [`scene_step`].
    AskScene,
    /// Look the group up, then ask [`group_step`].
    AskGroup,
    /// Return the source of a copy of the scene found, of the type
    /// [`scene_type`] gives.
    SceneCopy,
    /// Read the output flags, then ask [`flags_step`].
    AskFlags,
    /// Create a new source from a copy of the settings, then ask
    /// [`created_step`].
    Create,
    /// Copy the state of the source into the new one.
    CopyState,
}

/// `enum obs_scene_duplicate_item`: how `obs_scene_duplicate()` copies one
/// item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ItemRoute {
    /// `dup_group()`: a copy of the group with the same duplicate type.
    Group,
    /// `dup_child()`: a copy of the source, or an earlier item's copy.
    Child,
    /// `new_ref()`: a new reference to the same source.
    Ref,
}

/// `enum obs_scene_duplicate_reuse`: what `dup_child()` takes from one
/// earlier item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Reuse {
    /// Nothing: go on to the next earlier item.
    None,
    /// The new scene's item at the earlier item's index (the C's answer;
    /// never given here).
    AtIndex,
    /// The earlier item's own copy.
    Copy,
}

/// `obs_source_duplicate_type_step()`: [`Step::AskScene`] for
/// `OBS_SOURCE_TYPE_SCENE`, [`Step::AskFlags`] for any other type.
pub const fn type_step(source_type: i32) -> Step {
    if source_type == TYPE_SCENE {
        Step::AskScene
    } else {
        Step::AskFlags
    }
}

/// `obs_source_duplicate_scene_step()`: [`Step::Ref`] for a public
/// duplicate of a scene, [`Step::SceneCopy`] for a private one,
/// [`Step::AskGroup`] when no scene was found.
pub const fn scene_step(has_scene: bool, create_private: bool) -> Step {
    if has_scene && !create_private {
        Step::Ref
    } else if !has_scene {
        Step::AskGroup
    } else {
        Step::SceneCopy
    }
}

/// `obs_source_duplicate_group_step()`: [`Step::SceneCopy`] for a group,
/// [`Step::None`] for a scene-typed source that is neither.
pub const fn group_step(has_group: bool) -> Step {
    if has_group {
        Step::SceneCopy
    } else {
        Step::None
    }
}

/// `obs_source_duplicate_scene_type()`: [`DUP_PRIVATE_COPY`] for a private
/// copy, [`DUP_COPY`] otherwise.
pub const fn scene_type(create_private: bool) -> i32 {
    if create_private {
        DUP_PRIVATE_COPY
    } else {
        DUP_COPY
    }
}

/// `obs_source_duplicate_flags_step()`: [`Step::Ref`] when
/// [`DO_NOT_DUPLICATE`] is set, [`Step::Create`] otherwise.
pub const fn flags_step(output_flags: u32) -> Step {
    if output_flags & DO_NOT_DUPLICATE != 0 {
        Step::Ref
    } else {
        Step::Create
    }
}

/// `obs_source_duplicate_created_step()`: [`Step::CopyState`] when the
/// create returned a source, [`Step::None`] when it did not (the C copied
/// into it regardless).
pub const fn created_step(has_source: bool) -> Step {
    if has_source {
        Step::CopyState
    } else {
        Step::None
    }
}

/// `obs_source_duplicate_copies_filters()`: every source but a filter gets
/// copies of the filters.
pub const fn copies_filters(source_type: i32) -> bool {
    source_type != TYPE_FILTER
}

/// `obs_source_duplicate_filter_number()`: after `taken` names were found
/// taken (1 after the name itself), the number of the next name to try,
/// `taken + 1`. It stops at `i32::MAX`, where the C's `int` would
/// overflow; reaching it takes that many lookups.
pub const fn filter_number(taken: i32) -> i32 {
    taken.saturating_add(1)
}

/// `obs_scene_duplicate_unique()`: [`DUP_COPY`] and [`DUP_PRIVATE_COPY`]
/// copy the items' sources.
pub const fn unique(dup_type: i32) -> bool {
    dup_type == DUP_COPY || dup_type == DUP_PRIVATE_COPY
}

/// `obs_scene_duplicate_private()`: [`DUP_PRIVATE_REFS`] and
/// [`DUP_PRIVATE_COPY`] make the new scene and its copies private.
pub const fn private(dup_type: i32) -> bool {
    dup_type == DUP_PRIVATE_REFS || dup_type == DUP_PRIVATE_COPY
}

/// `obs_scene_duplicate_keeps_unique()`: a group's sources are copied only
/// for a private copy ([`DUP_PRIVATE_COPY`]); a public copy of a group
/// keeps its sources.
pub const fn keeps_unique(unique: bool, is_group: bool, dup_type: i32) -> bool {
    if is_group && dup_type != DUP_PRIVATE_COPY {
        false
    } else {
        unique
    }
}

/// `obs_scene_duplicate_item_route()`: a group item is copied as a group,
/// any other item's source is copied when `unique`, else referenced.
pub const fn item_route(is_group: bool, unique: bool) -> ItemRoute {
    if is_group {
        ItemRoute::Group
    } else if unique {
        ItemRoute::Child
    } else {
        ItemRoute::Ref
    }
}

/// `obs_scene_duplicate_reuse()`: an earlier item with the same source
/// whose copy was added to the new scene gives that copy
/// ([`Reuse::Copy`]); any other gives [`Reuse::None`]. `dup_child()` asks
/// the earlier items in order and stops at the first that is not
/// [`Reuse::None`].
pub fn reuse<S: PartialEq>(earlier: S, source: S, earlier_copied: bool) -> Reuse {
    if earlier == source && earlier_copied {
        Reuse::Copy
    } else {
        Reuse::None
    }
}

/// `obs_scene_duplicate_keeps_name()`: a private copy keeps the old name; a
/// public one gets a new unique name.
pub const fn keeps_name(make_private: bool) -> bool {
    make_private
}

/// `obs_scene_duplicate_resizes()`: a group is resized to its items.
pub const fn resizes(is_group: bool) -> bool {
    is_group
}
