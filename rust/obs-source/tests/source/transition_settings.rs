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
//!
//! The `steps_*` cases are `test_source_transition_settings.c` 1:1 (its ten
//! cases, same names, inputs and assertions), run on the Rust shims
//! (`obs_source::ffi::source_transition_settings`). Their transition is the
//! C oracle's test transition, whose fields the shims reach through the C
//! seam as in the hybrid libobs; what `obs_transition_start()`,
//! `obs_transition_set()`, `obs_transition_force_stop()` and
//! `obs_transition_clear()` leave in the fields the settings read (the two
//! transitioning flags, the mode) is written through the seam, and a
//! child's slot is fixed when the transition is made.

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

// ---------------------------------------------------------------------------
// test_source_transition_settings.c, 1:1

mod steps {
    use std::ffi::{CStr, c_int, c_void};
    use std::ptr;

    use obs_c_oracle::source_transition_settings::{
        oracle_transition_settings_free, oracle_transition_settings_new, seam,
    };
    use obs_data::ffi::obs_data::{
        obs_data_create, obs_data_first, obs_data_get_int, obs_data_get_string,
        obs_data_has_user_value, obs_data_item_get_name, obs_data_item_next, obs_data_release,
    };
    use obs_source::ffi::source_transition_settings::{
        obs_transition_enable_fixed, obs_transition_fixed, obs_transition_get_alignment,
        obs_transition_get_scale_type, obs_transition_get_size, obs_transition_is_active,
        obs_transition_save, obs_transition_set_alignment, obs_transition_set_scale_type,
        obs_transition_set_size,
    };

    const INPUT: c_int = 0;
    const TRANSITION: c_int = 2;
    const ALIGN_CENTER: u32 = 0;
    const ALIGN_LEFT: u32 = 1 << 0;
    const ALIGN_RIGHT: u32 = 1 << 1;
    const ALIGN_TOP: u32 = 1 << 2;
    const ALIGN_BOTTOM: u32 = 1 << 3;
    const MODE_AUTO: c_int = 0;
    const MODE_MANUAL: c_int = 1;
    const SCALE_MAX_ONLY: c_int = 0;
    const SCALE_ASPECT: c_int = 1;
    const SCALE_STRETCH: c_int = 2;

    /// A test source, freed on drop. A transition starts as
    /// `obs_transition_init()` leaves it: aligned left and top, the rest 0.
    struct Source(*mut c_void);

    impl Source {
        fn new(source_type: c_int, a: Option<&CStr>, b: Option<&CStr>) -> Self {
            let kind = |n: Option<&CStr>| c_int::from(n.is_some());
            let name = |n: Option<&CStr>| n.map_or(ptr::null(), CStr::as_ptr);
            // SAFETY: the oracle's constructor and the seam on its source.
            unsafe {
                let s =
                    oracle_transition_settings_new(source_type, kind(a), name(a), kind(b), name(b));
                if source_type == TRANSITION {
                    seam::obs_source_transition_seam_set_alignment(s, ALIGN_LEFT | ALIGN_TOP);
                }
                Self(s)
            }
        }

        fn transition(a: Option<&CStr>, b: Option<&CStr>) -> Self {
            Self::new(TRANSITION, a, b)
        }

        /// What `obs_transition_start()` leaves: transitioning, in `mode`.
        fn start(&self, mode: c_int) {
            // SAFETY: a live test source.
            unsafe {
                seam::obs_source_transition_seam_set_mode(self.0, mode);
                seam::obs_source_transition_seam_set_transitioning_video(self.0, true);
                seam::obs_source_transition_seam_set_transitioning_audio(self.0, true);
            }
        }

        /// What `obs_transition_set()` and `obs_transition_clear()` leave:
        /// not transitioning (the mode stays).
        fn end(&self) {
            // SAFETY: a live test source.
            unsafe {
                seam::obs_source_transition_seam_set_transitioning_video(self.0, false);
                seam::obs_source_transition_seam_set_transitioning_audio(self.0, false);
            }
        }
    }

    impl Drop for Source {
        fn drop(&mut self) {
            // SAFETY: made by the oracle, freed once.
            unsafe { oracle_transition_settings_free(self.0) }
        }
    }

    fn assert_size(tr: *const c_void, want_cx: u32, want_cy: u32) {
        let (mut cx, mut cy) = (0xdead_beef_u32, 0xdead_beef_u32);
        // SAFETY: NULL or a live test source, and two outputs.
        unsafe { obs_transition_get_size(tr, &raw mut cx, &raw mut cy) };
        assert_eq!((cx, cy), (want_cx, want_cy));
    }

