//! Tier 1: 1:1 port of `test/cmocka/test_effect_lookups.c`.
//!
//! Name walks are a first-match byte compare. Param indexes stop at the
//! count. Annotation indexes and pass indexes use `>`, so an index equal
//! to the count is one past the array. That return is characterized, not
//! endorsed: the test compares the pointer and does not dereference it.
//! A null array plus a zero offset is a null pointer, which is what the
//! C test observes for an empty annotation list and an empty pass list.

use obs_graphics::effect_lookups::{
    LooseIndex, count, find_name, loose_index, param_index, param_info, stored,
};
use obs_graphics::shader_param_names::param;

fn param_ptr<T>(present: bool, ptr: *const T, len: usize, index: usize) -> *const T {
    match param_index(present, len, index) {
        Some(i) => ptr.wrapping_add(i),
        None => std::ptr::null(),
    }
}

fn loose_ptr<T>(present: bool, ptr: *const T, len: usize, index: usize) -> *const T {
    match loose_index(present, len, index) {
        LooseIndex::At(i) => ptr.wrapping_add(i),
        LooseIndex::OnePast => ptr.wrapping_add(len),
        LooseIndex::Miss => std::ptr::null(),
    }
}

fn named<'a>(present: bool, names: &'a [&'a [u8]], name: &[u8]) -> Option<usize> {
    find_name(present, names, name)
}

#[test]
fn null_and_empty_test() {
    assert_eq!(named(false, &[b"Draw"], b"Draw"), None);
    assert!(stored(false, &1u8 as *const u8).is_null());
    assert_eq!(count(false, 4), 0);
    assert!(param_ptr(false, std::ptr::null::<u8>(), 0, 0).is_null());
    assert_eq!(named(false, &[b"color"], b"color"), None);
    assert!(stored(false, &2u8 as *const u8).is_null());
    assert!(stored(false, &3u8 as *const u8).is_null());

    assert_eq!(count(false, 3), 0);
    assert!(loose_ptr(false, std::ptr::null::<u8>(), 0, 0).is_null());
    assert_eq!(named(false, &[b"widget"], b"widget"), None);
    assert!(loose_ptr(false, std::ptr::null::<u8>(), 0, 0).is_null());
    assert_eq!(named(false, &[b"p0"], b"p0"), None);

    let mut name: &[u8] = b"sentinel";
    let mut ty = param::TEXTURE;
    if let Some(info) = param_info(None) {
        name = info.name;
        ty = info.type_;
    }
    assert_eq!(name, b"sentinel");
    assert_eq!(ty, param::TEXTURE);

    assert_eq!(count(true, 0), 0);
    assert!(param_ptr(true, std::ptr::null::<u8>(), 0, 0).is_null());
    assert_eq!(named(true, &[], b"color"), None);
    assert_eq!(named(true, &[], b"Draw"), None);
    assert!(stored(true, std::ptr::null::<u8>()).is_null());
    assert!(stored(true, std::ptr::null::<u8>()).is_null());
    assert!(stored(true, std::ptr::null::<u8>()).is_null());
}

#[test]
#[expect(
    clippy::cognitive_complexity,
    reason = "scenario table: one block per ported step"
)]
fn name_lookups_test() {
    let params: &[&[u8]] = &[b"color", b"Color", b"color", b""];
    assert_eq!(named(true, params, b"color"), Some(0));
    assert_ne!(named(true, params, b"color"), Some(2));
    assert_eq!(named(true, params, b"Color"), Some(1));
    assert_eq!(named(true, params, b""), Some(3));
    assert_eq!(named(true, params, b"col"), None);
    assert_eq!(named(true, params, b"missing"), None);

    let annotations: &[&[u8]] = &[b"ui", b"ui", b"widget"];
    assert_eq!(named(true, annotations, b"ui"), Some(0));
    assert_ne!(named(true, annotations, b"ui"), Some(1));
    assert_eq!(named(true, annotations, b"widget"), Some(2));
    assert_eq!(named(true, annotations, b"UI"), None);
    assert_eq!(named(true, annotations, b"color"), None);
    assert_eq!(named(true, &[] as &[&[u8]], b"ui"), None);

    let techniques: &[&[u8]] = &[b"Draw", b"Draw", b"Solid", b""];
    assert_eq!(named(true, techniques, b"Draw"), Some(0));
    assert_ne!(named(true, techniques, b"Draw"), Some(1));
    assert_eq!(named(true, techniques, b"Solid"), Some(2));
    assert_eq!(named(true, techniques, b""), Some(3));
    assert_eq!(named(true, techniques, b"draw"), None);
    assert_eq!(named(true, techniques, b"missing"), None);

    let passes: &[&[u8]] = &[b"p0", b"P0", b"p0"];
    assert_eq!(named(true, passes, b"p0"), Some(0));
    assert_ne!(named(true, passes, b"p0"), Some(2));
    assert_eq!(named(true, passes, b"P0"), Some(1));
    assert_eq!(named(true, passes, b"p1"), None);
    assert_eq!(named(true, &[] as &[&[u8]], b"p0"), None);

    // strcmp is byte-wise, so a non-UTF-8 name matches only itself.
    let raw: &[&[u8]] = &[b"c\xfflor", b"color"];
    assert_eq!(named(true, raw, b"c\xfflor"), Some(0));
    assert_eq!(named(true, raw, b"color"), Some(1));
}

