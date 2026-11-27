//! Tier 3: the original C `obs_source_addref()`, `obs_source_release()`,
//! `obs_source_get_ref()` and `obs_weak_source_get_source()` and the Rust
//! exports, on a noise-filled real-header source and control
//! (oracle/source_refs.c). The original reads the fixture's core as `obs`
//! and its `obs_source_destroy()` is recorded; the Rust side reaches the
//! production adapters, whose core check and destroy the fixture serves the
//! same way. Both drop the source's own weak reference through the Rust
//! `obs_weak_source_release()`. Compares what each call returns, the counts
//! afterwards, every destroy (of which source, and the counts at the
//! first), the log lines and every other byte, over every call, NULL input,
//! core state, missing control (get_ref) or source and a range of counts,
//! plus a proptest; and rejects mutants. No intentional differences.
//! Excluded: a missing control for the other three calls (the original
//! dereferences it) and a weak count that the source's own weak release
//! would free (the fixture is not a libobs allocation; test_source_refs
//! covers the free on the real core).
use core::ffi::{CStr, c_char, c_int, c_long, c_void};
use obs_source::ffi::source_refs::{
    obs_source_addref, obs_source_get_ref, obs_source_release, obs_weak_source_get_source,
};
use obs_source::ffi::source_weak_refs::obs_weak_source_release;
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use obs_util::platform::CLongAtomic;
use proptest::prelude::*;
use std::sync::atomic::Ordering::SeqCst;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct Case {
    call: u8,
    null_input: bool,
    core_alive: bool,
    no_control: bool,
    no_source: bool,
    noise: u8,
    refs: c_long,
    weak_refs: c_long,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Outcome {
    returned: u32,
    refs: c_long,
    weak_refs: c_long,
    destroys: u32,
    destroyed_source: bool,
    refs_at_destroy: c_long,
    weak_refs_at_destroy: c_long,
    log_count: u32,
    log_level: i32,
    log: [c_char; 128],
    untouched: bool,
}

impl Default for Outcome {
    fn default() -> Self {
        Self {
            returned: 0,
            refs: 0,
            weak_refs: 0,
            destroys: 0,
            destroyed_source: false,
            refs_at_destroy: 0,
            weak_refs_at_destroy: 0,
            log_count: 0,
            log_level: 0,
            log: [0; 128],
            untouched: false,
        }
    }
}

impl Outcome {
    fn log(&self) -> &str {
        // SAFETY: the oracle wrote a NUL-terminated string (or left zeros).
        unsafe { CStr::from_ptr(self.log.as_ptr()) }
            .to_str()
            .unwrap()
    }
}

type VoidCall = unsafe extern "C" fn(*mut c_void);
type PtrCall = unsafe extern "C" fn(*mut c_void) -> *mut c_void;
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);

#[repr(C)]
#[derive(Clone, Copy)]
struct Calls {
    addref: Option<VoidCall>,
    release: Option<VoidCall>,
    get_ref: Option<PtrCall>,
    weak_get_source: Option<PtrCall>,
}

const RUST: Calls = Calls {
    addref: Some(obs_source_addref),
    release: Some(obs_source_release),
    get_ref: Some(obs_source_get_ref),
    weak_get_source: Some(obs_weak_source_get_source),
};

unsafe extern "C" {
    fn oracle_source_refs_originals(calls: *mut Calls);
    fn oracle_source_refs_run(
        calls: *const Calls,
        get: GetLog,
        set: SetLog,
        case: *const Case,
        out: *mut Outcome,
    ) -> bool;
}

fn originals() -> Calls {
    let mut calls = Calls {
        addref: None,
        release: None,
        get_ref: None,
        weak_get_source: None,
    };
    // SAFETY: a writable table of four function pointers.
    unsafe { oracle_source_refs_originals(&raw mut calls) };
    calls
}

