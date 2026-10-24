//! Safe core of `libobs/obs-scene-item-names.c.h`: the scene item names and
//! coordinate keys of `scene_load_item()` and `scene_save_item()` in
//! `obs-scene.c.h`.
//!
//! `obs-scene.c.h` keeps every `obs_data_*` call, in its order, the
//! coordinate conversions, the source lookup, the transitions and the
//! private settings, and asks:
//! - [`scale_filter_from_name`], [`blend_method_from_name`] and
//!   [`blend_type_from_name`]: the value a saved name loads as. Each known
//!   name is matched, in the C's order and stopping at the first match, with
//!   the compare the caller gives: `obs-scene.c.h` passes libobs's own
//!   `astrcmpi` (in the hybrid, `obs-util`'s), so the case rules stay one
//!   implementation and this crate links no `obs-util` object. No name (C
//!   `NULL`) is never compared; it and any other name load as Disable,
//!   Default or Normal;
//! - [`scale_filter_name`], [`blend_method_name`] and [`blend_type_name`]:
//!   the name a value is saved under, a `&'static CStr`; any other value is
//!   saved as `disable`, `default` or `normal`;
//! - [`transform_keys`] and [`bounds_keys`]: which keys the position and
//!   scale, and the bounds, load from ([`Keys`]);
//! - [`save_relative`]: whether the relative keys are saved.
//!
//! Enum values from the item are `i32`, compared as they are: a value
//! outside its enum takes the C's fallthrough. Nothing here allocates, keeps
//! state, takes a lock or calls libobs.
//!
//! Characterized, not endorsed (`test_scene_item_names_steps`), and kept:
//! the names of one table load as the default in another; numbers are names,
//! not values; the bounds load from `bounds_rel` whether or not the
//! position's relative keys are there.
//!
//! No intentional differences.

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

/// `OBS_SCALE_DISABLE` (`enum obs_scale_type`).
pub const SCALE_DISABLE: i32 = 0;
/// `OBS_SCALE_POINT`.
pub const SCALE_POINT: i32 = 1;
/// `OBS_SCALE_BICUBIC`.
pub const SCALE_BICUBIC: i32 = 2;
/// `OBS_SCALE_BILINEAR`.
pub const SCALE_BILINEAR: i32 = 3;
/// `OBS_SCALE_LANCZOS`.
pub const SCALE_LANCZOS: i32 = 4;
/// `OBS_SCALE_AREA`.
pub const SCALE_AREA: i32 = 5;
/// `OBS_BLEND_METHOD_DEFAULT` (`enum obs_blending_method`).
pub const BLEND_METHOD_DEFAULT: i32 = 0;
/// `OBS_BLEND_METHOD_SRGB_OFF`.
pub const BLEND_METHOD_SRGB_OFF: i32 = 1;
/// `OBS_BLEND_NORMAL` (`enum obs_blending_type`).
pub const BLEND_NORMAL: i32 = 0;
/// `OBS_BLEND_ADDITIVE`.
pub const BLEND_ADDITIVE: i32 = 1;
/// `OBS_BLEND_SUBTRACT`.
pub const BLEND_SUBTRACT: i32 = 2;
/// `OBS_BLEND_SCREEN`.
pub const BLEND_SCREEN: i32 = 3;
/// `OBS_BLEND_MULTIPLY`.
pub const BLEND_MULTIPLY: i32 = 4;
/// `OBS_BLEND_LIGHTEN`.
pub const BLEND_LIGHTEN: i32 = 5;
/// `OBS_BLEND_DARKEN`.
pub const BLEND_DARKEN: i32 = 6;

/// The scale filter names `scene_load_item()` knows, in the order it tries
/// them.
pub const SCALE_FILTER_NAMES: [(&CStr, i32); 5] = [
    (c"point", SCALE_POINT),
    (c"bilinear", SCALE_BILINEAR),
    (c"bicubic", SCALE_BICUBIC),
    (c"lanczos", SCALE_LANCZOS),
    (c"area", SCALE_AREA),
];

/// The blend method names `scene_load_item()` knows.
pub const BLEND_METHOD_NAMES: [(&CStr, i32); 1] = [(c"srgb_off", BLEND_METHOD_SRGB_OFF)];

/// The blend type names `scene_load_item()` knows, in the order it tries
/// them.
pub const BLEND_TYPE_NAMES: [(&CStr, i32); 6] = [
    (c"additive", BLEND_ADDITIVE),
    (c"subtract", BLEND_SUBTRACT),
    (c"screen", BLEND_SCREEN),
    (c"multiply", BLEND_MULTIPLY),
    (c"lighten", BLEND_LIGHTEN),
    (c"darken", BLEND_DARKEN),
];

