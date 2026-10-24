//! Tier 3: the source registration rule shims against the original C.
//!
//! The oracle is `libobs/obs-module-source-rules.c.h`, unmodified, with its
//! entry points renamed to `oracle_*` (`obs-c-oracle/oracle/
//! module_source_rules.c`); its versioned id is printed by the original
//! `dstr_printf` (the dstr oracle's). Compared, the C enum codes as they
//! are returned:
//!
//! - `obs_module_source_rules_list_of()` over every type around the enum and
//!   both ends of `u32`, and in a proptest;
//! - `_size_supported()` around the size libobs knows and both ends of
//!   `size_t`, and in a proptest;
//! - `_output_flags()` and `_check()` exhaustively: every type around the
//!   enum × every subset of the flags the rules read (video, audio, async,
//!   custom draw, composite), for the caller's and the normalized flags,
//!   × every set of the four callbacks; and in proptests over any type,
//!   flags and callback bits;
//! - `_versioned_id()` over ids (empty, `%` sequences, bytes outside ASCII,
//!   NULL) × versions around both ends of `int`, and in a proptest;
//! - the whole decision of `obs_register_source_s()`, each side's five
//!   functions in the C's order, over any type, flags, size (with the field
//!   ends of `struct obs_source_info` weighted), callback mask and version:
//!   the list, the size, the copy's normalized flags, the rule broken and
//!   the id.
//!
//! No intentional differences.

use core::ffi::{CStr, c_char};
use std::ffi::CString;

use obs_c_oracle::module_source_rules::{
    oracle_obs_module_source_rules_check, oracle_obs_module_source_rules_list_of,
    oracle_obs_module_source_rules_output_flags, oracle_obs_module_source_rules_size_supported,
    oracle_obs_module_source_rules_versioned_id,
};
use obs_core::ffi::module_source_rules::{
    LIST_UNKNOWN, NO_GET_NAME, OK, obs_module_source_rules_check, obs_module_source_rules_list_of,
    obs_module_source_rules_output_flags, obs_module_source_rules_size_supported,
    obs_module_source_rules_versioned_id,
};
use obs_core::module_source_rules::{
    ASYNC, AUDIO, COMPOSITE, CUSTOM_DRAW, HAS_AUDIO_RENDER, HAS_GET_HEIGHT, HAS_GET_NAME,
    HAS_GET_WIDTH, VIDEO,
};
use obs_util::ffi::bmem::bfree;
use proptest::prelude::*;

/// `sizeof(struct obs_source_info)` and the offsets the registration reads,
/// on 64-bit targets.
const SIZE: usize = 424;
const OFF_TYPE: usize = 8;
const OFF_OUTPUT_FLAGS: usize = 12;
const OFF_GET_NAME: usize = 16;
const OFF_GET_WIDTH: usize = 40;
const OFF_GET_HEIGHT: usize = 48;
const OFF_AUDIO_RENDER: usize = 232;
const OFF_VERSION: usize = 368;
const PTR: usize = 8;

const TYPES: &[u32] = &[
    0,
    1,
    2,
    3,
    4,
    5,
    7,
    0x100,
    0x101,
    0x7fff_ffff,
    0x8000_0000,
    u32::MAX - 1,
    u32::MAX,
];

/// The flags the rules read.
const RULE_FLAGS: [u32; 5] = [VIDEO, AUDIO, ASYNC, CUSTOM_DRAW, COMPOSITE];

/// Every subset of [`RULE_FLAGS`].
fn flag_sets() -> Vec<u32> {
    (0u32..32)
        .map(|m| {
            RULE_FLAGS
                .iter()
                .enumerate()
                .filter(|(i, _)| m & (1 << i) != 0)
                .fold(0, |acc, (_, f)| acc | f)
        })
        .collect()
}

const ALL_CALLBACKS: u32 = HAS_GET_NAME | HAS_GET_WIDTH | HAS_GET_HEIGHT | HAS_AUDIO_RENDER;

fn list_both(t: u32) -> (u32, u32) {
    // SAFETY: plain values.
    unsafe {
        (
            obs_module_source_rules_list_of(t),
            oracle_obs_module_source_rules_list_of(t),
        )
    }
}

