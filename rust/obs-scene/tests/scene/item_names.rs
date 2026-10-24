//! Tier 1: `obs_scene::item_names`, the scene item names and coordinate
//! keys of `scene_load_item()` and `scene_save_item()`.
//!
//! The `steps_*` cases port `test/cmocka/test_scene_item_names_steps.c` 1:1:
//! its 14 cases under the same names, in the same order, with the same
//! inputs and expectations, the characterized-not-endorsed cases included.
//! As in the C test, the loaders are given `astrcmpi`: here `obs-util`'s,
//! the one the hybrid libobs passes.
//!
//! The `edge_*` cases are new: every upper/lower-case spelling of every
//! name; every name with one byte changed, cut or added; the tables' own
//! shape; and proptests of the loaders (their values and the compares they
//! make) over any byte string and of the savers over any `i32`, against
//! models written another way.

use std::ffi::{CStr, CString};

use obs_scene::item_names::{
    BLEND_ADDITIVE, BLEND_DARKEN, BLEND_LIGHTEN, BLEND_METHOD_DEFAULT, BLEND_METHOD_NAMES,
    BLEND_METHOD_SRGB_OFF, BLEND_MULTIPLY, BLEND_NORMAL, BLEND_SCREEN, BLEND_SUBTRACT,
    BLEND_TYPE_NAMES, Keys, SCALE_AREA, SCALE_BICUBIC, SCALE_BILINEAR, SCALE_DISABLE,
    SCALE_FILTER_NAMES, SCALE_LANCZOS, SCALE_POINT, blend_method_from_name as blend_method_core,
    blend_method_name, blend_type_from_name as blend_type_core, blend_type_name, bounds_keys,
    save_relative, scale_filter_from_name as scale_filter_core, scale_filter_name, transform_keys,
};
use obs_util::dstr::astrcmpi;
use proptest::prelude::*;

const INT_MAX: i32 = i32::MAX;
const INT_MIN: i32 = i32::MIN;

type FromName = fn(Option<&CStr>) -> i32;

/// `astrcmpi(name, known) == 0`, as `obs-scene.c.h` asks it.
fn astrcmpi_eq(name: &CStr, known: &CStr) -> bool {
    astrcmpi(Some(name.to_bytes()), Some(known.to_bytes())) == 0
}

fn scale_filter_from_name(name: Option<&CStr>) -> i32 {
    scale_filter_core(name, astrcmpi_eq)
}

fn blend_method_from_name(name: Option<&CStr>) -> i32 {
    blend_method_core(name, astrcmpi_eq)
}

fn blend_type_from_name(name: Option<&CStr>) -> i32 {
    blend_type_core(name, astrcmpi_eq)
}

type Core = fn(Option<&CStr>, &mut dyn FnMut(&CStr, &CStr) -> bool) -> i32;

/// The known names each loader compared `name` with, in order.
fn compares(core: Core, name: Option<&CStr>) -> Vec<Vec<u8>> {
    let mut seen = Vec::new();
    core(name, &mut |n: &CStr, known: &CStr| {
        assert_eq!(Some(n), name, "the compare is given the name first");
        seen.push(known.to_bytes().to_vec());
        astrcmpi_eq(n, known)
    });
    seen
}

const SCALE_CORE: Core = |n, eq| scale_filter_core(n, eq);
const METHOD_CORE: Core = |n, eq| blend_method_core(n, eq);
const BLEND_CORE: Core = |n, eq| blend_type_core(n, eq);
type Name = fn(i32) -> &'static CStr;

fn load(from_name: FromName, name: &[u8]) -> i32 {
    let name = CString::new(name).expect("no NUL in a test name");
    from_name(Some(&name))
}

const SCALE_FILTER_CASES: &[(&[u8], i32)] = &[
    (b"point", SCALE_POINT),
    (b"POINT", SCALE_POINT),
    (b"PoInT", SCALE_POINT),
    (b"bilinear", SCALE_BILINEAR),
    (b"BILINEAR", SCALE_BILINEAR),
    (b"BiLinear", SCALE_BILINEAR),
    (b"bicubic", SCALE_BICUBIC),
    (b"BICUBIC", SCALE_BICUBIC),
    (b"BiCubic", SCALE_BICUBIC),
    (b"lanczos", SCALE_LANCZOS),
    (b"LANCZOS", SCALE_LANCZOS),
    (b"Lanczos", SCALE_LANCZOS),
    (b"area", SCALE_AREA),
    (b"AREA", SCALE_AREA),
    (b"aReA", SCALE_AREA),
];

