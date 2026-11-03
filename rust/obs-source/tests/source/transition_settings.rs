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
//! cases, same names, inputs and assertions), run on the safe functions
//! (`is_active`, the setters and getters, `save_to`) over a plain Rust
//! [`Transition`]: a transition starts as `obs_transition_init()` leaves it,
//! and what `obs_transition_start()`, `obs_transition_set()`,
//! `obs_transition_force_stop()` and `obs_transition_clear()` leave in the
//! fields the settings read (the two transitioning flags, the mode, the
//! children) is set on it directly. The `obs_data` the save writes is a list
//! of keys and values. The fake also records every `transition_valid()`
//! name and lock call, so the checks, log names and locking are asserted
//! too.

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
// test_source_transition_settings.c, 1:1, on the safe API

mod steps {
    use std::cell::RefCell;
    use std::ffi::{CStr, CString};

    use obs_source::transition_settings::{
        SaveData, Transition, enable_fixed, fixed, get_alignment, get_scale_type, get_size,
        is_active, save_to, set_alignment, set_scale_type, set_size,
    };

    const ALIGN_CENTER: u32 = 0;
    const ALIGN_LEFT: u32 = 1 << 0;
    const ALIGN_RIGHT: u32 = 1 << 1;
    const ALIGN_TOP: u32 = 1 << 2;
    const ALIGN_BOTTOM: u32 = 1 << 3;
    const MODE_AUTO: i32 = 0;
    const MODE_MANUAL: i32 = 1;
    const SCALE_MAX_ONLY: i32 = 0;
    const SCALE_ASPECT: i32 = 1;
    const SCALE_STRETCH: i32 = 2;

    thread_local! {
        /// The names `transition_valid()` was asked with, in order.
        static CHECKS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    }

    fn take_checks() -> Vec<String> {
        CHECKS.with(|c| c.take())
    }

    /// A transition's fields, as `obs_transition_init()` leaves them (aligned
    /// left and top, the rest 0), or a source that is not a transition.
    struct Fake {
        is_transition: bool,
        audio: bool,
        video: bool,
        mode: i32,
        scale_type: i32,
        alignment: u32,
        cx: u32,
        cy: u32,
        use_fixed: bool,
        duration: u32,
        /// The two slots: empty, or a child and its name.
        slots: [Option<Option<CString>>; 2],
        /// "lock" and "unlock", in order.
        locks: RefCell<Vec<&'static str>>,
    }

    impl Fake {
        fn transition() -> Self {
            Self {
                is_transition: true,
                audio: false,
                video: false,
                mode: MODE_AUTO,
                scale_type: SCALE_MAX_ONLY,
                alignment: ALIGN_LEFT | ALIGN_TOP,
                cx: 0,
                cy: 0,
                use_fixed: false,
                duration: 0,
                slots: [None, None],
                locks: RefCell::new(Vec::new()),
            }
        }

        fn input() -> Self {
            Self {
                is_transition: false,
                alignment: 0,
                ..Self::transition()
            }
        }

        fn child(name: &str) -> Option<Option<CString>> {
            Some(Some(CString::new(name).unwrap()))
        }

        /// What `obs_transition_set(tr, child)` leaves: `child` in slot A,
        /// not transitioning (the mode stays).
        fn set(&mut self, child: &str) {
            self.slots = [Self::child(child), None];
            self.audio = false;
            self.video = false;
        }

        /// What `obs_transition_start(tr, mode, duration, dest)` leaves:
        /// `dest` in slot B, transitioning in `mode`.
        fn start(&mut self, mode: i32, dest: &str) {
            self.slots[1] = Self::child(dest);
            self.mode = mode;
            self.audio = true;
            self.video = true;
        }

        /// What `obs_transition_clear()` leaves: both slots empty, not
        /// transitioning.
        fn clear(&mut self) {
            self.slots = [None, None];
            self.audio = false;
            self.video = false;
        }
    }

    impl Transition for Fake {
        type Child = usize;