fn size_both(size: usize, supported: usize) -> (bool, bool) {
    // SAFETY: plain values.
    unsafe {
        (
            obs_module_source_rules_size_supported(size, supported),
            oracle_obs_module_source_rules_size_supported(size, supported),
        )
    }
}

fn flags_both(t: u32, f: u32) -> (u32, u32) {
    // SAFETY: plain values.
    unsafe {
        (
            obs_module_source_rules_output_flags(t, f),
            oracle_obs_module_source_rules_output_flags(t, f),
        )
    }
}

fn check_both(t: u32, info: u32, flags: u32, cb: u32) -> (u32, u32) {
    // SAFETY: plain values.
    unsafe {
        (
            obs_module_source_rules_check(t, info, flags, cb),
            oracle_obs_module_source_rules_check(t, info, flags, cb),
        )
    }
}

/// The bytes of a `bmalloc`'d id, freed.
fn take(p: *mut c_char) -> Option<Vec<u8>> {
    if p.is_null() {
        return None;
    }
    // SAFETY: a NUL-terminated `bmalloc`'d string from a shim or the oracle.
    let out = unsafe { CStr::from_ptr(p) }.to_bytes().to_vec();
    // SAFETY: allocated by `bmalloc`.
    unsafe { bfree(p.cast()) };
    Some(out)
}

fn id_both(id: &Option<CString>, version: u32) -> (Option<Vec<u8>>, Option<Vec<u8>>) {
    let p = id.as_ref().map_or(core::ptr::null(), |s| s.as_ptr());
    // SAFETY: NULL or a NUL-terminated string; the results are freed.
    unsafe {
        (
            take(obs_module_source_rules_versioned_id(p, version)),
            take(oracle_obs_module_source_rules_versioned_id(p, version)),
        )
    }
}

// ------------------------------------------------------ fixed cases

#[test]
fn list_of_matches_c() {
    let mut unknown = 0;
    for &t in TYPES {
        let (rust, oracle) = list_both(t);
        assert_eq!(rust, oracle, "list_of({t:#x})");
        unknown += usize::from(rust == LIST_UNKNOWN);
    }
    assert_eq!(unknown, TYPES.len() - 4);
}

#[test]
fn size_supported_matches_c() {
    for size in [0, 1, SIZE - 1, SIZE, SIZE + 1, usize::MAX - 1, usize::MAX] {
        for supported in [0, SIZE, usize::MAX] {
            let (rust, oracle) = size_both(size, supported);
            assert_eq!(rust, oracle, "size_supported({size}, {supported})");
        }
    }
}

#[test]
fn output_flags_match_c() {
    for &t in TYPES {
        for f in flag_sets() {
            for high in [0, 1 << 8, 1 << 31, !0x7f] {
                let (rust, oracle) = flags_both(t, f | high);
                assert_eq!(rust, oracle, "output_flags({t:#x}, {:#x})", f | high);
            }
        }
    }
}

#[test]
fn check_matches_c() {
    let sets = flag_sets();
    let mut ok = 0usize;
    let mut broken = [0usize; 7];
    for &t in TYPES {
        for &info in &sets {
            for &flags in &sets {
                for cb in 0..=ALL_CALLBACKS {
                    let (rust, oracle) = check_both(t, info, flags, cb);
                    assert_eq!(
                        rust, oracle,
                        "check({t:#x}, {info:#x}, {flags:#x}, {cb:#x})"
                    );
                    ok += usize::from(rust == OK);
                    if let Some(n) = broken.get_mut(rust as usize) {
                        *n += 1;
                    }
                }
            }
        }
    }
    // Sanity: every code is reached.
    assert!(ok > 0);
    assert!(broken.iter().all(|&n| n > 0), "{broken:?}");
}

