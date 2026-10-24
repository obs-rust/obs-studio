//! obs-scene: the scene item names and coordinate keys
//! (`libobs/obs-scene-item-names.c.h`). Loading a scene collection matches
//! each item's scale filter, blend method and blend type names and picks its
//! keys; saving one names the three values. Each side runs the calls for 16
//! items: their saved names in the case OBS writes, some in upper case and
//! some unknown. Each loader gets its library's astrcmpi: obs-util's for the
//! shim (as the hybrid passes it), the C dstr.c's for the oracle.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::dstr::oracle_astrcmpi;
use obs_c_oracle::scene_item_names::{
    oracle_obs_scene_item_blend_method_from_name, oracle_obs_scene_item_blend_method_name,
    oracle_obs_scene_item_blend_type_from_name, oracle_obs_scene_item_blend_type_name,
    oracle_obs_scene_item_bounds_keys, oracle_obs_scene_item_scale_filter_from_name,
    oracle_obs_scene_item_scale_filter_name, oracle_obs_scene_item_transform_keys,
};
use obs_scene::ffi::scene_item_names::{
    obs_scene_item_blend_method_from_name, obs_scene_item_blend_method_name,
    obs_scene_item_blend_type_from_name, obs_scene_item_blend_type_name,
    obs_scene_item_bounds_keys, obs_scene_item_scale_filter_from_name,
    obs_scene_item_scale_filter_name, obs_scene_item_transform_keys,
};
use obs_util::ffi::dstr::astrcmpi;
use std::ffi::{CStr, c_char};
use std::hint::black_box;

const SCALE: [&CStr; 8] = [
    c"disable",
    c"point",
    c"bicubic",
    c"bilinear",
    c"lanczos",
    c"area",
    c"AREA",
    c"nearest",
];
const METHOD: [&CStr; 3] = [c"default", c"srgb_off", c"SRGB_OFF"];
const BLEND: [&CStr; 9] = [
    c"normal",
    c"additive",
    c"subtract",
    c"screen",
    c"multiply",
    c"lighten",
    c"darken",
    c"Darken",
    c"overlay",
];

/// Item `i`: its three saved names, its three values and its key flags.
#[derive(Clone, Copy)]
struct Item {
    names: [*const c_char; 3],
    values: [i32; 3],
    flags: u32,
}

fn items() -> Vec<Item> {
    (0..16usize)
        .map(|i| Item {
            names: [
                SCALE[i % SCALE.len()].as_ptr(),
                METHOD[i % METHOD.len()].as_ptr(),
                BLEND[i % BLEND.len()].as_ptr(),
            ],
            values: [(i % 7) as i32, (i % 3) as i32, (i % 8) as i32],
            flags: (i * 5) as u32,
        })
        .collect()
}

fn rust_load(items: &[Item]) -> i64 {
    items.iter().fold(0, |acc, it| {
        let it = black_box(*it);
        let f = it.flags;
        // SAFETY: static NUL-terminated strings.
        let (s, m, b) = unsafe {
            (
                obs_scene_item_scale_filter_from_name(it.names[0], astrcmpi),
                obs_scene_item_blend_method_from_name(it.names[1], astrcmpi),
                obs_scene_item_blend_type_from_name(it.names[2], astrcmpi),
            )
        };
        let t = obs_scene_item_transform_keys(f & 16 != 0, f & 1 != 0, f & 2 != 0, f & 4 != 0);
        let k = obs_scene_item_bounds_keys(f & 16 != 0, f & 8 != 0);
        acc + i64::from(s) * 1000 + i64::from(m) * 100 + i64::from(b) * 10 + i64::from(t + k)
    })
}

fn c_load(items: &[Item]) -> i64 {
    items.iter().fold(0, |acc, it| {
        let it = black_box(*it);
        let f = it.flags;
        // SAFETY: as above; the rest are plain values.
        let (s, m, b, t, k) = unsafe {
            (
                oracle_obs_scene_item_scale_filter_from_name(it.names[0], oracle_astrcmpi),
                oracle_obs_scene_item_blend_method_from_name(it.names[1], oracle_astrcmpi),
                oracle_obs_scene_item_blend_type_from_name(it.names[2], oracle_astrcmpi),
                oracle_obs_scene_item_transform_keys(
                    f & 16 != 0,
                    f & 1 != 0,
                    f & 2 != 0,
                    f & 4 != 0,
                ),
                oracle_obs_scene_item_bounds_keys(f & 16 != 0, f & 8 != 0),
            )
        };
        acc + i64::from(s) * 1000 + i64::from(m) * 100 + i64::from(b) * 10 + i64::from(t + k)
    })
}

/// The first byte of each name, so the pointers are read as the saver's
/// `obs_data_set_string()` reads them.
fn first_bytes(names: [*const c_char; 3]) -> u64 {
    names.iter().fold(0, |acc, &p| {
        // SAFETY: each saver returns a static NUL-terminated string.
        (acc << 8) | u64::from(unsafe { *p.cast::<u8>() })
    })
}

fn rust_save(items: &[Item]) -> u64 {
    items.iter().fold(0, |acc, it| {
        let it = black_box(*it);
        let names = [
            obs_scene_item_scale_filter_name(it.values[0]),
            obs_scene_item_blend_method_name(it.values[1]),
            obs_scene_item_blend_type_name(it.values[2]),
        ];
        acc.wrapping_mul(31).wrapping_add(first_bytes(names))
    })
}

fn c_save(items: &[Item]) -> u64 {
    items.iter().fold(0, |acc, it| {
        let it = black_box(*it);
        // SAFETY: plain values; static strings back.
        let names = unsafe {
            [
                oracle_obs_scene_item_scale_filter_name(it.values[0]),
                oracle_obs_scene_item_blend_method_name(it.values[1]),
                oracle_obs_scene_item_blend_type_name(it.values[2]),
            ]
        };
        acc.wrapping_mul(31).wrapping_add(first_bytes(names))
    })
}

pub fn benchmarks(criterion: &mut Criterion) {
    let items = items();
    assert_eq!(rust_load(&items), c_load(&items));
    assert_eq!(rust_save(&items), c_save(&items));
    pair(
        criterion,
        "scene_item_names/load/16",
        || rust_load(&items),
        || c_load(&items),
    );
    pair(
        criterion,
        "scene_item_names/save/16",
        || rust_save(&items),
        || c_save(&items),
    );
}
