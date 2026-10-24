//! C ABI for `libobs/obs-scene-item-names.c.h`, the scene item names and
//! coordinate keys of `scene_load_item()` and `scene_save_item()`, declared
//! in `libobs/obs-scene-item-names.h` without `EXPORT`.
//!
//! `obs-scene.c.h` (still C) makes every `obs_data_*` call, in its order,
//! and passes the strings it read and the item's values and flags; it stores
//! what comes back. So `struct obs_scene_item` is never mirrored, and these
//! shims take no lock and call nothing in libobs but the compare C passes
//! (libobs's `astrcmpi`), as `cmp(name, known)` in the C's order. The
//! loaders only read the name, through `&CStr`, and hand the compare the
//! name's own pointer. The savers return a pointer to a `&'static CStr`:
//! nothing is allocated, and `obs_data_set_string()` copies it as it copied
//! the C's literal. All stay hidden (`rust-hidden.txt`).
//!
//! No intentional differences: a `NULL` name is the default without a
//! compare, as in C. A `NULL` compare is not allowed (the C would call
//! through it too); libobs always passes `astrcmpi`.

use std::ffi::{CStr, c_char, c_int};

use crate::item_names as core;

/// `obs_scene_item_name_cmp`: `astrcmpi`'s signature.
#[allow(non_camel_case_types)]
pub type obs_scene_item_name_cmp =
    unsafe extern "C" fn(name: *const c_char, known: *const c_char) -> c_int;

/// `cmp(name, known) == 0`, as the C asks it.
fn equal(cmp: obs_scene_item_name_cmp) -> impl FnMut(&CStr, &CStr) -> bool {
    move |name, known| {
        // SAFETY: both are NUL-terminated strings; the caller guarantees
        // `cmp` reads two such strings, as astrcmpi does.
        unsafe { cmp(name.as_ptr(), known.as_ptr()) == 0 }
    }
}

/// `enum obs_scene_item_keys`, as the C's enum is returned (an `int` or
/// `unsigned int`; every value fits both).
#[allow(non_camel_case_types)]
pub type obs_scene_item_keys = u32;

/// `OBS_SCENE_ITEM_KEYS_RELATIVE`.
pub const OBS_SCENE_ITEM_KEYS_RELATIVE: obs_scene_item_keys = 0;
/// `OBS_SCENE_ITEM_KEYS_FROM_ABSOLUTE`.
pub const OBS_SCENE_ITEM_KEYS_FROM_ABSOLUTE: obs_scene_item_keys = 1;
/// `OBS_SCENE_ITEM_KEYS_ABSOLUTE`.
pub const OBS_SCENE_ITEM_KEYS_ABSOLUTE: obs_scene_item_keys = 2;

fn keys_value(keys: core::Keys) -> obs_scene_item_keys {
    match keys {
        core::Keys::Relative => OBS_SCENE_ITEM_KEYS_RELATIVE,
        core::Keys::FromAbsolute => OBS_SCENE_ITEM_KEYS_FROM_ABSOLUTE,
        core::Keys::Absolute => OBS_SCENE_ITEM_KEYS_ABSOLUTE,
    }
}

/// The name C passed: `None` for `NULL`.
///
/// # Safety
///
/// `name` is `NULL` or a NUL-terminated string that outlives `'a`.
unsafe fn opt_name<'a>(name: *const c_char) -> Option<&'a CStr> {
    // SAFETY: the caller passes NULL or a NUL-terminated string.
    (!name.is_null()).then(|| unsafe { CStr::from_ptr(name) })
}

/// Same contract as C `obs_scene_item_scale_filter_from_name`.
///
/// # Safety
///
/// `name` must be `NULL` or a NUL-terminated string, and `cmp` a compare
/// of two NUL-terminated strings (libobs passes `astrcmpi`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_scene_item_scale_filter_from_name(
    name: *const c_char,
    cmp: obs_scene_item_name_cmp,
) -> c_int {
    // SAFETY: forwarded to the caller.
    core::scale_filter_from_name(unsafe { opt_name(name) }, equal(cmp))
}

/// Same contract as C `obs_scene_item_blend_method_from_name`.
///
/// # Safety
///
/// `name` must be `NULL` or a NUL-terminated string, and `cmp` a compare
/// of two NUL-terminated strings (libobs passes `astrcmpi`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_scene_item_blend_method_from_name(
    name: *const c_char,
    cmp: obs_scene_item_name_cmp,
) -> c_int {
    // SAFETY: forwarded to the caller.
    core::blend_method_from_name(unsafe { opt_name(name) }, equal(cmp))
}

/// Same contract as C `obs_scene_item_blend_type_from_name`.
///
/// # Safety
///
/// `name` must be `NULL` or a NUL-terminated string, and `cmp` a compare
/// of two NUL-terminated strings (libobs passes `astrcmpi`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_scene_item_blend_type_from_name(
    name: *const c_char,
    cmp: obs_scene_item_name_cmp,
) -> c_int {
    // SAFETY: forwarded to the caller.
    core::blend_type_from_name(unsafe { opt_name(name) }, equal(cmp))
}

/// Same contract as C `obs_scene_item_scale_filter_name`: a static string.
#[unsafe(no_mangle)]
pub extern "C" fn obs_scene_item_scale_filter_name(scale_filter: c_int) -> *const c_char {
    core::scale_filter_name(scale_filter).as_ptr()
}

/// Same contract as C `obs_scene_item_blend_method_name`: a static string.
#[unsafe(no_mangle)]
pub extern "C" fn obs_scene_item_blend_method_name(blend_method: c_int) -> *const c_char {
    core::blend_method_name(blend_method).as_ptr()
}

/// Same contract as C `obs_scene_item_blend_type_name`: a static string.
#[unsafe(no_mangle)]
pub extern "C" fn obs_scene_item_blend_type_name(blend_type: c_int) -> *const c_char {
    core::blend_type_name(blend_type).as_ptr()
}

/// Same contract as C `obs_scene_item_transform_keys`. No pointers.
#[unsafe(no_mangle)]
pub extern "C" fn obs_scene_item_transform_keys(
    absolute_coordinates: bool,
    has_pos_rel: bool,
    has_scale_rel: bool,
    has_scale_ref: bool,
) -> obs_scene_item_keys {
    keys_value(core::transform_keys(
        absolute_coordinates,
        has_pos_rel,
        has_scale_rel,
        has_scale_ref,
    ))
}

/// Same contract as C `obs_scene_item_bounds_keys`. No pointers.
#[unsafe(no_mangle)]
pub extern "C" fn obs_scene_item_bounds_keys(
    absolute_coordinates: bool,
    has_bounds_rel: bool,
) -> obs_scene_item_keys {
    keys_value(core::bounds_keys(absolute_coordinates, has_bounds_rel))
}

/// Same contract as C `obs_scene_item_save_relative`. No pointers.
#[unsafe(no_mangle)]
pub extern "C" fn obs_scene_item_save_relative(absolute_coordinates: bool) -> bool {
    core::save_relative(absolute_coordinates)
}
