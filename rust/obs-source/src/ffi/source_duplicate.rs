//! C ABI for `libobs/obs-source-duplicate.c.h`, the duplicate decisions of
//! `obs_source_duplicate()`, `get_new_filter_name()`,
//! `obs_scene_duplicate()`, `dup_child()` and `dup_group()`, declared in
//! `libobs/obs-source-duplicate.h` without `EXPORT`.
//!
//! `obs-source.c.h` and `obs-scene.c.h` (still C) make every lookup, read,
//! create and reference themselves and pass what they found as flags,
//! integers and, for [`obs_scene_duplicate_reuse`], two source pointers that
//! are only compared by address. So no libobs structure is mirrored, no
//! pointer is followed, and these shims take no lock and call nothing in
//! libobs. All fifteen stay hidden (`rust-hidden.txt`). The enum results are
//! returned as `int`, as the header declares them.
//!
//! Two intentional differences from C, followed by the callers:
//! - [`obs_source_duplicate_created_step`] gives `OBS_SOURCE_DUPLICATE_NONE`
//!   when the create returned no source, and `obs_source_duplicate()`
//!   releases the settings and returns NULL (the C wrote through it);
//! - [`obs_scene_duplicate_reuse`] gives `OBS_SCENE_DUPLICATE_REUSE_COPY`,
//!   and only for an earlier item whose copy was added to the new scene, so
//!   `dup_child()` takes that item's own copy (the C gave
//!   `OBS_SCENE_DUPLICATE_REUSE_AT_INDEX`, the new scene's item at the old
//!   index, another item's once an earlier item was not added).

use std::ffi::c_void;
use std::os::raw::c_int;

use crate::source_duplicate::{self as core, ItemRoute, Reuse, Step};

/// `OBS_SOURCE_DUPLICATE_NONE`.
pub const OBS_SOURCE_DUPLICATE_NONE: c_int = 0;
/// `OBS_SOURCE_DUPLICATE_REF`.
pub const OBS_SOURCE_DUPLICATE_REF: c_int = 1;
/// `OBS_SOURCE_DUPLICATE_ASK_SCENE`.
pub const OBS_SOURCE_DUPLICATE_ASK_SCENE: c_int = 2;
/// `OBS_SOURCE_DUPLICATE_ASK_GROUP`.
pub const OBS_SOURCE_DUPLICATE_ASK_GROUP: c_int = 3;
/// `OBS_SOURCE_DUPLICATE_SCENE_COPY`.
pub const OBS_SOURCE_DUPLICATE_SCENE_COPY: c_int = 4;
/// `OBS_SOURCE_DUPLICATE_ASK_FLAGS`.
pub const OBS_SOURCE_DUPLICATE_ASK_FLAGS: c_int = 5;
/// `OBS_SOURCE_DUPLICATE_CREATE`.
pub const OBS_SOURCE_DUPLICATE_CREATE: c_int = 6;
/// `OBS_SOURCE_DUPLICATE_COPY_STATE`.
pub const OBS_SOURCE_DUPLICATE_COPY_STATE: c_int = 7;

/// `OBS_SCENE_DUPLICATE_ITEM_GROUP`.
pub const OBS_SCENE_DUPLICATE_ITEM_GROUP: c_int = 0;
/// `OBS_SCENE_DUPLICATE_ITEM_CHILD`.
pub const OBS_SCENE_DUPLICATE_ITEM_CHILD: c_int = 1;
/// `OBS_SCENE_DUPLICATE_ITEM_REF`.
pub const OBS_SCENE_DUPLICATE_ITEM_REF: c_int = 2;

/// `OBS_SCENE_DUPLICATE_REUSE_NONE`.
pub const OBS_SCENE_DUPLICATE_REUSE_NONE: c_int = 0;
/// `OBS_SCENE_DUPLICATE_REUSE_AT_INDEX`.
pub const OBS_SCENE_DUPLICATE_REUSE_AT_INDEX: c_int = 1;
/// `OBS_SCENE_DUPLICATE_REUSE_COPY`.
pub const OBS_SCENE_DUPLICATE_REUSE_COPY: c_int = 2;