const BLEND_METHOD_CASES: &[(&[u8], i32)] = &[
    (b"srgb_off", BLEND_METHOD_SRGB_OFF),
    (b"SRGB_OFF", BLEND_METHOD_SRGB_OFF),
    (b"sRGB_Off", BLEND_METHOD_SRGB_OFF),
];

const BLEND_TYPE_CASES: &[(&[u8], i32)] = &[
    (b"additive", BLEND_ADDITIVE),
    (b"ADDITIVE", BLEND_ADDITIVE),
    (b"Additive", BLEND_ADDITIVE),
    (b"subtract", BLEND_SUBTRACT),
    (b"SUBTRACT", BLEND_SUBTRACT),
    (b"SubTract", BLEND_SUBTRACT),
    (b"screen", BLEND_SCREEN),
    (b"SCREEN", BLEND_SCREEN),
    (b"sCrEeN", BLEND_SCREEN),
    (b"multiply", BLEND_MULTIPLY),
    (b"MULTIPLY", BLEND_MULTIPLY),
    (b"Multiply", BLEND_MULTIPLY),
    (b"lighten", BLEND_LIGHTEN),
    (b"LIGHTEN", BLEND_LIGHTEN),
    (b"LighTen", BLEND_LIGHTEN),
    (b"darken", BLEND_DARKEN),
    (b"DARKEN", BLEND_DARKEN),
    (b"darKEN", BLEND_DARKEN),
];

/// `unknown_names` of the C test, in its order.
const UNKNOWN_NAMES: &[&[u8]] = &[
    b"",
    b"unknown",
    b"point ",
    b" point",
    b"poin",
    b"pointt",
    b"bi linear",
    b"srgb_off ",
    b"srgb-off",
    b"srgboff",
    b"darken\t",
    b"darken\n",
    b"normal",
    b"default",
    b"disable",
    b"NORMAL",
    b"none",
    // bytes above 0x7F
    b"\x80",
    b"\xff",
    b"po\xc3\xafnt",
    b"\xc3\xa0rea",
    b"area\xc2\xa0",
    b"darken\xff",
    b"\xe2\x80\x8bdarken",
    b"SRGB_OFF\x80",
    // a dotted capital I (U+0130) and a dotless small i (U+0131)
    b"L\xc4\xb0GHTEN",
    b"l\xc4\xb1ghten",
];

fn check_names(what: &str, from_name: FromName, cases: &[(&[u8], i32)]) {
    for &(name, value) in cases {
        assert_eq!(
            load(from_name, name),
            value,
            "{what} {:?}",
            name.escape_ascii().to_string()
        );
    }
}

fn check_unknown(what: &str, from_name: FromName, expected: i32) {
    for (i, name) in UNKNOWN_NAMES.iter().enumerate() {
        assert_eq!(load(from_name, name), expected, "{what} unknown #{i}");
    }
    // obs_data_get_string() never gives NULL, but the C checks for it
    assert_eq!(from_name(None), expected, "{what} NULL");
}

// ---------------------------------------------------------------------------
// Load: names

#[test]
fn steps_scale_filter_from_name() {
    check_names("scale filter", scale_filter_from_name, SCALE_FILTER_CASES);
}

#[test]
fn steps_blend_method_from_name() {
    check_names("blend method", blend_method_from_name, BLEND_METHOD_CASES);
}

#[test]
fn steps_blend_type_from_name() {
    check_names("blend type", blend_type_from_name, BLEND_TYPE_CASES);
}

#[test]
fn steps_unknown_names_load_as_default() {
    check_unknown("scale filter", scale_filter_from_name, SCALE_DISABLE);
    check_unknown("blend method", blend_method_from_name, BLEND_METHOD_DEFAULT);
    check_unknown("blend type", blend_type_from_name, BLEND_NORMAL);
}

