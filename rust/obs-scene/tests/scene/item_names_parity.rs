//! Tier 3: the shims of `obs_scene::ffi::scene_item_names` against the
//! original `obs-scene-item-names.c.h` (the oracle).
//!
//! The loaders take the compare as a parameter. The shims are given
//! `obs-util`'s `astrcmpi` (what the hybrid libobs passes) and the oracle
//! the C `dstr.c`'s (`oracle_astrcmpi`, what libobs-c-reference passes), so
//! the value runs compare the two libraries' whole name match. The compare
//! runs give both sides the same recording compare and check that they ask
//! it the same questions in the same order: the name's own pointer, then
//! each known name's bytes, stopping at the first match.
//!
//! Every output is compared: the loaded values and the keys as integers, the
//! saved names byte for byte (both are NUL-terminated static strings).
//!
//! Runs:
//! - every enum value and constant against the C header;
//! - the compares made, for `NULL`, every one-byte string and every name of
//!   the value runs below but the two-byte strings, and in the proptest;
//! - exhaustive: the keys and the save choice over all 16 patterns of the
//!   four relative keys with and without absolute coordinates; the savers
//!   over every value from -4096 to 4096 and the ends of `i32`; the loaders
//!   over `NULL`, every string of one or two bytes (65,280 strings), every
//!   upper/lower-case spelling of every name, every name with one byte
//!   changed to any other byte, cut short, or with any byte added at either
//!   end, and the characterization's unknown names;
//! - proptests (10,000 cases each): the loaders over arbitrary byte strings
//!   with no interior NUL (biased towards the names, their case and a byte
//!   or two changed), and the savers over any `i32`.
//!
//! No intentional differences. `test_scene_item_names_steps` compiles the C
//! into itself, so this is what compares the swap;
//! `test_scene_item_names_steps_rust` runs the same C test against the
//! shims, and the real-libobs scene tests see it.

use std::cell::RefCell;
use std::ffi::{CStr, CString, c_char, c_int};

use obs_c_oracle::dstr::oracle_astrcmpi;
use obs_c_oracle::scene_item_names::{
    LAYOUT_COUNT, NameCmp, oracle_obs_scene_item_blend_method_from_name,
    oracle_obs_scene_item_blend_method_name, oracle_obs_scene_item_blend_type_from_name,
    oracle_obs_scene_item_blend_type_name, oracle_obs_scene_item_bounds_keys,
    oracle_obs_scene_item_save_relative, oracle_obs_scene_item_scale_filter_from_name,
    oracle_obs_scene_item_scale_filter_name, oracle_obs_scene_item_transform_keys,
    oracle_scene_item_names_layout,
};
use obs_scene::ffi::scene_item_names as shim;
use obs_scene::item_names as core;
use obs_util::ffi::dstr::astrcmpi;
use proptest::prelude::*;

type Loader = unsafe extern "C" fn(*const c_char, NameCmp) -> c_int;
type Saver = unsafe extern "C" fn(c_int) -> *const c_char;

/// Each loader, shim and oracle.
const LOADERS: [(&str, Loader, Loader); 3] = [
    (
        "scale filter",
        shim::obs_scene_item_scale_filter_from_name,
        oracle_obs_scene_item_scale_filter_from_name,
    ),
    (
        "blend method",
        shim::obs_scene_item_blend_method_from_name,
        oracle_obs_scene_item_blend_method_from_name,
    ),
    (
        "blend type",
        shim::obs_scene_item_blend_type_from_name,
        oracle_obs_scene_item_blend_type_from_name,
    ),
];

/// Each saver, shim and oracle.
const SAVERS: [(&str, Saver, Saver); 3] = [
    (
        "scale filter",
        shim::obs_scene_item_scale_filter_name,
        oracle_obs_scene_item_scale_filter_name,
    ),
    (
        "blend method",
        shim::obs_scene_item_blend_method_name,
        oracle_obs_scene_item_blend_method_name,
    ),
    (
        "blend type",
        shim::obs_scene_item_blend_type_name,
        oracle_obs_scene_item_blend_type_name,
    ),
];

/// Both loaders' values for `name` (`None` is `NULL`): the shim with
/// `obs-util`'s `astrcmpi`, the oracle with the C one.
fn load_both(rust: Loader, c: Loader, name: Option<&CStr>) -> (c_int, c_int) {
    let ptr = name.map_or(std::ptr::null(), CStr::as_ptr);
    // SAFETY: NULL or a NUL-terminated string that outlives the calls; both
    // compares read two NUL-terminated strings.
    unsafe { (rust(ptr, astrcmpi), c(ptr, oracle_astrcmpi)) }
}

thread_local! {
    /// The calls of `record`: the name's pointer and the known name.
    static CALLS: RefCell<Vec<(usize, Vec<u8>)>> = const { RefCell::new(Vec::new()) };
}