        fn valid(source: Option<&Self>, func: &'static CStr) -> bool {
            CHECKS.with(|c| c.borrow_mut().push(func.to_str().unwrap().to_owned()));
            source.is_some_and(|s| s.is_transition)
        }
        fn active(&self) -> bool {
            self.audio || self.video
        }
        fn source(&self, idx: usize) -> Option<usize> {
            self.slots[idx].as_ref().map(|_| idx)
        }
        fn child_name(&self, child: usize) -> Option<&CStr> {
            self.slots[child].as_ref().unwrap().as_deref()
        }
        fn mode(&self) -> i32 {
            self.mode
        }
        fn scale_type(&self) -> i32 {
            self.scale_type
        }
        fn set_scale_type(&mut self, value: i32) {
            self.scale_type = value;
        }
        fn alignment(&self) -> u32 {
            self.alignment
        }
        fn set_alignment(&mut self, value: u32) {
            self.alignment = value;
        }
        fn cx(&self) -> u32 {
            self.cx
        }
        fn set_cx(&mut self, value: u32) {
            self.cx = value;
        }
        fn cy(&self) -> u32 {
            self.cy
        }
        fn set_cy(&mut self, value: u32) {
            self.cy = value;
        }
        fn use_fixed_duration(&self) -> bool {
            self.use_fixed
        }
        fn set_use_fixed_duration(&mut self, value: bool) {
            self.use_fixed = value;
        }
        fn set_fixed_duration(&mut self, value: u32) {
            self.duration = value;
        }
        fn lock(&self) {
            self.locks.borrow_mut().push("lock");
        }
        fn unlock(&self) {
            self.locks.borrow_mut().push("unlock");
        }
    }

    /// What `obs_transition_save()` wrote: a string (`None` for NULL) or an
    /// integer per key, in order.
    #[derive(Debug, Default)]
    struct Data(Vec<(String, Result<Option<String>, i64>)>);

    impl SaveData for Data {
        fn set_string(&mut self, key: &'static CStr, value: Option<&CStr>) {
            let value = value.map(|v| v.to_str().unwrap().to_owned());
            self.0.push((key.to_str().unwrap().to_owned(), Ok(value)));
        }
        fn set_int(&mut self, key: &'static CStr, value: i64) {
            self.0.push((key.to_str().unwrap().to_owned(), Err(value)));
        }
    }

    fn assert_size(tr: Option<&Fake>, want_cx: u32, want_cy: u32) {
        assert_eq!(get_size(tr), (want_cx, want_cy));
    }