/// Runs one side; panics when the oracle refused the case or an adapter
/// does not reach its field.
fn run(calls: &Calls, case: &Case) -> Outcome {
    let mut out = Outcome::default();
    // SAFETY: the isolated child owns the log handler; the calls, the case
    // and the outcome are valid for the call.
    let valid = unsafe {
        oracle_source_refs_run(
            calls,
            base_get_log_handler,
            base_set_log_handler,
            case,
            &mut out,
        )
    };
    assert!(
        valid,
        "the oracle refused the case or an adapter failed: {case:?}"
    );
    out
}

/// The original's outcome, when the given calls agree with it.
fn check_with(calls: &Calls, case: &Case) -> Option<Outcome> {
    let original = run(&originals(), case);
    let ported = run(calls, case);
    (ported == original).then_some(original)
}

fn check(case: &Case) -> Outcome {
    check_with(&RUST, case)
        .unwrap_or_else(|| panic!("the Rust exports disagree with the original C: {case:?}"))
}

fn case(call: u8, refs: c_long) -> Case {
    Case {
        call,
        core_alive: true,
        noise: 0xa5,
        refs,
        weak_refs: 1,
        ..Case::default()
    }
}

// --- mutants ---------------------------------------------------------------

unsafe extern "C" {
    fn obs_source_refs_control(source: *mut c_void) -> *mut c_void;
    fn obs_weak_source_refs_field(weak: *mut c_void) -> *const CLongAtomic;
    fn obs_weak_source_source_field(weak: *const c_void) -> *mut c_void;
    fn obs_source_refs_core_alive() -> bool;
    fn obs_source_refs_destroy(source: *mut c_void);
    fn blog(level: c_int, format: *const c_char, ...);
}

fn refs<'a>(control: *mut c_void) -> &'a CLongAtomic {
    // SAFETY: the fixture's control, alive for the run.
    unsafe { &*obs_weak_source_refs_field(control) }
}

fn warn(level: c_int) {
    // SAFETY: static format, no conversions.
    unsafe {
        blog(
            level,
            c"Tried to release a source when the OBS core is shut down!".as_ptr(),
        )
    };
}

#[derive(Clone, Copy)]
enum Release {
    NoCoreCheck,
    WarnAtInfo,
    DestroyAtZero,
    WeakFirst,
    KeepWeak,
    NullChecksCoreFirst,
}

fn mutant_release(change: Release, source: *mut c_void) {
    if source.is_null() {
        if matches!(change, Release::NullChecksCoreFirst) {
            // SAFETY: reads the fixture's core.
            if !unsafe { obs_source_refs_core_alive() } {
                warn(200);
            }
        }
        return;
    }
    // SAFETY: the fixture's live source and control throughout.
    unsafe {
        if !matches!(change, Release::NoCoreCheck) && !obs_source_refs_core_alive() {
            warn(if matches!(change, Release::WarnAtInfo) {
                300
            } else {
                200
            });
            return;
        }
        let control = obs_source_refs_control(source);
        let before = refs(control).fetch_sub(1, SeqCst);
        let last = if matches!(change, Release::DestroyAtZero) {
            before == 1
        } else {
            before == 0
        };
        if last {
            match change {
                Release::WeakFirst => {
                    obs_weak_source_release(control);
                    obs_source_refs_destroy(source);
                }
                Release::KeepWeak => obs_source_refs_destroy(source),
                _ => {
                    obs_source_refs_destroy(source);
                    obs_weak_source_release(control);
                }
            }
        }
    }
}

#[derive(Clone, Copy)]
enum Promote {
    AtMinusOne,
    NoIncrement,
    ReturnsControl,
}