#[test]
#[expect(
    clippy::cognitive_complexity,
    reason = "scenario table: one block per ported step"
)]
fn index_bounds_test() {
    let params = [0u8, 1u8];
    let params_ptr = params.as_ptr();
    assert_eq!(count(true, params.len()), 2);
    assert_eq!(param_ptr(true, params_ptr, params.len(), 0), params_ptr);
    assert_eq!(
        param_ptr(true, params_ptr, params.len(), 1),
        params_ptr.wrapping_add(1)
    );
    assert!(param_ptr(true, params_ptr, params.len(), 2).is_null());
    assert!(param_ptr(true, params_ptr, params.len(), 3).is_null());
    assert_eq!(param_index(true, params.len(), 2), None);

    let annotations = [0u8, 1u8];
    let ann_ptr = annotations.as_ptr();
    assert_eq!(count(true, annotations.len()), 2);
    assert_eq!(loose_ptr(true, ann_ptr, annotations.len(), 0), ann_ptr);
    assert_eq!(
        loose_ptr(true, ann_ptr, annotations.len(), 1),
        ann_ptr.wrapping_add(1)
    );
    // characterized, not endorsed: index == num returns one past the array
    assert_eq!(loose_index(true, annotations.len(), 2), LooseIndex::OnePast);
    assert_eq!(
        loose_ptr(true, ann_ptr, annotations.len(), 2),
        ann_ptr.wrapping_add(2)
    );
    assert_eq!(loose_index(true, annotations.len(), 3), LooseIndex::Miss);
    assert!(loose_ptr(true, ann_ptr, annotations.len(), 3).is_null());

    let bare: *const u8 = std::ptr::null();
    assert_eq!(count(true, 0), 0);
    // index == num on a null array is one past, and the pointer is null
    assert_eq!(loose_index(true, 0, 0), LooseIndex::OnePast);
    assert!(loose_ptr(true, bare, 0, 0).is_null());
    assert_eq!(loose_index(true, 0, 1), LooseIndex::Miss);
    assert!(loose_ptr(true, bare, 0, 1).is_null());

    let passes = [0u8, 1u8];
    let pass_ptr = passes.as_ptr();
    assert_eq!(loose_ptr(true, pass_ptr, passes.len(), 0), pass_ptr);
    assert_eq!(
        loose_ptr(true, pass_ptr, passes.len(), 1),
        pass_ptr.wrapping_add(1)
    );
    // characterized, not endorsed: index == num returns one past the array
    assert_eq!(loose_index(true, passes.len(), 2), LooseIndex::OnePast);
    assert_eq!(
        loose_ptr(true, pass_ptr, passes.len(), 2),
        pass_ptr.wrapping_add(2)
    );
    assert!(loose_ptr(true, pass_ptr, passes.len(), 3).is_null());

    let empty: *const u8 = std::ptr::null();
    assert!(loose_ptr(true, empty, 0, 0).is_null());
    assert!(loose_ptr(true, empty, 0, 1).is_null());
}

#[test]
fn stored_pointers_test() {
    let view = 1u8;
    let world = 2u8;
    assert_eq!(stored(true, &view as *const u8), &view as *const u8);
    assert_eq!(stored(true, &world as *const u8), &world as *const u8);

    let draw = 3u8;
    assert_eq!(stored(true, &draw as *const u8), &draw as *const u8);

    let outsider = 4u8;
    assert_eq!(stored(true, &outsider as *const u8), &outsider as *const u8);

    assert!(stored(true, std::ptr::null::<u8>()).is_null());
}

#[test]
fn param_info_test() {
    let mut name: &[u8] = b"sentinel";
    let mut ty = param::TEXTURE;
    if let Some(info) = param_info(None) {
        name = info.name;
        ty = info.type_;
    }
    assert_eq!(name, b"sentinel");
    assert_eq!(ty, param::TEXTURE);

    let color: &[u8] = b"color";
    let info = param_info(Some((color, param::VEC4))).unwrap();
    assert_eq!(info.name.as_ptr(), color.as_ptr());
    assert_eq!(info.name, b"color");
    assert_eq!(info.type_, param::VEC4);
}
