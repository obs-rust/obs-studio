//! Safe core of `libobs/obs-scene-transform-update.c.h`: the transform
//! update decisions of `scene_video_render()` and
//! `update_transforms_and_prune_sources()` in `obs-scene.c.h`.
//!
//! `obs-scene.c.h` keeps the video locks, the item list walk, the recursion
//! into a group's scene, the removed-source check and the removal, the group
//! rebuild flag, `update_item_transform()`, `resize_group()` and every size
//! read, and asks each question where the C decided it:
//! - [`scene_size`]: `scene_size_changed()`, whether the scene has a new size
//!   since its last render; the stored size changes only on a change;
//! - [`item_needs_update`]: whether an item's transform is computed again on
//!   this walk: its `update_transform` flag, a source size that is not the
//!   one of its last transform, or a new scene size.
//!
//! The C reads the source size only when the flag is clear (the `||` of the
//! original stopped at the flag) and passes 0 x 0 otherwise, so a set flag
//! decides alone. Nothing here keeps state, takes a lock or calls libobs.
//!
//! No intentional differences.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::as_conversions,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable
)]

/// A width and height in pixels: a scene's or a source's size, or the size
/// stored at the last render or transform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

impl Size {
    /// `width` x `height`.
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

/// `scene_size_changed()`: `false` when `now` is the stored size; otherwise
/// stores `now` in `last` and returns `true`.
pub fn scene_size(last: &mut Size, now: Size) -> bool {
    if *last == now {
        return false;
    }

    *last = now;
    true
}

/// The test of `update_transforms_and_prune_sources()`, with
/// `source_size_changed()`'s comparison: `true` when `update_transform` is
/// set, when `source` is not `last` (each dimension compared on its own), or
/// when `scene_size_changed` is set. `source` is ignored when
/// `update_transform` is set (the C does not read it then).
pub fn item_needs_update(
    update_transform: bool,
    last: Size,
    source: Size,
    scene_size_changed: bool,
) -> bool {
    update_transform || source != last || scene_size_changed
}
