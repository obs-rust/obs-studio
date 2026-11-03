//! Tier 1 tests for `obs_source::transition_settings`, the safe core of
//! `libobs/obs-source-transition-settings.c.h`.
//!
//! `test_source_transition_settings.c` pins the same rules through the real
//! libobs: the getters' answers for NULL and for a source that is not a
//! transition, and every key and value `obs_transition_save()` writes. Here
//! they are checked on plain values: the log name of each function (the
//! one intentional difference: the size getter logs under its own name),
//! the invalid-source defaults, the save record and its order of reads and
//! writes, and how the host widens the enums.

use std::ffi::CStr;

use obs_source::transition_settings::{
    Child, ENUMS_ARE_UNSIGNED, Func, SCALE_MAX_ONLY, SaveReads, Saved, Settings, Value,
    enum_to_i64, log_name, save, save_each,
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

/// `(int64_t)` of an enum field: GCC and Clang make these enums `unsigned
/// int`, so a stored -1 saves 4294967295; MSVC's `int` enums save -1.
#[test]
fn enum_widening_follows_the_host() {
    let (minus_one, min) = if ENUMS_ARE_UNSIGNED {
        (4_294_967_295, 2_147_483_648)
    } else {
        (-1, i64::from(i32::MIN))
    };
    assert_eq!(enum_to_i64(-1), minus_one);
    assert_eq!(enum_to_i64(i32::MIN), min);
    assert_eq!(enum_to_i64(0), 0);
    assert_eq!(enum_to_i64(2), 2);
    assert_eq!(enum_to_i64(i32::MAX), i64::from(i32::MAX));
    let rec = save(saved(None, 0, -1, i32::MIN, 0, 0));
    assert_eq!(rec[2].1, Value::Int(minus_one));
    assert_eq!(rec[3].1, Value::Int(min));
}

/// Each value is read just before it is written, in the C's order, and a
/// child with a NULL name is written as NULL.
#[test]
fn save_each_reads_before_each_write() {
    use std::cell::RefCell;

    struct Reads<'t>(&'t RefCell<Vec<String>>);
    impl Reads<'_> {
        fn note(&self, what: &str) {
            self.0.borrow_mut().push(format!("read {what}"));
        }
    }
    impl SaveReads<'static> for Reads<'_> {
        fn source_a(&mut self) -> Child<'static> {
            self.note("source_a");
            Child::Unnamed
        }
        fn alignment(&mut self) -> u32 {
            self.note("alignment");
            1
        }
        fn mode(&mut self) -> i32 {
            self.note("mode");
            2
        }
        fn scale_type(&mut self) -> i32 {
            self.note("scale_type");
            3
        }
        fn cx(&mut self) -> u32 {
            self.note("cx");
            4
        }
        fn cy(&mut self) -> u32 {
            self.note("cy");
            5
        }
    }

    let trace = RefCell::new(Vec::new());
    let mut reads = Reads(&trace);
    save_each(&mut reads, |key, value| {
        trace
            .borrow_mut()
            .push(format!("write {} {value:?}", key.to_str().unwrap()));
    });
    assert_eq!(
        trace.into_inner(),
        [
            "read source_a",
            "write transition_source_a Null",
            "read alignment",
            "write transition_alignment Int(1)",
            "read mode",
            "write transition_mode Int(2)",
            "read scale_type",
            "write transition_scale_type Int(3)",
            "read cx",
            "write transition_cx Int(4)",
            "read cy",
            "write transition_cy Int(5)",
        ]
    );
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
                (KEYS[2], Value::Int(enum_to_i64(mode))),
                (KEYS[3], Value::Int(enum_to_i64(scale_type))),
                (KEYS[4], Value::Int(i64::from(cx))),
                (KEYS[5], Value::Int(i64::from(cy))),
            ]
        );
    }
}