#[test]
fn steps_names_of_other_tables() {
    // each table knows only its own names
    assert_eq!(load(scale_filter_from_name, b"darken"), SCALE_DISABLE);
    assert_eq!(load(scale_filter_from_name, b"srgb_off"), SCALE_DISABLE);
    assert_eq!(load(blend_method_from_name, b"point"), BLEND_METHOD_DEFAULT);
    assert_eq!(
        load(blend_method_from_name, b"additive"),
        BLEND_METHOD_DEFAULT
    );
    assert_eq!(load(blend_type_from_name, b"area"), BLEND_NORMAL);
    assert_eq!(load(blend_type_from_name, b"srgb_off"), BLEND_NORMAL);
    // characterized, not endorsed: numbers are names, not values
    assert_eq!(load(scale_filter_from_name, b"1"), SCALE_DISABLE);
    assert_eq!(load(blend_type_from_name, b"6"), BLEND_NORMAL);
}

fn check_calls(core: Core, name: Option<&[u8]>, want: &[&[u8]]) {
    let owned = name.map(|n| CString::new(n).expect("no NUL in a test name"));
    let got = compares(core, owned.as_deref());
    let want: Vec<Vec<u8>> = want.iter().map(|w| w.to_vec()).collect();
    assert_eq!(got, want, "{:?}", name.map(<[u8]>::escape_ascii));
}

#[test]
fn steps_compare_order() {
    const SCALE: &[&[u8]] = &[b"point", b"bilinear", b"bicubic", b"lanczos", b"area"];
    const METHOD: &[&[u8]] = &[b"srgb_off"];
    const BLEND: &[&[u8]] = &[
        b"additive",
        b"subtract",
        b"screen",
        b"multiply",
        b"lighten",
        b"darken",
    ];

    // stops at the first match
    check_calls(SCALE_CORE, Some(b"POINT"), &SCALE[..1]);
    check_calls(SCALE_CORE, Some(b"bicubic"), &SCALE[..3]);
    check_calls(METHOD_CORE, Some(b"srgb_off"), &METHOD[..1]);
    check_calls(BLEND_CORE, Some(b"screen"), &BLEND[..3]);
    check_calls(BLEND_CORE, Some(b"Darken"), &BLEND[..6]);
    // every known name, then the default
    check_calls(SCALE_CORE, Some(b"disable"), SCALE);
    check_calls(METHOD_CORE, Some(b"default"), METHOD);
    check_calls(BLEND_CORE, Some(b"normal"), BLEND);
    check_calls(BLEND_CORE, Some(b""), BLEND);
    // NULL: no compare
    check_calls(SCALE_CORE, None, &[]);
    check_calls(METHOD_CORE, None, &[]);
    check_calls(BLEND_CORE, None, &[]);
}

// ---------------------------------------------------------------------------
// Save: names

#[test]
fn steps_scale_filter_names_saved() {
    assert_eq!(scale_filter_name(SCALE_DISABLE), c"disable");
    assert_eq!(scale_filter_name(SCALE_POINT), c"point");
    assert_eq!(scale_filter_name(SCALE_BICUBIC), c"bicubic");
    assert_eq!(scale_filter_name(SCALE_BILINEAR), c"bilinear");
    assert_eq!(scale_filter_name(SCALE_LANCZOS), c"lanczos");
    assert_eq!(scale_filter_name(SCALE_AREA), c"area");
    // outside the enum
    assert_eq!(scale_filter_name(-1), c"disable");
    assert_eq!(scale_filter_name(6), c"disable");
    assert_eq!(scale_filter_name(INT_MIN), c"disable");
    assert_eq!(scale_filter_name(INT_MAX), c"disable");
}

#[test]
fn steps_blend_method_names_saved() {
    assert_eq!(blend_method_name(BLEND_METHOD_DEFAULT), c"default");
    assert_eq!(blend_method_name(BLEND_METHOD_SRGB_OFF), c"srgb_off");
    // outside the enum
    assert_eq!(blend_method_name(-1), c"default");
    assert_eq!(blend_method_name(2), c"default");
    assert_eq!(blend_method_name(INT_MIN), c"default");
    assert_eq!(blend_method_name(INT_MAX), c"default");
}