    /// `obs_transition_save()` into new data, checked as `assert_saved()`.
    fn assert_saved(
        tr: &Source,
        source_a: &CStr,
        alignment: i64,
        mode: i64,
        scale_type: i64,
        cx: i64,
        cy: i64,
    ) {
        // SAFETY: a live transition and new data, released here.
        unsafe {
            let data = obs_data_create();
            obs_transition_save(tr.0, data);
            let mut n = 0;
            let mut item = obs_data_first(data);
            while !item.is_null() {
                if CStr::from_ptr(obs_data_item_get_name(item))
                    .to_bytes()
                    .starts_with(b"transition_")
                {
                    n += 1;
                }
                obs_data_item_next(&raw mut item);
            }
            assert_eq!(n, 6);
            assert!(obs_data_has_user_value(
                data,
                c"transition_source_a".as_ptr()
            ));
            assert_eq!(
                CStr::from_ptr(obs_data_get_string(data, c"transition_source_a".as_ptr())),
                source_a
            );
            let int = |key: &CStr| obs_data_get_int(data, key.as_ptr());
            assert_eq!(int(c"transition_alignment"), alignment);
            assert_eq!(int(c"transition_mode"), mode);
            assert_eq!(int(c"transition_scale_type"), scale_type);
            assert_eq!(int(c"transition_cx"), cx);
            assert_eq!(int(c"transition_cy"), cy);
            obs_data_release(data);
        }
    }

    /// A new transition: aligned left and top, size 0x0, scale type
    /// MAX_ONLY, no fixed duration, not transitioning.
    #[test]
    fn steps_new_transition_defaults() {
        let tr = Source::transition(None, None);
        // SAFETY: a live test transition.
        unsafe {
            assert_eq!(obs_transition_get_alignment(tr.0), ALIGN_LEFT | ALIGN_TOP);
            assert_size(tr.0, 0, 0);
            assert_eq!(obs_transition_get_scale_type(tr.0), SCALE_MAX_ONLY);
            assert!(!obs_transition_fixed(tr.0));
            assert!(!obs_transition_is_active(tr.0));
        }
    }

    #[test]
    fn steps_alignment_after_set() {
        let tr = Source::transition(None, None);
        for value in [
            ALIGN_CENTER,
            ALIGN_RIGHT | ALIGN_BOTTOM,
            ALIGN_LEFT,
            ALIGN_TOP,
            0x5a5a_5a5a,
            u32::MAX,
            ALIGN_CENTER,
        ] {
            // Any value is stored as given, unchecked.
            // SAFETY: a live test transition.
            unsafe {
                obs_transition_set_alignment(tr.0, value);
                assert_eq!(obs_transition_get_alignment(tr.0), value);
            }
        }
    }

    #[test]
    fn steps_size_after_set() {
        let tr = Source::transition(None, None);
        for (cx, cy) in [(1920, 1080), (0, 77), (u32::MAX, 1), (0, 0)] {
            // SAFETY: a live test transition.
            unsafe { obs_transition_set_size(tr.0, cx, cy) };
            assert_size(tr.0, cx, cy);
        }
    }

    #[test]
    fn steps_scale_type_after_set() {
        let tr = Source::transition(None, None);
        // SAFETY: a live test transition.
        unsafe {
            for t in [SCALE_STRETCH, SCALE_ASPECT, SCALE_MAX_ONLY] {
                obs_transition_set_scale_type(tr.0, t);
                assert_eq!(obs_transition_get_scale_type(tr.0), t);
            }
            // Out of range is stored as MAX_ONLY, from any earlier value.
            for t in [SCALE_STRETCH + 1, -1, i32::MAX, i32::MIN] {
                obs_transition_set_scale_type(tr.0, SCALE_STRETCH);
                obs_transition_set_scale_type(tr.0, t);
                assert_eq!(obs_transition_get_scale_type(tr.0), SCALE_MAX_ONLY);
            }
        }
    }

    #[test]
    fn steps_fixed_after_enable() {
        let tr = Source::transition(None, None);
        for (enable, duration) in [(true, 300), (false, 300), (true, 0), (false, 0)] {
            // SAFETY: a live test transition.
            unsafe {
                obs_transition_enable_fixed(tr.0, enable, duration);
                assert_eq!(obs_transition_fixed(tr.0), enable);
            }
        }
    }