#[test]
fn versioned_id_matches_c() {
    let ids: Vec<Option<&[u8]>> = vec![
        None,
        Some(b""),
        Some(b"x"),
        Some(b"color_source"),
        Some(b"x_v1"),
        Some(b"%s%d%n"),
        Some(b"\xff\x80"),
        "é".as_bytes().into(),
    ];
    let versions = [
        0,
        1,
        2,
        9,
        10,
        0x7fff_fffe,
        0x7fff_ffff,
        0x8000_0000,
        0x8000_0001,
        u32::MAX - 1,
        u32::MAX,
    ];
    for id in &ids {
        let id = id.map(|i| CString::new(i).unwrap());
        for &v in &versions {
            let (rust, oracle) = id_both(&id, v);
            assert_eq!(rust, oracle, "versioned_id({id:?}, {v})");
        }
    }
}

// ------------------------------------------------------ the registration

/// What one side of `obs_register_source_s()` decides.
#[derive(Debug, PartialEq, Eq)]
enum Decision {
    UnknownType,
    TooBig,
    Broken {
        list: u32,
        flags: u32,
        rule: u32,
    },
    Registered {
        list: u32,
        flags: u32,
        id: Option<Vec<u8>>,
    },
}

/// A caller's `obs_source_info`: the values the registration reads.
#[derive(Debug, Clone)]
struct Info {
    source_type: u32,
    output_flags: u32,
    /// `OBS_MODULE_SOURCE_RULES_HAS_*` for the callbacks that are set.
    set: u32,
    version: u32,
}

/// A `u32` at `offset` as the copy keeps it: whole inside the size, or
/// zero. The copy ends at the last field wholly within the size, so a field
/// the size cuts is not partly copied (#2203).
fn copied_u32(value: u32, offset: usize, size: usize) -> u32 {
    if offset + 4 <= size { value } else { 0 }
}

/// `HAS_VAL()`: set and whole inside the size.
fn has_val(info: &Info, size: usize) -> u32 {
    [
        (HAS_GET_NAME, OFF_GET_NAME),
        (HAS_GET_WIDTH, OFF_GET_WIDTH),
        (HAS_GET_HEIGHT, OFF_GET_HEIGHT),
        (HAS_AUDIO_RENDER, OFF_AUDIO_RENDER),
    ]
    .iter()
    .filter(|(bit, off)| info.set & bit != 0 && off + PTR <= size)
    .fold(0, |acc, (bit, _)| acc | bit)
}

type Fns = (
    unsafe extern "C" fn(u32) -> u32,
    unsafe extern "C" fn(usize, usize) -> bool,
    unsafe extern "C" fn(u32, u32) -> u32,
    unsafe extern "C" fn(u32, u32, u32, u32) -> u32,
    unsafe extern "C" fn(*const c_char, u32) -> *mut c_char,
);

const SHIM: Fns = (
    obs_module_source_rules_list_of,
    obs_module_source_rules_size_supported,
    obs_module_source_rules_output_flags,
    obs_module_source_rules_check,
    obs_module_source_rules_versioned_id,
);

const ORACLE: Fns = (
    oracle_obs_module_source_rules_list_of,
    oracle_obs_module_source_rules_size_supported,
    oracle_obs_module_source_rules_output_flags,
    oracle_obs_module_source_rules_check,
    oracle_obs_module_source_rules_versioned_id,
);

/// `obs_register_source_s()` on one side, without the duplicate lookup
/// (C on both sides) and the lists.
fn decide(side: &Fns, info: &Info, size: usize) -> Decision {
    let (list_of, size_supported, output_flags, check, versioned_id) = *side;
    // SAFETY: plain values; the id is NUL-terminated and the result freed.
    unsafe {
        let list = list_of(info.source_type);
        if list == LIST_UNKNOWN {
            return Decision::UnknownType;
        }
        if !size_supported(size, SIZE) {
            return Decision::TooBig;
        }
        let data_type = copied_u32(info.source_type, OFF_TYPE, size);
        let data_flags = copied_u32(info.output_flags, OFF_OUTPUT_FLAGS, size);
        let flags = output_flags(data_type, data_flags);
        let rule = check(
            info.source_type,
            info.output_flags,
            flags,
            has_val(info, size),
        );
        if rule != OK {
            return Decision::Broken { list, flags, rule };
        }
        let version = copied_u32(info.version, OFF_VERSION, size);
        let id = take(versioned_id(c"src".as_ptr(), version));
        Decision::Registered { list, flags, id }
    }
}