#[test]
fn steps_blend_type_names_saved() {
    assert_eq!(blend_type_name(BLEND_NORMAL), c"normal");
    assert_eq!(blend_type_name(BLEND_ADDITIVE), c"additive");
    assert_eq!(blend_type_name(BLEND_SUBTRACT), c"subtract");
    assert_eq!(blend_type_name(BLEND_SCREEN), c"screen");
    assert_eq!(blend_type_name(BLEND_MULTIPLY), c"multiply");
    assert_eq!(blend_type_name(BLEND_LIGHTEN), c"lighten");
    assert_eq!(blend_type_name(BLEND_DARKEN), c"darken");
    // outside the enum
    assert_eq!(blend_type_name(-1), c"normal");
    assert_eq!(blend_type_name(7), c"normal");
    assert_eq!(blend_type_name(INT_MIN), c"normal");
    assert_eq!(blend_type_name(INT_MAX), c"normal");
}

#[test]
fn steps_saved_names_load_back() {
    // every value in each enum, and the fallback, round-trips
    for v in -2..=8 {
        let want = if (SCALE_DISABLE..=SCALE_AREA).contains(&v) {
            v
        } else {
            SCALE_DISABLE
        };
        assert_eq!(
            scale_filter_from_name(Some(scale_filter_name(v))),
            want,
            "scale filter {v}"
        );
        let want = if (BLEND_METHOD_DEFAULT..=BLEND_METHOD_SRGB_OFF).contains(&v) {
            v
        } else {
            BLEND_METHOD_DEFAULT
        };
        assert_eq!(
            blend_method_from_name(Some(blend_method_name(v))),
            want,
            "blend method {v}"
        );
        let want = if (BLEND_NORMAL..=BLEND_DARKEN).contains(&v) {
            v
        } else {
            BLEND_NORMAL
        };
        assert_eq!(
            blend_type_from_name(Some(blend_type_name(v))),
            want,
            "blend type {v}"
        );
    }
}

// ---------------------------------------------------------------------------
// Coordinate keys

#[test]
fn steps_transform_keys() {
    // all 16 patterns of the four keys, with and without absolute
    // coordinates: pos_rel, scale_rel, scale_ref, bounds_rel
    for abs in [false, true] {
        for flags in 0..16u32 {
            let pos_rel = flags & 1 != 0;
            let scale_rel = flags & 2 != 0;
            let scale_ref = flags & 4 != 0;
            let want = if abs {
                Keys::Absolute
            } else if pos_rel && scale_rel && scale_ref {
                Keys::Relative
            } else {
                Keys::FromAbsolute
            };
            assert_eq!(
                transform_keys(abs, pos_rel, scale_rel, scale_ref),
                want,
                "transform abs {abs} flags {flags}"
            );
        }
    }
}

#[test]
fn steps_bounds_keys() {
    for abs in [false, true] {
        for flags in 0..16u32 {
            let bounds_rel = flags & 8 != 0;
            let want = if abs {
                Keys::Absolute
            } else if bounds_rel {
                Keys::Relative
            } else {
                Keys::FromAbsolute
            };
            assert_eq!(
                bounds_keys(abs, bounds_rel),
                want,
                "bounds abs {abs} flags {flags}"
            );
        }
    }
    // characterized, not endorsed: the bounds load from bounds_rel without
    // the transform's three keys, and the transform ignores bounds_rel
    assert_eq!(bounds_keys(false, true), Keys::Relative);
    assert_eq!(
        transform_keys(false, false, false, false),
        Keys::FromAbsolute
    );
}

#[test]
fn steps_save_relative() {
    assert!(save_relative(false));
    assert!(!save_relative(true));
}

#[test]
fn steps_key_values() {
    assert_eq!(Keys::Relative as u32, 0);
    assert_eq!(Keys::FromAbsolute as u32, 1);
    assert_eq!(Keys::Absolute as u32, 2);
    assert_eq!(size_of::<Keys>(), size_of::<i32>());
}

