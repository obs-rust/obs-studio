//! Tier 1 tests for `obs_source::transition_settings`, the safe core of
//! `libobs/obs-source-transition-settings.c.h`.
//!
//! `test_source_transition_settings.c` pins the same rules through the real
//! libobs: the getters' answers for NULL and for a source that is not a
//! transition, and every key and value `obs_transition_save()` writes. Here
//! they are checked on plain values: the log name of each function (the
//! one intentional difference: the size getter logs under its own name),
//! the invalid-source defaults and the save record.

use std::ffi::CStr;

use obs_source::transition_settings::{
    Func, SCALE_MAX_ONLY, Saved, Settings, Value, log_name, save,
};
use proptest::prelude::*;

const ALL: [Func; 8] = [
    Func::SetScaleType,
    Func::GetScaleType,
    Func::SetAlignment,
    Func::GetAlignment,
    Func::SetSize,
    Func::GetSize,
    Func::EnableFixed,
    Func::Fixed,
];

const KEYS: [&CStr; 6] = [
    c"transition_source_a",
    c"transition_alignment",
    c"transition_mode",
    c"transition_scale_type",
    c"transition_cx",
    c"transition_cy",
];

fn saved(
    source_a: Option<&CStr>,
    alignment: u32,
    mode: i32,
    scale_type: i32,
    cx: u32,
    cy: u32,
) -> Saved<'_> {
    Saved {
        source_a,
        alignment,
        mode,
        scale_type,
        cx,
        cy,
    }
}

/// Each function names itself, as the C's literals do.
#[test]
fn names_match_the_c_literals() {
    assert_eq!(
        log_name(Func::SetScaleType),
        c"obs_transition_set_scale_type"
    );
    assert_eq!(
        log_name(Func::GetScaleType),
        c"obs_transition_get_scale_type"
    );
    assert_eq!(
        log_name(Func::SetAlignment),
        c"obs_transition_set_alignment"
    );
    assert_eq!(
        log_name(Func::GetAlignment),
        c"obs_transition_get_alignment"
    );
    assert_eq!(log_name(Func::SetSize), c"obs_transition_set_size");
    assert_eq!(log_name(Func::EnableFixed), c"obs_transition_enable_fixed");
    assert_eq!(log_name(Func::Fixed), c"obs_transition_fixed");
}

/// The size getter logs as `obs_transition_get_size`; the C names the setter
/// (intentional difference).
#[test]
fn get_size_logs_its_own_name() {
    assert_eq!(log_name(Func::GetSize), c"obs_transition_get_size");
    assert_ne!(log_name(Func::GetSize), log_name(Func::SetSize));
}

/// No two functions share a name.
#[test]
fn names_are_distinct() {
    for (i, a) in ALL.iter().enumerate() {
        for b in &ALL[i + 1..] {
            assert_ne!(log_name(*a), log_name(*b), "{a:?} and {b:?}");
        }
    }
}

/// test_source_transition_settings.c's invalid-source defaults.
#[test]
fn invalid_defaults() {
    assert_eq!(SCALE_MAX_ONLY, 0);
    assert_eq!(
        Settings::INVALID,
        Settings {
            scale_type: SCALE_MAX_ONLY,
            alignment: 0,
            cx: 0,
            cy: 0,
            use_fixed_duration: false,
        }
    );
}

/// The six keys, in the C's order.
#[test]
fn save_keys_in_order() {
    let rec = save(saved(Some(c"a"), 1, 2, 3, 4, 5));
    let keys: Vec<&CStr> = rec.iter().map(|(k, _)| *k).collect();
    assert_eq!(keys, KEYS);
}

/// An empty active slot saves `""`; the integers are the fields.
#[test]
fn save_empty_slot() {
    let rec = save(saved(None, 0, 0, 0, 0, 0));
    assert_eq!(rec[0], (KEYS[0], Value::Str(c"")));
    for (i, (key, value)) in rec.iter().enumerate().skip(1) {
        assert_eq!(*key, KEYS[i]);
        assert_eq!(*value, Value::Int(0));
    }
}

/// The child's name, and test_source_transition_settings.c's saved values.
#[test]
fn save_child_and_values() {
    let rec = save(saved(Some(c"settings child"), 5, 1, 2, 1280, 720));
    assert_eq!(
        rec,
        [
            (KEYS[0], Value::Str(c"settings child")),
            (KEYS[1], Value::Int(5)),
            (KEYS[2], Value::Int(1)),
            (KEYS[3], Value::Int(2)),
            (KEYS[4], Value::Int(1280)),
            (KEYS[5], Value::Int(720)),
        ]
    );
}

/// The `uint32_t` fields are zero-extended: `UINT32_MAX` saves 4294967295.
#[test]
fn save_zero_extends_the_sizes() {
    let rec = save(saved(None, u32::MAX, 0, 0, u32::MAX, 0));
    assert_eq!(rec[1].1, Value::Int(4_294_967_295));
    assert_eq!(rec[4].1, Value::Int(4_294_967_295));
    assert_eq!(rec[5].1, Value::Int(0));
}

/// The enums are sign-extended: a mode or scale type of -1 saves -1.
#[test]
fn save_sign_extends_the_enums() {
    let rec = save(saved(None, 0, -1, i32::MIN, 0, 0));
    assert_eq!(rec[2].1, Value::Int(-1));
    assert_eq!(rec[3].1, Value::Int(i64::from(i32::MIN)));
}

proptest! {
    /// Any record saves its own fields under the six keys.
    #[test]
    fn save_any_record(
        named in any::<bool>(),
        alignment in any::<u32>(),
        mode in any::<i32>(),
        scale_type in any::<i32>(),
        cx in any::<u32>(),
        cy in any::<u32>(),
    ) {
        let name = c"child";
        let rec = save(saved(named.then_some(name), alignment, mode, scale_type, cx, cy));
        prop_assert_eq!(
            rec,
            [
                (KEYS[0], Value::Str(if named { name } else { c"" })),
                (KEYS[1], Value::Int(i64::from(alignment))),
                (KEYS[2], Value::Int(i64::from(mode))),
                (KEYS[3], Value::Int(i64::from(scale_type))),
                (KEYS[4], Value::Int(i64::from(cx))),
                (KEYS[5], Value::Int(i64::from(cy))),
            ]
        );
    }
}