    /// Transitioning from the start until set or clear ends it; a forced
    /// stop leaves it transitioning.
    #[test]
    fn steps_is_active_follows_the_transition() {
        let tr = Source::transition(Some(c"is active A"), Some(c"is active B"));
        // SAFETY: a live test transition.
        let active = || unsafe { obs_transition_is_active(tr.0) };
        tr.end(); // obs_transition_set(tr, a)
        assert!(!active());
        tr.start(MODE_AUTO);
        assert!(active());
        // obs_transition_force_stop(): still transitioning
        assert!(active());
        tr.end(); // obs_transition_set(tr, b)
        assert!(!active());
        tr.start(MODE_AUTO);
        assert!(active());
        tr.end(); // obs_transition_clear()
        assert!(!active());
    }

    /// NULL and a source that is not a transition: every setter does nothing
    /// and every getter returns its default; the size getter zeroes both.
    #[test]
    fn steps_not_a_transition_defaults() {
        let input = Source::new(INPUT, None, None);
        for s in [ptr::null_mut(), input.0] {
            // SAFETY: NULL or a live test source; these eight check it.
            unsafe {
                obs_transition_set_scale_type(s, SCALE_STRETCH);
                obs_transition_set_alignment(s, ALIGN_CENTER);
                obs_transition_set_size(s, 640, 480);
                obs_transition_enable_fixed(s, true, 300);

                assert_eq!(obs_transition_get_scale_type(s), SCALE_MAX_ONLY);
                assert_eq!(obs_transition_get_alignment(s), 0);
                assert_size(s, 0, 0);
                assert!(!obs_transition_fixed(s));
            }
        }
        // No check: it reads the flags of any source, false for an input.
        // SAFETY: a live test source.
        assert!(!unsafe { obs_transition_is_active(input.0) });
    }

    /// With no child, the name is written as "", not left out.
    #[test]
    fn steps_save_with_no_child() {
        let tr = Source::transition(None, None);
        assert_saved(
            &tr,
            c"",
            i64::from(ALIGN_LEFT | ALIGN_TOP),
            i64::from(MODE_AUTO),
            i64::from(SCALE_MAX_ONLY),
            0,
            0,
        );
    }

    #[test]
    fn steps_save_writes_the_settings() {
        let tr = Source::transition(Some(c"save A"), None);
        // SAFETY: a live test transition.
        unsafe {
            obs_transition_set_alignment(tr.0, ALIGN_RIGHT | ALIGN_BOTTOM);
            obs_transition_set_scale_type(tr.0, SCALE_STRETCH);
            obs_transition_set_size(tr.0, 1280, 720);
            // Not saved.
            obs_transition_enable_fixed(tr.0, true, 300);
        }
        assert_saved(
            &tr,
            c"save A",
            i64::from(ALIGN_RIGHT | ALIGN_BOTTOM),
            i64::from(MODE_AUTO),
            i64::from(SCALE_STRETCH),
            1280,
            720,
        );
        // Each save reads the fields again.
        // SAFETY: a live test transition.
        unsafe {
            obs_transition_set_alignment(tr.0, u32::MAX);
            obs_transition_set_size(tr.0, u32::MAX, 0);
        }
        assert_saved(
            &tr,
            c"save A",
            i64::from(u32::MAX),
            i64::from(MODE_AUTO),
            i64::from(SCALE_STRETCH),
            i64::from(u32::MAX),
            0,
        );
    }

    /// While transitioning, the saved source is slot B and the saved mode
    /// the running transition's.
    #[test]
    fn steps_save_while_transitioning() {
        let tr = Source::transition(Some(c"running A"), Some(c"running B"));
        // SAFETY: a live test transition.
        unsafe { obs_transition_set_size(tr.0, 64, 32) };
        tr.start(MODE_MANUAL);
        let left_top = i64::from(ALIGN_LEFT | ALIGN_TOP);
        let manual = i64::from(MODE_MANUAL);
        let max_only = i64::from(SCALE_MAX_ONLY);
        assert_saved(&tr, c"running B", left_top, manual, max_only, 64, 32);
        // A forced stop leaves it transitioning: still slot B.
        assert_saved(&tr, c"running B", left_top, manual, max_only, 64, 32);
        // Once set ends it, slot A again; the mode stays.
        tr.end();
        // SAFETY: a live test transition.
        assert!(!unsafe { obs_transition_is_active(tr.0) });
        assert_saved(&tr, c"running A", left_top, manual, max_only, 64, 32);
    }
}