    /// `obs_transition_save()` into new data, checked as `assert_saved()`:
    /// the six keys and their values, no other key, under one lock.
    fn assert_saved(
        tr: &Fake,
        source_a: &str,
        alignment: i64,
        mode: i64,
        scale_type: i64,
        cx: i64,
        cy: i64,
    ) {
        let mut data = Data::default();
        save_to(tr, &mut data);
        assert_eq!(tr.locks.take(), ["lock", "unlock"]);
        let transition_keys = data
            .0
            .iter()
            .filter(|(k, _)| k.starts_with("transition_"))
            .count();
        assert_eq!(transition_keys, 6);
        let get = |key: &str| {
            data.0
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.clone())
                .unwrap_or_else(|| panic!("{key} not written"))
        };
        assert_eq!(get("transition_source_a"), Ok(Some(source_a.to_owned())));
        assert_eq!(get("transition_alignment"), Err(alignment));
        assert_eq!(get("transition_mode"), Err(mode));
        assert_eq!(get("transition_scale_type"), Err(scale_type));
        assert_eq!(get("transition_cx"), Err(cx));
        assert_eq!(get("transition_cy"), Err(cy));
    }

    /// A new transition: aligned left and top, size 0x0, scale type
    /// MAX_ONLY, no fixed duration, not transitioning.
    #[test]
    fn steps_new_transition_defaults() {
        let tr = Fake::transition();
        assert_eq!(get_alignment(Some(&tr)), ALIGN_LEFT | ALIGN_TOP);
        assert_size(Some(&tr), 0, 0);
        assert_eq!(get_scale_type(Some(&tr)), SCALE_MAX_ONLY);
        assert!(!fixed(Some(&tr)));
        assert!(!is_active(&tr));
    }

    #[test]
    fn steps_alignment_after_set() {
        let mut tr = Fake::transition();
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
            set_alignment(Some(&mut tr), value);
            assert_eq!(get_alignment(Some(&tr)), value);
        }
    }

    #[test]
    fn steps_size_after_set() {
        let mut tr = Fake::transition();
        for (cx, cy) in [(1920, 1080), (0, 77), (u32::MAX, 1), (0, 0)] {
            set_size(Some(&mut tr), cx, cy);
            assert_size(Some(&tr), cx, cy);
        }
    }

    #[test]
    fn steps_scale_type_after_set() {
        let mut tr = Fake::transition();
        for t in [SCALE_STRETCH, SCALE_ASPECT, SCALE_MAX_ONLY] {
            set_scale_type(Some(&mut tr), t);
            assert_eq!(get_scale_type(Some(&tr)), t);
        }
        // Out of range is stored as MAX_ONLY, from any earlier value.
        for t in [SCALE_STRETCH + 1, -1, i32::MAX, i32::MIN] {
            set_scale_type(Some(&mut tr), SCALE_STRETCH);
            set_scale_type(Some(&mut tr), t);
            assert_eq!(get_scale_type(Some(&tr)), SCALE_MAX_ONLY);
        }
    }

    #[test]
    fn steps_fixed_after_enable() {
        let mut tr = Fake::transition();
        for (enable, duration) in [(true, 300), (false, 300), (true, 0), (false, 0)] {
            enable_fixed(Some(&mut tr), enable, duration);
            assert_eq!(fixed(Some(&tr)), enable);
            assert_eq!(tr.duration, duration);
        }
    }

    /// Transitioning from the start until set or clear ends it; a forced
    /// stop leaves it transitioning.
    #[test]
    fn steps_is_active_follows_the_transition() {
        let mut tr = Fake::transition();
        tr.set("is active A");
        assert!(!is_active(&tr));
        tr.start(MODE_AUTO, "is active B");
        assert!(is_active(&tr));
        // obs_transition_force_stop() only signals the stop
        assert!(is_active(&tr));
        tr.set("is active B");
        assert!(!is_active(&tr));
        tr.start(MODE_AUTO, "is active A");
        assert!(is_active(&tr));
        tr.clear();
        assert!(!is_active(&tr));
    }

    /// NULL and a source that is not a transition: every setter does nothing
    /// and every getter returns its default; the size getter zeroes both.
    #[test]
    fn steps_not_a_transition_defaults() {
        let mut input = Fake::input();
        for is_null in [true, false] {
            macro_rules! s {
                () => {
                    if is_null { None } else { Some(&mut input) }
                };
            }
            set_scale_type(s!(), SCALE_STRETCH);
            set_alignment(s!(), ALIGN_CENTER);
            set_size(s!(), 640, 480);
            enable_fixed(s!(), true, 300);
            let s = if is_null { None } else { Some(&input) };
            assert_eq!(get_scale_type(s), SCALE_MAX_ONLY);
            assert_eq!(get_alignment(s), 0);
            assert_size(s, 0, 0);
            assert!(!fixed(s));
        }
        // Nothing was written to the input.
        assert_eq!(
            (
                input.scale_type,
                input.cx,
                input.cy,
                input.use_fixed,
                input.duration
            ),
            (SCALE_MAX_ONLY, 0, 0, false, 0)
        );
        // obs_transition_is_active() has no check: it reads the flags of any
        // source, which are false for an input.
        assert!(!is_active(&input));
    }

    /// With no child, the name is written as "", not left out.
    #[test]
    fn steps_save_with_no_child() {
        let tr = Fake::transition();
        assert_saved(
            &tr,
            "",
            i64::from(ALIGN_LEFT | ALIGN_TOP),
            i64::from(MODE_AUTO),
            i64::from(SCALE_MAX_ONLY),
            0,
            0,
        );
    }

    #[test]
    fn steps_save_writes_the_settings() {
        let mut tr = Fake::transition();
        set_alignment(Some(&mut tr), ALIGN_RIGHT | ALIGN_BOTTOM);
        set_scale_type(Some(&mut tr), SCALE_STRETCH);
        set_size(Some(&mut tr), 1280, 720);
        // Not saved.
        enable_fixed(Some(&mut tr), true, 300);
        tr.set("save A");
        assert_saved(
            &tr,
            "save A",
            i64::from(ALIGN_RIGHT | ALIGN_BOTTOM),
            i64::from(MODE_AUTO),
            i64::from(SCALE_STRETCH),
            1280,
            720,
        );
        // Each save reads the fields again.
        set_alignment(Some(&mut tr), u32::MAX);
        set_size(Some(&mut tr), u32::MAX, 0);
        assert_saved(
            &tr,
            "save A",
            i64::from(u32::MAX),
            i64::from(MODE_AUTO),
            i64::from(SCALE_STRETCH),
            i64::from(u32::MAX),
            0,
        );
    }

    /// While transitioning, the saved source is slot B (the destination) and
    /// the saved mode the running transition's.
    #[test]
    fn steps_save_while_transitioning() {
        let mut tr = Fake::transition();
        set_size(Some(&mut tr), 64, 32);
        tr.set("running A");
        tr.start(MODE_MANUAL, "running B");
        let left_top = i64::from(ALIGN_LEFT | ALIGN_TOP);
        let manual = i64::from(MODE_MANUAL);
        let max_only = i64::from(SCALE_MAX_ONLY);
        assert_saved(&tr, "running B", left_top, manual, max_only, 64, 32);
        // obs_transition_force_stop() leaves it transitioning: still slot B.
        assert_saved(&tr, "running B", left_top, manual, max_only, 64, 32);
        // Once obs_transition_set() ends it, slot A again; the mode stays.
        tr.set("running A");
        assert!(!is_active(&tr));
        assert_saved(&tr, "running A", left_top, manual, max_only, 64, 32);
    }

    // -----------------------------------------------------------------------
    // the checks and log names, the NULL name and the reads of the save

    /// Each checking function asks `transition_valid()` once, under its own
    /// name (the size getter too: the intentional difference), and the two
    /// that do not check never ask.
    #[test]
    fn each_function_checks_under_its_name() {
        let mut tr = Fake::transition();
        take_checks();
        set_scale_type(Some(&mut tr), 1);
        let _ = get_scale_type(Some(&tr));
        set_alignment(Some(&mut tr), 1);
        let _ = get_alignment(Some(&tr));
        set_size(None::<&mut Fake>, 1, 1);
        let _ = get_size(None::<&Fake>);
        enable_fixed(Some(&mut tr), true, 1);
        let _ = fixed(Some(&tr));
        let _ = is_active(&tr);
        save_to(&tr, &mut Data::default());
        assert_eq!(
            take_checks(),
            [
                "obs_transition_set_scale_type",
                "obs_transition_get_scale_type",
                "obs_transition_set_alignment",
                "obs_transition_get_alignment",
                "obs_transition_set_size",
                "obs_transition_get_size",
                "obs_transition_enable_fixed",
                "obs_transition_fixed",
            ]
        );
    }

    /// A child whose name is NULL is saved as NULL, not "".
    #[test]
    fn save_passes_a_null_child_name() {
        let mut tr = Fake::transition();
        tr.slots[0] = Some(None);
        let mut data = Data::default();
        save_to(&tr, &mut data);
        assert_eq!(data.0[0], ("transition_source_a".to_owned(), Ok(None)));
    }

    /// The save writes in the C's order, and a stored -1 mode widens as the
    /// host's C does.
    #[test]
    fn save_order_and_enum_widening() {
        let mut tr = Fake::transition();
        tr.mode = -1;
        let mut data = Data::default();
        save_to(&tr, &mut data);
        let keys: Vec<&str> = data.0.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(
            keys,
            [
                "transition_source_a",
                "transition_alignment",
                "transition_mode",
                "transition_scale_type",
                "transition_cx",
                "transition_cy",
            ]
        );
        assert_eq!(data.0[2].1, Err(super::enum_to_i64(-1)));
    }
}