#[test]
fn registration_matches_c_at_every_field_end() {
    let mut registered = 0;
    let mut no_name = 0;
    for &source_type in &[0, 1, 2, 3, 4] {
        for output_flags in flag_sets() {
            for set in [0, ALL_CALLBACKS, ALL_CALLBACKS & !HAS_AUDIO_RENDER] {
                let info = Info {
                    source_type,
                    output_flags,
                    set,
                    version: 3,
                };
                for size in field_ends() {
                    let (rust, oracle) = (decide(&SHIM, &info, size), decide(&ORACLE, &info, size));
                    assert_eq!(rust, oracle, "{info:?} size {size}");
                    registered += usize::from(matches!(rust, Decision::Registered { .. }));
                    no_name += usize::from(matches!(
                        rust,
                        Decision::Broken {
                            rule: NO_GET_NAME,
                            ..
                        }
                    ));
                }
            }
        }
    }
    assert!(registered > 100 && no_name > 100, "{registered} {no_name}");
}

/// Sizes around the end of every field the registration reads.
fn field_ends() -> Vec<usize> {
    let mut out = vec![0];
    for end in [
        OFF_TYPE,
        OFF_OUTPUT_FLAGS,
        OFF_GET_NAME,
        OFF_GET_NAME + PTR,
        OFF_GET_WIDTH + PTR,
        OFF_GET_HEIGHT + PTR,
        OFF_AUDIO_RENDER + PTR,
        OFF_VERSION + 4,
        SIZE,
    ] {
        out.extend([end - 1, end, end + 1]);
    }
    out
}

// ------------------------------------------------------ proptests

fn any_type() -> impl Strategy<Value = u32> {
    prop_oneof![0u32..6, any::<u32>()]
}

fn any_flags() -> impl Strategy<Value = u32> {
    prop_oneof![0u32..0x80, any::<u32>()]
}

fn any_size() -> impl Strategy<Value = usize> {
    prop_oneof![
        prop::sample::select(field_ends()),
        0usize..=SIZE + 2,
        any::<usize>(),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    #[test]
    fn list_of_matches_c_for_any_type(t in any::<u32>()) {
        let (rust, oracle) = list_both(t);
        prop_assert_eq!(rust, oracle);
    }

    #[test]
    fn size_supported_matches_c_for_any_size(size in any_size(), supported in any_size()) {
        let (rust, oracle) = size_both(size, supported);
        prop_assert_eq!(rust, oracle);
    }

    #[test]
    fn output_flags_match_c_for_any_flags(t in any_type(), f in any::<u32>()) {
        let (rust, oracle) = flags_both(t, f);
        prop_assert_eq!(rust, oracle);
    }

    #[test]
    fn check_matches_c_for_any_input(
        t in any_type(),
        info in any_flags(),
        flags in any_flags(),
        cb in prop_oneof![0u32..16, any::<u32>()],
    ) {
        let (rust, oracle) = check_both(t, info, flags, cb);
        prop_assert_eq!(rust, oracle);
    }

    #[test]
    fn versioned_id_matches_c_for_any_id(
        id in prop::collection::vec(1u8..=255, 0..16),
        null in any::<bool>(),
        version in prop_oneof![Just(0u32), 0u32..20, any::<u32>()],
    ) {
        let id = (!null).then(|| CString::new(id).unwrap());
        let (rust, oracle) = id_both(&id, version);
        prop_assert_eq!(rust, oracle);
    }

    #[test]
    fn registration_matches_c(
        source_type in any_type(),
        output_flags in any_flags(),
        set in prop_oneof![Just(ALL_CALLBACKS), 0u32..16],
        version in prop_oneof![Just(0u32), 1u32..4, any::<u32>()],
        size in any_size(),
    ) {
        let info = Info { source_type, output_flags, set, version };
        prop_assert_eq!(decide(&SHIM, &info, size), decide(&ORACLE, &info, size));
    }
}