fn mutant_promote(change: Promote, weak: *mut c_void) -> *mut c_void {
    if weak.is_null() {
        return core::ptr::null_mut();
    }
    let count = refs(weak);
    let owners = count.load(SeqCst);
    let ok = match change {
        Promote::AtMinusOne => owners >= -1,
        _ => owners > -1,
    };
    if !ok {
        return core::ptr::null_mut();
    }
    if !matches!(change, Promote::NoIncrement) {
        count.fetch_add(1, SeqCst);
    }
    if matches!(change, Promote::ReturnsControl) {
        weak
    } else {
        // SAFETY: the fixture's control.
        unsafe { obs_weak_source_source_field(weak) }
    }
}

unsafe extern "C" fn addref_decrements(source: *mut c_void) {
    if !source.is_null() {
        // SAFETY: the fixture's live source.
        refs(unsafe { obs_source_refs_control(source) }).fetch_sub(1, SeqCst);
    }
}

unsafe extern "C" fn get_ref_skips_control_check(source: *mut c_void) -> *mut c_void {
    // SAFETY: the fixture's live source; a missing control is passed on.
    let control = unsafe { obs_source_refs_control(source) };
    if control.is_null() {
        // What dereferencing it would have given: anything but NULL.
        return source;
    }
    // SAFETY: as above.
    unsafe { obs_weak_source_get_source(control) }
}

macro_rules! release_mutant {
    ($name:ident, $change:expr) => {
        unsafe extern "C" fn $name(source: *mut c_void) {
            mutant_release($change, source);
        }
    };
}
release_mutant!(r_no_core_check, Release::NoCoreCheck);
release_mutant!(r_warn_at_info, Release::WarnAtInfo);
release_mutant!(r_destroy_at_zero, Release::DestroyAtZero);
release_mutant!(r_weak_first, Release::WeakFirst);
release_mutant!(r_keep_weak, Release::KeepWeak);
release_mutant!(r_null_checks_core, Release::NullChecksCoreFirst);

macro_rules! promote_mutant {
    ($name:ident, $change:expr) => {
        unsafe extern "C" fn $name(weak: *mut c_void) -> *mut c_void {
            mutant_promote($change, weak)
        }
    };
}
promote_mutant!(p_at_minus_one, Promote::AtMinusOne);
promote_mutant!(p_no_increment, Promote::NoIncrement);
promote_mutant!(p_returns_control, Promote::ReturnsControl);

// --- runs -----------------------------------------------------------------

const COUNTS: [c_long; 7] = [-3, -2, -1, 0, 1, 2, 41];

fn grid() -> impl Iterator<Item = Case> {
    let mut cases = Vec::new();
    for call in 0..4u8 {
        for null_input in [false, true] {
            for core_alive in [true, false] {
                for no_control in [false, true] {
                    if no_control && call != 2 {
                        continue;
                    }
                    for no_source in [false, true] {
                        for refs in COUNTS {
                            for weak_refs in [1, 2] {
                                cases.push(Case {
                                    call,
                                    null_input,
                                    core_alive,
                                    no_control,
                                    no_source,
                                    noise: 0x5a,
                                    refs,
                                    weak_refs,
                                });
                            }
                        }
                    }
                }
            }
        }
    }
    cases.into_iter()
}