/// A compare that records its calls and answers as the C `astrcmpi`.
unsafe extern "C" fn record(name: *const c_char, known: *const c_char) -> c_int {
    // SAFETY: the loaders pass two NUL-terminated strings.
    let known_bytes = unsafe { CStr::from_ptr(known) }.to_bytes().to_vec();
    CALLS.with(|c| c.borrow_mut().push((name as usize, known_bytes)));
    // SAFETY: as above.
    unsafe { oracle_astrcmpi(name, known) }
}

/// The calls `load` makes of the compare for `name`, and its value.
fn recorded(load: Loader, name: Option<&CStr>) -> (Vec<(usize, Vec<u8>)>, c_int) {
    CALLS.with(|c| c.borrow_mut().clear());
    let ptr = name.map_or(std::ptr::null(), CStr::as_ptr);
    // SAFETY: as in load_both.
    let value = unsafe { load(ptr, record) };
    (CALLS.with(|c| c.borrow_mut().split_off(0)), value)
}

/// Both sides ask the same compares, in the same order, for `name`.
fn check_compares(name: Option<&CStr>) {
    for (what, rust, c) in LOADERS {
        let (r, rv) = recorded(rust, name);
        let (o, ov) = recorded(c, name);
        assert_eq!(rv, ov, "{what} {name:?}");
        assert_eq!(r, o, "{what} {name:?} compares");
        if let Some(n) = name {
            assert!(r.iter().all(|(p, _)| *p == n.as_ptr() as usize), "{what}");
        } else {
            assert!(r.is_empty(), "{what} NULL");
        }
    }
}

/// The values for `bytes`, with each side's own astrcmpi.
fn check_value_of(bytes: &[u8]) {
    let name = CString::new(bytes).expect("no interior NUL");
    for (what, rust, c) in LOADERS {
        let (r, o) = load_both(rust, c, Some(&name));
        assert_eq!(r, o, "{what} {}", bytes.escape_ascii());
    }
}

/// The values and the compares for `bytes`.
fn check_name(bytes: &[u8]) {
    check_value_of(bytes);
    let name = CString::new(bytes).expect("no interior NUL");
    check_compares(Some(&name));
}

/// Both savers' names for `value`, as bytes.
fn save_both(rust: Saver, c: Saver, value: c_int) -> (Vec<u8>, Vec<u8>) {
    // SAFETY: both return a static NUL-terminated string.
    unsafe {
        let r = rust(value);
        let o = c(value);
        assert!(!r.is_null() && !o.is_null());
        (
            CStr::from_ptr(r).to_bytes().to_vec(),
            CStr::from_ptr(o).to_bytes().to_vec(),
        )
    }
}

fn check_value(value: c_int) {
    for (what, rust, c) in SAVERS {
        let (r, o) = save_both(rust, c, value);
        assert_eq!(r, o, "{what} {value}");
    }
}

fn layout(which: usize) -> usize {
    // SAFETY: plain value, no pointers.
    unsafe { oracle_scene_item_names_layout(which) }
}

fn as_usize(v: i32) -> usize {
    usize::try_from(v).expect("a non-negative constant")
}

// ---------------------------------------------------------------------------
// Constants

#[test]
fn constants_match_the_header() {
    let values = [
        size_of::<shim::obs_scene_item_keys>(),
        shim::OBS_SCENE_ITEM_KEYS_RELATIVE as usize,
        shim::OBS_SCENE_ITEM_KEYS_FROM_ABSOLUTE as usize,
        shim::OBS_SCENE_ITEM_KEYS_ABSOLUTE as usize,
        as_usize(core::SCALE_DISABLE),
        as_usize(core::SCALE_POINT),
        as_usize(core::SCALE_BICUBIC),
        as_usize(core::SCALE_BILINEAR),
        as_usize(core::SCALE_LANCZOS),
        as_usize(core::SCALE_AREA),
        as_usize(core::BLEND_METHOD_DEFAULT),
        as_usize(core::BLEND_METHOD_SRGB_OFF),
        as_usize(core::BLEND_NORMAL),
        as_usize(core::BLEND_ADDITIVE),
        as_usize(core::BLEND_SUBTRACT),
        as_usize(core::BLEND_SCREEN),
        as_usize(core::BLEND_MULTIPLY),
        as_usize(core::BLEND_LIGHTEN),
        as_usize(core::BLEND_DARKEN),
    ];
    assert_eq!(values.len(), LAYOUT_COUNT);
    for (i, &v) in values.iter().enumerate() {
        assert_eq!(v, layout(i), "value {i}");
    }
    assert_eq!(layout(LAYOUT_COUNT), 0);
}

// ---------------------------------------------------------------------------
// Exhaustive

#[test]
fn keys_every_pattern() {
    for abs in [false, true] {
        for flags in 0..16u32 {
            let [a, b, c, d] = [0, 1, 2, 3].map(|bit| flags & (1 << bit) != 0);
            // SAFETY: plain values, no pointers.
            unsafe {
                assert_eq!(
                    shim::obs_scene_item_transform_keys(abs, a, b, c),
                    oracle_obs_scene_item_transform_keys(abs, a, b, c),
                    "transform abs {abs} flags {flags}"
                );
                assert_eq!(
                    shim::obs_scene_item_bounds_keys(abs, d),
                    oracle_obs_scene_item_bounds_keys(abs, d),
                    "bounds abs {abs} flags {flags}"
                );
            }
        }
        // SAFETY: plain value, no pointers.
        unsafe {
            assert_eq!(
                shim::obs_scene_item_save_relative(abs),
                oracle_obs_scene_item_save_relative(abs),
                "save abs {abs}"
            );
        }
    }
}

