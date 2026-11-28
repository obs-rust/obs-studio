//! Tier 3: the original C per-source callback steps of
//! `obs_source_video_tick()` (`obs_source_deferred_update()`,
//! `activate_source()`, `deactivate_source()`, `show_source()`,
//! `hide_source()`, `source_video_tick_callbacks.c`) against the Rust
//! exports, each on its own real-layout source with recording type
//! callbacks and real source and core signal handlers under a private core.
//! Both run over the same production adapters. Every call's events in
//! order (callbacks, global and source signals), the identity of every
//! argument a callback or handler saw, and the deferred count after it
//! must match, over every combination of data, callbacks and privacy with
//! edge counts and an update deferred again from inside the callback, the
//! characterized sequence, and random call sequences. Six mutants of the
//! exports, built on the same adapters, must not.
//! No intentional differences or exclusions.
use core::ffi::{CStr, c_char, c_int, c_long, c_void};
use core::sync::atomic::Ordering::SeqCst;
use obs_source::ffi::source_video_tick_callbacks as rust;
use obs_util::platform::CLongAtomic;
use proptest::prelude::*;

type StepFn = unsafe extern "C" fn(*mut c_void);

unsafe extern "C" {
    fn oracle_obs_source_deferred_update(source: *mut c_void);
    fn oracle_activate_source(source: *mut c_void);
    fn oracle_deactivate_source(source: *mut c_void);
    fn oracle_show_source(source: *mut c_void);
    fn oracle_hide_source(source: *mut c_void);
    fn oracle_vtc_new(bits: u32, defer: c_long, redefer: c_long) -> *mut c_void;
    fn oracle_vtc_run(fixture: *mut c_void, step: StepFn);
    fn oracle_vtc_record(
        fixture: *const c_void,
        identity: *mut bool,
        defer: *mut c_long,
    ) -> *const c_char;
    fn oracle_vtc_free(fixture: *mut c_void);

    // The production adapters, for the mutants.
    fn obs_source_tick_callbacks_can_update(source: *const c_void) -> bool;
    fn obs_source_tick_callbacks_defer_count(source: *mut c_void) -> *const CLongAtomic;
    fn obs_source_tick_callbacks_update(source: *mut c_void);
    fn obs_source_tick_callbacks_has(source: *const c_void, step: c_int) -> bool;
    fn obs_source_tick_callbacks_call(source: *mut c_void, step: c_int);
    fn obs_source_tick_callbacks_signal(
        source: *mut c_void,
        signal_obs: *const c_char,
        signal_source: *const c_char,
    );
}

/// Which of the five steps a call runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Which {
    Update,
    Activate,
    Deactivate,
    Show,
    Hide,
}

const WHICH: [Which; 5] = [
    Which::Update,
    Which::Activate,
    Which::Deactivate,
    Which::Show,
    Which::Hide,
];

/// Data, update, activate, deactivate, show, hide, private.
const BITS: u32 = 1 << 7;

#[derive(Debug, Clone)]
struct Case {
    bits: u32,
    defer: c_long,
    /// Added to the count from inside each update callback.
    redefer: c_long,
    calls: Vec<Which>,
}

/// After each call: every event so far, whether every argument seen was
/// the fixture's own, and the deferred count.
type Outcome = Vec<(String, bool, c_long)>;

#[derive(Clone, Copy)]
enum Side {
    C,
    Rust,
    Mutant(Mutant),
}

/// The exports' sequences on the adapters, with one change each.
#[derive(Debug, Clone, Copy)]
enum Mutant {
    /// A step sends its signals before calling the callback.
    SignalBeforeCallback,
    /// A step without a callback sends no signal.
    NoSignalWithoutCallback,
    /// The deferred update signals even when it is skipped.
    SignalWhenSkipped,
    /// The count is loaded after the update callback.
    LoadAfterCallback,
    /// The count is set to 0 whatever it holds.
    StoresZero,
    /// Hide sends show's signals.
    HideSendsShow,
}

fn c_step(which: Which) -> StepFn {
    match which {
        Which::Update => oracle_obs_source_deferred_update,
        Which::Activate => oracle_activate_source,
        Which::Deactivate => oracle_deactivate_source,
        Which::Show => oracle_show_source,
        Which::Hide => oracle_hide_source,
    }
}