fn pinned() {
    // The final release: destroy first (refs already -1, the source's weak
    // reference still held), then the weak release.
    let out = check(&case(1, 0));
    assert_eq!((out.refs, out.weak_refs, out.destroys), (-1, 0, 1));
    assert!(out.destroyed_source);
    assert_eq!((out.refs_at_destroy, out.weak_refs_at_destroy), (-1, 1));
    assert_eq!(out.log_count, 0);

    // Not the last owner, or already below: no destroy.
    for refs in [1, -1] {
        let out = check(&case(1, refs));
        assert_eq!((out.refs, out.destroys), (refs - 1, 0));
    }

    // Shut down: one warning, nothing touched.
    let mut c = case(1, 0);
    c.core_alive = false;
    let out = check(&c);
    assert_eq!((out.refs, out.destroys, out.log_count), (0, 0, 1));
    assert_eq!(out.log_level, 200);
    assert_eq!(
        out.log(),
        "Tried to release a source when the OBS core is shut down!"
    );
    c.null_input = true;
    assert_eq!(check(&c).log_count, 0);

    // addref.
    let out = check(&case(0, 2));
    assert_eq!((out.refs, out.returned), (3, 0));

    // Promotions: the source with one more owner, or NULL at -1.
    for call in [2, 3] {
        let out = check(&case(call, 0));
        assert_eq!((out.returned, out.refs), (1, 1));
        let out = check(&case(call, -1));
        assert_eq!((out.returned, out.refs), (0, -1));
        let mut c = case(call, 0);
        c.null_input = true;
        assert_eq!(check(&c).returned, 0);
    }
    let mut c = case(2, 0);
    c.no_control = true;
    let out = check(&c);
    assert_eq!((out.returned, out.refs), (0, 0));
    let mut c = case(3, 0);
    c.no_source = true;
    let out = check(&c);
    assert_eq!((out.returned, out.refs), (0, 1));
}

fn grid_and_mutants() {
    let mut count = 0usize;
    for c in grid() {
        let out = check(&c);
        assert!(out.untouched, "{c:?}");
        count += 1;
    }
    assert_eq!(count, (3 * 2 * 2 * 2 + 2 * 2 * 2 * 2) * COUNTS.len() * 2);

    let release = |f: VoidCall| Calls {
        release: Some(f),
        ..RUST
    };
    let get_ref = |f: PtrCall| Calls {
        get_ref: Some(f),
        ..RUST
    };
    let weak_get = |f: PtrCall| Calls {
        weak_get_source: Some(f),
        ..RUST
    };
    let mutants: [(&str, Calls); 11] = [
        ("release skips the core check", release(r_no_core_check)),
        ("release warns at LOG_INFO", release(r_warn_at_info)),
        ("release destroys at 1 to 0", release(r_destroy_at_zero)),
        ("release drops the weak ref first", release(r_weak_first)),
        ("release keeps the weak ref", release(r_keep_weak)),
        (
            "release(NULL) warns when shut down",
            release(r_null_checks_core),
        ),
        ("promotion at -1", weak_get(p_at_minus_one)),
        ("promotion without the increment", weak_get(p_no_increment)),
        ("promotion returns the control", weak_get(p_returns_control)),
        (
            "addref decrements",
            Calls {
                addref: Some(addref_decrements),
                ..RUST
            },
        ),
        (
            "get_ref skips the control check",
            get_ref(get_ref_skips_control_check),
        ),
    ];
    for (name, calls) in mutants {
        assert!(
            grid().any(|c| check_with(&calls, &c).is_none()),
            "the mutant '{name}' agreed with the original C"
        );
    }
}

fn random_cases() {
    let mut runner = proptest::test_runner::TestRunner::new(ProptestConfig {
        cases: obs_c_oracle::parity::cases(512),
        failure_persistence: None,
        ..ProptestConfig::default()
    });
    runner
        .run(
            &(
                (0u8..4, any::<bool>(), any::<bool>(), any::<bool>()),
                (any::<bool>(), any::<u8>()),
                (-5 as c_long..50, 1 as c_long..5),
            ),
            |(
                (call, null_input, core_alive, no_control),
                (no_source, noise),
                (refs, weak_refs),
            )| {
                let c = Case {
                    call,
                    null_input,
                    core_alive,
                    no_control: no_control && call == 2,
                    no_source,
                    noise,
                    refs,
                    weak_refs,
                };
                let out = check(&c);
                prop_assert!(out.untouched);
                prop_assert!(out.destroys <= 1);
                Ok(())
            },
        )
        .unwrap();
}

#[test]
fn strong_refs_match_c() {
    if !crate::output::isolated(concat!(module_path!(), "::strong_refs_match_c")) {
        return;
    }
    pinned();
    grid_and_mutants();
    random_cases();
}