// ---------------------------------------------------------------------------
// Edge cases

/// A loader: its name, the loader, its table and its default.
type LoaderCase = (&'static str, FromName, &'static [(&'static CStr, i32)], i32);

/// The three loaders with their tables and defaults.
fn loaders() -> [LoaderCase; 3] {
    [
        (
            "scale filter",
            scale_filter_from_name,
            &SCALE_FILTER_NAMES,
            SCALE_DISABLE,
        ),
        (
            "blend method",
            blend_method_from_name,
            &BLEND_METHOD_NAMES,
            BLEND_METHOD_DEFAULT,
        ),
        (
            "blend type",
            blend_type_from_name,
            &BLEND_TYPE_NAMES,
            BLEND_NORMAL,
        ),
    ]
}

/// The tables are the C's: every name lower case ASCII, distinct, mapped to
/// a distinct value that is not the default, and saved under that name.
#[test]
fn edge_tables_match_the_savers() {
    let savers: [Name; 3] = [scale_filter_name, blend_method_name, blend_type_name];
    for ((what, _, table, default), saver) in loaders().into_iter().zip(savers) {
        for (i, &(name, value)) in table.iter().enumerate() {
            let bytes = name.to_bytes();
            assert!(!bytes.is_empty(), "{what} #{i}");
            assert!(
                bytes.iter().all(|b| b.is_ascii_lowercase() || *b == b'_'),
                "{what} {name:?}"
            );
            assert_ne!(value, default, "{what} {name:?}");
            assert_eq!(saver(value), name, "{what} {value}");
            for &(other, other_value) in &table[..i] {
                assert_ne!(other, name, "{what}");
                assert_ne!(other_value, value, "{what}");
            }
        }
    }
    assert_eq!(scale_filter_name(SCALE_DISABLE), c"disable");
    assert_eq!(blend_method_name(BLEND_METHOD_DEFAULT), c"default");
    assert_eq!(blend_type_name(BLEND_NORMAL), c"normal");
}

/// Every upper/lower-case spelling of every name loads as its value.
#[test]
fn edge_every_case_of_every_name() {
    for (what, from_name, table, _) in loaders() {
        for &(name, value) in table {
            let bytes = name.to_bytes();
            let letters: Vec<usize> = (0..bytes.len())
                .filter(|&i| bytes[i].is_ascii_alphabetic())
                .collect();
            for mask in 0..1u32 << letters.len() {
                let mut spelled = bytes.to_vec();
                for (bit, &i) in letters.iter().enumerate() {
                    if mask & (1 << bit) != 0 {
                        spelled[i] = spelled[i].to_ascii_uppercase();
                    }
                }
                assert_eq!(
                    load(from_name, &spelled),
                    value,
                    "{what} {}",
                    spelled.escape_ascii()
                );
            }
        }
    }
}

/// A name with one byte changed (to any other byte that is not the same
/// letter in the other case), cut short, or with any byte added at either
/// end loads as the default.
#[test]
fn edge_near_misses_load_as_default() {
    for (what, from_name, table, default) in loaders() {
        for &(name, _) in table {
            let bytes = name.to_bytes();
            for i in 0..bytes.len() {
                for b in 1..=255u8 {
                    if b.eq_ignore_ascii_case(&bytes[i]) {
                        continue;
                    }
                    let mut changed = bytes.to_vec();
                    changed[i] = b;
                    assert_eq!(
                        load(from_name, &changed),
                        default,
                        "{what} {}",
                        changed.escape_ascii()
                    );
                }
                assert_eq!(load(from_name, &bytes[..i]), default, "{what} cut {i}");
            }
            for b in 1..=255u8 {
                let mut after = bytes.to_vec();
                after.push(b);
                assert_eq!(load(from_name, &after), default, "{what} + {b:#x}");
                let mut before = vec![b];
                before.extend_from_slice(bytes);
                assert_eq!(load(from_name, &before), default, "{what} {b:#x} +");
            }
        }
    }
}

/// The model: the first table entry equal to the name with ASCII letters
/// folded, else the default.
fn model(table: &[(&CStr, i32)], default: i32, name: &[u8]) -> i32 {
    table
        .iter()
        .find(|(entry, _)| entry.to_bytes().eq_ignore_ascii_case(name))
        .map_or(default, |&(_, value)| value)
}

/// A name: any bytes but NUL, biased towards the table's names with their
/// case and a byte or two changed.
fn any_name() -> impl Strategy<Value = Vec<u8>> {
    let known: Vec<Vec<u8>> = loaders()
        .iter()
        .flat_map(|(_, _, table, _)| table.iter().map(|(n, _)| n.to_bytes().to_vec()))
        .collect();
    prop_oneof![
        prop::collection::vec(1..=255u8, 0..24),
        (
            prop::sample::select(known),
            prop::collection::vec(any::<bool>(), 8),
            prop::option::of((0..8usize, 1..=255u8)),
            prop::option::of(1..=255u8),
        )
            .prop_map(|(mut name, upper, change, append)| {
                for (b, up) in name.iter_mut().zip(upper) {
                    if up {
                        *b = b.to_ascii_uppercase();
                    }
                }
                if let Some((i, b)) = change {
                    let i = i % name.len();
                    name[i] = b;
                }
                name.extend(append);
                name
            }),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4000))]

    /// Each loader against the model, over any name: its value, and the
    /// known names it compared, which are the table's up to and including
    /// the first match.
    #[test]
    fn edge_loaders_model(name in any_name()) {
        let cores = [SCALE_CORE, METHOD_CORE, BLEND_CORE];
        for ((what, from_name, table, default), core) in loaders().into_iter().zip(cores) {
            prop_assert_eq!(
                load(from_name, &name),
                model(table, default, &name),
                "{} {}", what, name.escape_ascii()
            );
            let hit = table
                .iter()
                .position(|(entry, _)| entry.to_bytes().eq_ignore_ascii_case(&name));
            let asked = hit.map_or(table.len(), |i| i + 1);
            let want: Vec<Vec<u8>> =
                table[..asked].iter().map(|(n, _)| n.to_bytes().to_vec()).collect();
            let c_name = CString::new(name.clone()).expect("no NUL");
            prop_assert_eq!(compares(core, Some(&c_name)), want, "{} compares", what);
        }
    }

    /// Each saver over any value: the table's name inside the enum, the
    /// default's outside it.
    #[test]
    fn edge_savers_model(value in prop_oneof![any::<i32>(), -3..10i32]) {
        let scale = if (SCALE_DISABLE..=SCALE_AREA).contains(&value) { value } else { SCALE_DISABLE };
        prop_assert_eq!(scale_filter_from_name(Some(scale_filter_name(value))), scale);
        let method = if (BLEND_METHOD_DEFAULT..=BLEND_METHOD_SRGB_OFF).contains(&value) {
            value
        } else {
            BLEND_METHOD_DEFAULT
        };
        prop_assert_eq!(blend_method_from_name(Some(blend_method_name(value))), method);
        let blend = if (BLEND_NORMAL..=BLEND_DARKEN).contains(&value) { value } else { BLEND_NORMAL };
        prop_assert_eq!(blend_type_from_name(Some(blend_type_name(value))), blend);
    }
}

/// The keys' invariants: absolute coordinates always read the absolute keys
/// as they are; otherwise relative exactly when every relative key is there.
#[test]
fn edge_keys_invariants() {
    for flags in 0..16u32 {
        let [a, b, c, d] = [0, 1, 2, 3].map(|bit| flags & (1 << bit) != 0);
        assert_eq!(transform_keys(true, a, b, c), Keys::Absolute);
        assert_eq!(bounds_keys(true, d), Keys::Absolute);
        assert_eq!(
            transform_keys(false, a, b, c) == Keys::Relative,
            a && b && c
        );
        assert_eq!(bounds_keys(false, d) == Keys::Relative, d);
        assert_ne!(transform_keys(false, a, b, c), Keys::Absolute);
        assert_ne!(bounds_keys(false, d), Keys::Absolute);
        // the save side writes the relative keys exactly when the load side
        // would read them (given they are written)
        assert_eq!(
            save_relative(false),
            transform_keys(false, true, true, true) == Keys::Relative
        );
    }
}