fn rust_step(which: Which) -> StepFn {
    match which {
        Which::Update => rust::obs_source_deferred_update,
        Which::Activate => rust::activate_source,
        Which::Deactivate => rust::deactivate_source,
        Which::Show => rust::show_source,
        Which::Hide => rust::hide_source,
    }
}

const fn step_number(which: Which) -> c_int {
    match which {
        Which::Activate => 0,
        Which::Deactivate => 1,
        Which::Show => 2,
        Which::Hide | Which::Update => 3,
    }
}

fn signals(which: Which) -> (&'static CStr, &'static CStr) {
    match which {
        Which::Update => (c"source_update", c"update"),
        Which::Activate => (c"source_activate", c"activate"),
        Which::Deactivate => (c"source_deactivate", c"deactivate"),
        Which::Show => (c"source_show", c"show"),
        Which::Hide => (c"source_hide", c"hide"),
    }
}

fn mutant_step(mutant: Mutant, which: Which, source: *mut c_void) {
    // SAFETY: the fixture's live source, under its private core.
    unsafe {
        let signal = |which: Which| {
            let (global, local) = signals(which);
            obs_source_tick_callbacks_signal(source, global.as_ptr(), local.as_ptr());
        };
        if which == Which::Update {
            let count = &*obs_source_tick_callbacks_defer_count(source);
            if !obs_source_tick_callbacks_can_update(source) {
                if matches!(mutant, Mutant::SignalWhenSkipped) {
                    signal(which);
                }
                return;
            }
            let mut loaded = count.load(SeqCst);
            obs_source_tick_callbacks_update(source);
            if matches!(mutant, Mutant::LoadAfterCallback) {
                loaded = count.load(SeqCst);
            }
            if matches!(mutant, Mutant::StoresZero) {
                count.store(0, SeqCst);
            } else {
                let _ = count.compare_exchange(loaded, 0, SeqCst, SeqCst);
            }
            signal(which);
            return;
        }
        let shown = if matches!(mutant, Mutant::HideSendsShow) && which == Which::Hide {
            Which::Show
        } else {
            which
        };
        if matches!(mutant, Mutant::SignalBeforeCallback) {
            signal(shown);
        }
        let has = obs_source_tick_callbacks_has(source, step_number(which));
        if has {
            obs_source_tick_callbacks_call(source, step_number(which));
        }
        if !matches!(mutant, Mutant::SignalBeforeCallback)
            && (has || !matches!(mutant, Mutant::NoSignalWithoutCallback))
        {
            signal(shown);
        }
    }
}

macro_rules! mutant_fns {
    ($($name:ident: $mutant:ident, $which:ident;)*) => {
        $(
            unsafe extern "C" fn $name(source: *mut c_void) {
                mutant_step(Mutant::$mutant, Which::$which, source);
            }
        )*
        fn mutant_fn(mutant: Mutant, which: Which) -> StepFn {
            match (mutant, which) {
                $((Mutant::$mutant, Which::$which) => $name,)*
                _ => rust_step(which),
            }
        }
    };
}
mutant_fns! {
    m_signal_first_show: SignalBeforeCallback, Show;
    m_no_signal_activate: NoSignalWithoutCallback, Activate;
    m_signal_skipped: SignalWhenSkipped, Update;
    m_load_after: LoadAfterCallback, Update;
    m_stores_zero: StoresZero, Update;
    m_hide_shows: HideSendsShow, Hide;
}

fn run(case: &Case, side: Side) -> Outcome {
    // SAFETY: the oracle allocates the fixture and frees it below; the
    // events string it returns lives in the fixture.
    unsafe {
        let fixture = oracle_vtc_new(case.bits, case.defer, case.redefer);
        assert!(!fixture.is_null());
        let mut outcome = Vec::new();
        for &which in &case.calls {
            let step = match side {
                Side::C => c_step(which),
                Side::Rust => rust_step(which),
                Side::Mutant(mutant) => mutant_fn(mutant, which),
            };
            oracle_vtc_run(fixture, step);
            let mut identity = false;
            let mut defer = 0;
            let events = oracle_vtc_record(fixture, &raw mut identity, &raw mut defer);
            let events = CStr::from_ptr(events).to_string_lossy().into_owned();
            outcome.push((events, identity, defer));
        }
        oracle_vtc_free(fixture);
        outcome
    }
}

