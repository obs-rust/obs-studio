//! C ABI for `libobs/obs-scene-transform-update.c.h`, the transform update
//! decisions of `scene_video_render()` and
//! `update_transforms_and_prune_sources()`, declared in
//! `libobs/obs-scene-transform-update.h` without `EXPORT`.
//!
//! `obs-scene.c.h` (still C) reads the sizes and the item's flag where it
//! made each decision and passes them by value, the scene's stored size by
//! pointer to its two fields. So no scene or item struct is mirrored, no
//! source pointer crosses, and these shims take no lock and call nothing in
//! libobs. Both stay hidden (`rust-hidden.txt`).
//!
//! No intentional differences. A null pointer is not checked: libobs passes
//! the scene's own fields.

use crate::transform_update::{self as core, Size};

/// `bool obs_scene_transform_update_scene_size(uint32_t *last_width,
/// uint32_t *last_height, uint32_t width, uint32_t height)`.
///
/// # Safety
///
/// `last_width` and `last_height` point to initialized, aligned `uint32_t`s
/// that nothing else reads or writes during the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_scene_transform_update_scene_size(
    last_width: *mut u32,
    last_height: *mut u32,
    width: u32,
    height: u32,
) -> bool {
    // SAFETY: the caller guarantees both pointers are readable.
    let mut last = unsafe { Size::new(*last_width, *last_height) };
    let changed = core::scene_size(&mut last, Size::new(width, height));
    // Written only on a change, as the C does.
    if changed {
        // SAFETY: the caller guarantees both pointers are writable and that
        // nothing else uses them during the call.
        unsafe {
            *last_width = last.width;
            *last_height = last.height;
        }
    }
    changed
}

/// `bool obs_scene_transform_update_item(bool update_transform, uint32_t
/// last_width, uint32_t last_height, uint32_t width, uint32_t height, bool
/// scene_size_changed)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_scene_transform_update_item(
    update_transform: bool,
    last_width: u32,
    last_height: u32,
    width: u32,
    height: u32,
    scene_size_changed: bool,
) -> bool {
    core::item_needs_update(
        update_transform,
        Size::new(last_width, last_height),
        Size::new(width, height),
        scene_size_changed,
    )
}