#[test]
fn savers_every_small_value_and_the_ends() {
    for value in -4096..=4096 {
        check_value(value);
    }
    for value in [i32::MIN, i32::MIN + 1, i32::MAX - 1, i32::MAX] {
        check_value(value);
    }
}

#[test]
fn loaders_null() {
    for (what, rust, c) in LOADERS {
        let (r, o) = load_both(rust, c, None);
        assert_eq!(r, o, "{what} NULL");
    }
    check_compares(None);
}

#[test]
fn loaders_every_short_string() {
    check_name(b"");
    for a in 1..=255u8 {
        check_name(&[a]);
        for b in 1..=255u8 {
            check_value_of(&[a, b]);
        }
    }
}

/// Every name of every table, as the oracle's savers give them.
fn known_names() -> Vec<Vec<u8>> {
    let mut names = Vec::new();
    for (_, _, c) in SAVERS {
        for value in -1..=7 {
            // SAFETY: a static NUL-terminated string.
            let name = unsafe { CStr::from_ptr(c(value)) }.to_bytes().to_vec();
            if !names.contains(&name) {
                names.push(name);
            }
        }
    }
    names
}

#[test]
fn loaders_every_case_and_near_miss_of_every_name() {
    let names = known_names();
    // disable, point, bicubic, bilinear, lanczos, area, default, srgb_off,
    // normal, additive, subtract, screen, multiply, lighten, darken
    assert_eq!(names.len(), 15);
    for name in &names {
        let letters: Vec<usize> = (0..name.len())
            .filter(|&i| name[i].is_ascii_alphabetic())
            .collect();
        for mask in 0..1u32 << letters.len() {
            let mut spelled = name.clone();
            for (bit, &i) in letters.iter().enumerate() {
                if mask & (1 << bit) != 0 {
                    spelled[i] = spelled[i].to_ascii_uppercase();
                }
            }
            check_name(&spelled);
        }
        for i in 0..name.len() {
            for b in 1..=255u8 {
                let mut changed = name.clone();
                changed[i] = b;
                check_name(&changed);
            }
            check_name(&name[..i]);
        }
        for b in 1..=255u8 {
            let mut after = name.clone();
            after.push(b);
            check_name(&after);
            let mut before = vec![b];
            before.extend_from_slice(name);
            check_name(&before);
        }
    }
}

#[test]
fn loaders_characterization_names() {
    for name in [
        &b"unknown"[..],
        b"point ",
        b" point",
        b"bi linear",
        b"srgb-off",
        b"srgboff",
        b"darken\t",
        b"darken\n",
        b"NORMAL",
        b"none",
        b"po\xc3\xafnt",
        b"\xc3\xa0rea",
        b"area\xc2\xa0",
        b"\xe2\x80\x8bdarken",
        b"SRGB_OFF\x80",
        b"L\xc4\xb0GHTEN",
        b"l\xc4\xb1ghten",
        b"1",
        b"6",
    ] {
        check_name(name);
    }
}

// ---------------------------------------------------------------------------
// Proptests

/// Any bytes but NUL, biased towards the names with their case and a byte or
/// two changed.
fn any_name() -> impl Strategy<Value = Vec<u8>> {
    prop_oneof![
        prop::collection::vec(1..=255u8, 0..40),
        (
            prop::sample::select(known_names()),
            prop::collection::vec(any::<bool>(), 8),
            prop::collection::vec((0..8usize, 1..=255u8), 0..3),
            prop::collection::vec(1..=255u8, 0..3),
        )
            .prop_map(|(mut name, upper, changes, append)| {
                for (b, up) in name.iter_mut().zip(upper) {
                    if up {
                        *b = b.to_ascii_uppercase();
                    }
                }
                for (i, b) in changes {
                    let i = i % name.len();
                    name[i] = b;
                }
                name.extend(append);
                name
            }),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn loaders_any_bytes(name in any_name()) {
        let c_name = CString::new(name.clone()).expect("no interior NUL");
        for (what, rust, c) in LOADERS {
            let (r, o) = load_both(rust, c, Some(&c_name));
            prop_assert_eq!(r, o, "{} {}", what, name.escape_ascii());
            let (rc, rv) = recorded(rust, Some(&c_name));
            let (oc, ov) = recorded(c, Some(&c_name));
            prop_assert_eq!(rv, ov);
            prop_assert_eq!(rc, oc, "{} {} compares", what, name.escape_ascii());
        }
    }

    #[test]
    fn savers_any_value(value in any::<i32>()) {
        for (what, rust, c) in SAVERS {
            let (r, o) = save_both(rust, c, value);
            prop_assert_eq!(r, o, "{} {}", what, value);
        }
    }
}