fn compare(case: &Case) -> (Outcome, Outcome) {
    (run(case, Side::C), run(case, Side::Rust))
}

const DEFERS: [c_long; 6] = [c_long::MIN, -1, 0, 1, 2, c_long::MAX];

fn cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for bits in 0..BITS {
        for which in WHICH {
            if which == Which::Update {
                for defer in DEFERS {
                    for redefer in [-1, 0, 1] {
                        cases.push(Case {
                            bits,
                            defer,
                            redefer,
                            calls: vec![which, which],
                        });
                    }
                }
            } else {
                cases.push(Case {
                    bits,
                    defer: 3,
                    redefer: 1,
                    calls: vec![which],
                });
            }
        }
    }
    cases
}

#[test]
fn matches_c_over_every_combination() {
    if !crate::output::isolated(concat!(
        module_path!(),
        "::matches_c_over_every_combination"
    )) {
        return;
    }
    for case in cases() {
        let (c, rust) = compare(&case);
        assert_eq!(c, rust, "{case:?}");
    }
}

#[test]
fn pins_the_characterized_sequence() {
    if !crate::output::isolated(concat!(module_path!(), "::pins_the_characterized_sequence")) {
        return;
    }
    // Data, update and show, public; one update deferred again per call.
    let case = Case {
        bits: 0b1_0011,
        defer: 1,
        redefer: 1,
        calls: vec![Which::Update, Which::Show, Which::Hide, Which::Activate],
    };
    let (c, rust) = compare(&case);
    assert_eq!(c, rust);
    let update = "cb:update g:source_update l:update";
    let show = format!("{update} cb:show g:source_show l:show");
    let hide = format!("{show} g:source_hide l:hide");
    let activate = format!("{hide} g:source_activate l:activate");
    assert_eq!(
        c,
        [
            (update.to_owned(), true, 2),
            (show, true, 2),
            (hide, true, 2),
            (activate, true, 2),
        ]
    );
    // A private source sends only its own signal; without update nothing.
    let private = Case {
        bits: 0b100_0001,
        defer: 1,
        redefer: 0,
        calls: vec![Which::Update, Which::Deactivate],
    };
    let (c, rust) = compare(&private);
    assert_eq!(c, rust);
    assert_eq!(
        c,
        [
            (String::new(), true, 1),
            ("l:deactivate".to_owned(), true, 1)
        ]
    );
}

#[test]
fn rejects_mutants() {
    if !crate::output::isolated(concat!(module_path!(), "::rejects_mutants")) {
        return;
    }
    let all = Case {
        bits: 0b11_1111,
        defer: 1,
        redefer: 1,
        calls: vec![
            Which::Update,
            Which::Activate,
            Which::Show,
            Which::Hide,
            Which::Deactivate,
        ],
    };
    let bare = Case {
        bits: 0b1,
        defer: 1,
        redefer: 0,
        calls: vec![Which::Update, Which::Activate, Which::Hide],
    };
    for (mutant, case) in [
        (Mutant::SignalBeforeCallback, &all),
        (Mutant::NoSignalWithoutCallback, &bare),
        (Mutant::SignalWhenSkipped, &bare),
        (Mutant::LoadAfterCallback, &all),
        (Mutant::StoresZero, &all),
        (Mutant::HideSendsShow, &all),
    ] {
        let c = run(case, Side::C);
        assert_eq!(c, run(case, Side::Rust), "{case:?}");
        assert_ne!(c, run(case, Side::Mutant(mutant)), "{mutant:?} {case:?}");
    }
}

fn random_case() -> impl Strategy<Value = Case> {
    (
        0..BITS,
        prop_oneof![prop::sample::select(DEFERS.to_vec()), any::<c_long>()],
        -2..3 as c_long,
        prop::collection::vec(prop::sample::select(WHICH.to_vec()), 1..12),
    )
        .prop_map(|(bits, defer, redefer, calls)| Case {
            bits,
            defer,
            redefer,
            calls,
        })
}

#[test]
fn matches_c_on_random_sequences() {
    if !crate::output::isolated(concat!(module_path!(), "::matches_c_on_random_sequences")) {
        return;
    }
    let mut runner = proptest::test_runner::TestRunner::new(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    });
    runner
        .run(&random_case(), |case| {
            let (c, rust) = compare(&case);
            prop_assert_eq!(c, rust);
            Ok(())
        })
        .unwrap();
}