const fn step_value(step: Step) -> c_int {
    match step {
        Step::None => OBS_SOURCE_DUPLICATE_NONE,
        Step::Ref => OBS_SOURCE_DUPLICATE_REF,
        Step::AskScene => OBS_SOURCE_DUPLICATE_ASK_SCENE,
        Step::AskGroup => OBS_SOURCE_DUPLICATE_ASK_GROUP,
        Step::SceneCopy => OBS_SOURCE_DUPLICATE_SCENE_COPY,
        Step::AskFlags => OBS_SOURCE_DUPLICATE_ASK_FLAGS,
        Step::Create => OBS_SOURCE_DUPLICATE_CREATE,
        Step::CopyState => OBS_SOURCE_DUPLICATE_COPY_STATE,
    }
}

const fn item_value(route: ItemRoute) -> c_int {
    match route {
        ItemRoute::Group => OBS_SCENE_DUPLICATE_ITEM_GROUP,
        ItemRoute::Child => OBS_SCENE_DUPLICATE_ITEM_CHILD,
        ItemRoute::Ref => OBS_SCENE_DUPLICATE_ITEM_REF,
    }
}

const fn reuse_value(reuse: Reuse) -> c_int {
    match reuse {
        Reuse::None => OBS_SCENE_DUPLICATE_REUSE_NONE,
        Reuse::AtIndex => OBS_SCENE_DUPLICATE_REUSE_AT_INDEX,
        Reuse::Copy => OBS_SCENE_DUPLICATE_REUSE_COPY,
    }
}

/// `int obs_source_duplicate_type_step(int type)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_duplicate_type_step(source_type: c_int) -> c_int {
    step_value(core::type_step(source_type))
}

/// `int obs_source_duplicate_scene_step(bool has_scene, bool
/// create_private)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_duplicate_scene_step(has_scene: bool, create_private: bool) -> c_int {
    step_value(core::scene_step(has_scene, create_private))
}

/// `int obs_source_duplicate_group_step(bool has_group)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_duplicate_group_step(has_group: bool) -> c_int {
    step_value(core::group_step(has_group))
}

/// `int obs_source_duplicate_scene_type(bool create_private)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_duplicate_scene_type(create_private: bool) -> c_int {
    core::scene_type(create_private)
}

/// `int obs_source_duplicate_flags_step(uint32_t output_flags)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_duplicate_flags_step(output_flags: u32) -> c_int {
    step_value(core::flags_step(output_flags))
}

/// `int obs_source_duplicate_created_step(bool has_source)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_duplicate_created_step(has_source: bool) -> c_int {
    step_value(core::created_step(has_source))
}

/// `bool obs_source_duplicate_copies_filters(int type)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_duplicate_copies_filters(source_type: c_int) -> bool {
    core::copies_filters(source_type)
}

/// `int obs_source_duplicate_filter_number(int taken)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_duplicate_filter_number(taken: c_int) -> c_int {
    core::filter_number(taken)
}

/// `bool obs_scene_duplicate_unique(int type)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_scene_duplicate_unique(dup_type: c_int) -> bool {
    core::unique(dup_type)
}

/// `bool obs_scene_duplicate_private(int type)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_scene_duplicate_private(dup_type: c_int) -> bool {
    core::private(dup_type)
}

/// `bool obs_scene_duplicate_keeps_unique(bool unique, bool is_group, int
/// type)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_scene_duplicate_keeps_unique(
    unique: bool,
    is_group: bool,
    dup_type: c_int,
) -> bool {
    core::keeps_unique(unique, is_group, dup_type)
}

/// `int obs_scene_duplicate_item_route(bool is_group, bool unique)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_scene_duplicate_item_route(is_group: bool, unique: bool) -> c_int {
    item_value(core::item_route(is_group, unique))
}

/// `int obs_scene_duplicate_reuse(const void *earlier, const void *source,
/// bool earlier_copied)`. The pointers are compared, never read.
#[unsafe(no_mangle)]
pub extern "C" fn obs_scene_duplicate_reuse(
    earlier: *const c_void,
    source: *const c_void,
    earlier_copied: bool,
) -> c_int {
    reuse_value(core::reuse(earlier, source, earlier_copied))
}

/// `bool obs_scene_duplicate_keeps_name(bool make_private)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_scene_duplicate_keeps_name(make_private: bool) -> bool {
    core::keeps_name(make_private)
}

/// `bool obs_scene_duplicate_resizes(bool is_group)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_scene_duplicate_resizes(is_group: bool) -> bool {
    core::resizes(is_group)
}