/// `enum obs_scene_item_keys`: which keys `scene_load_item()` reads a
/// position and scale, or the bounds, from.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Keys {
    /// `pos_rel`, `scale_rel` and `scale_ref`, or `bounds_rel`, as they are.
    Relative = 0,
    /// `pos` and `scale`, or `bounds`, converted from absolute.
    FromAbsolute = 1,
    /// `pos` and `scale`, or `bounds`, as they are.
    Absolute = 2,
}

/// The value of the first entry of `table` that `equal(name, entry)` finds
/// equal, asked in the C's order and not past the first match, else
/// `default`. `None` (the C's `NULL`) is the default without a compare, as
/// the C's `if (name)`.
fn from_name(
    table: &[(&CStr, i32)],
    default: i32,
    name: Option<&CStr>,
    mut equal: impl FnMut(&CStr, &CStr) -> bool,
) -> i32 {
    let Some(name) = name else {
        return default;
    };
    table
        .iter()
        .find(|(entry, _)| equal(name, entry))
        .map_or(default, |&(_, value)| value)
}

/// The choice of [`transform_keys`] and [`bounds_keys`], once `relative`
/// says whether every relative key is there.
fn keys(absolute_coordinates: bool, relative: bool) -> Keys {
    if !absolute_coordinates && relative {
        Keys::Relative
    } else if !absolute_coordinates {
        Keys::FromAbsolute
    } else {
        Keys::Absolute
    }
}

/// `obs_scene_item_scale_filter_from_name`: `None` is the C's `NULL`.
/// `equal(name, known)` is the C's `cmp(name, known) == 0`.
pub fn scale_filter_from_name(name: Option<&CStr>, equal: impl FnMut(&CStr, &CStr) -> bool) -> i32 {
    from_name(&SCALE_FILTER_NAMES, SCALE_DISABLE, name, equal)
}

/// `obs_scene_item_blend_method_from_name`.
pub fn blend_method_from_name(name: Option<&CStr>, equal: impl FnMut(&CStr, &CStr) -> bool) -> i32 {
    from_name(&BLEND_METHOD_NAMES, BLEND_METHOD_DEFAULT, name, equal)
}

/// `obs_scene_item_blend_type_from_name`.
pub fn blend_type_from_name(name: Option<&CStr>, equal: impl FnMut(&CStr, &CStr) -> bool) -> i32 {
    from_name(&BLEND_TYPE_NAMES, BLEND_NORMAL, name, equal)
}

/// `obs_scene_item_scale_filter_name`.
pub fn scale_filter_name(scale_filter: i32) -> &'static CStr {
    match scale_filter {
        SCALE_POINT => c"point",
        SCALE_BILINEAR => c"bilinear",
        SCALE_BICUBIC => c"bicubic",
        SCALE_LANCZOS => c"lanczos",
        SCALE_AREA => c"area",
        _ => c"disable",
    }
}

/// `obs_scene_item_blend_method_name`.
pub fn blend_method_name(blend_method: i32) -> &'static CStr {
    match blend_method {
        BLEND_METHOD_SRGB_OFF => c"srgb_off",
        _ => c"default",
    }
}

/// `obs_scene_item_blend_type_name`.
pub fn blend_type_name(blend_type: i32) -> &'static CStr {
    match blend_type {
        BLEND_NORMAL => c"normal",
        BLEND_ADDITIVE => c"additive",
        BLEND_SUBTRACT => c"subtract",
        BLEND_SCREEN => c"screen",
        BLEND_MULTIPLY => c"multiply",
        BLEND_LIGHTEN => c"lighten",
        BLEND_DARKEN => c"darken",
        _ => c"normal",
    }
}

/// `obs_scene_item_transform_keys`.
pub fn transform_keys(
    absolute_coordinates: bool,
    has_pos_rel: bool,
    has_scale_rel: bool,
    has_scale_ref: bool,
) -> Keys {
    keys(
        absolute_coordinates,
        has_pos_rel && has_scale_rel && has_scale_ref,
    )
}

/// `obs_scene_item_bounds_keys`.
pub fn bounds_keys(absolute_coordinates: bool, has_bounds_rel: bool) -> Keys {
    keys(absolute_coordinates, has_bounds_rel)
}

/// `obs_scene_item_save_relative`.
pub fn save_relative(absolute_coordinates: bool) -> bool {
    !absolute_coordinates
}
