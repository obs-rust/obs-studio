//! Tier 2 layout and Tier 3 parity: the shims against the original
//! `libobs/obs-hotkey-pair.c.h` (oracle/hotkey_pair.c).
//!
//! The unchanged C test `test_hotkey_pair_latch` sees the Rust swap through
//! a real libobs; `test_hotkey_pair_steps` compiles the C into itself, so it
//! characterizes the C only. There is no `_rust` variant of it linking
//! `libobs_rust-static`: obs-hotkey pulls in obs-data, obs-util and hidden
//! libobs symbols that such a test does not link. This file compares the two
//! implementations directly, on every OS, over every state and far more
//! sequences than the C tests script.
//!
//! Both sides run on the same fake pairs. This file defines the two hidden
//! seams the C and the shims call (`obs_hotkey_pair_fields` and
//! `obs_hotkey_pair_call_func`) and logs every call, per thread: which
//! pair's fields were taken; each func call with its pair, half, hotkey,
//! pressed value and both latches as the func sees them; and each return,
//! with the result and both latches as the func leaves them. A func follows
//! a script: it returns a set result, and before or after that it may set
//! either latch of either pair and trigger either half of either pair
//! through the same side's callbacks, as a C func that triggers its partner
//! does. So a latch read made after the call instead of before, a write
//! from a copy taken before the call, a write of the other half's latch, or
//! a write made before the func returns changes the log or the latches.
//!
//! A run feeds one sequence of events and of latch changes from outside to
//! a C world and a Rust world, and compares after every step: the call log
//! one to one (order and count) and the latches of every pair.
//!
//! Intentional differences, excluded here: a null `data`, or a null latch
//! pointer from the accessor, does nothing in Rust; C dereferences it
//! (`null_pair_does_nothing` checks the Rust side). Nothing else is
//! excluded.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::ffi::c_void;
use std::mem::{align_of, offset_of, size_of};

use obs_c_oracle::hotkey_pair::{
    LAYOUT_NAMES, oracle_hotkey_pair_layout, oracle_obs_hotkey_pair_first_func,
    oracle_obs_hotkey_pair_second_func,
};
use obs_hotkey::ffi::pair::{
    obs_hotkey_pair, obs_hotkey_pair_fields as Fields, obs_hotkey_pair_first_func,
    obs_hotkey_pair_second_func, obs_hotkey_t,
};
use proptest::prelude::*;

#[test]
fn layout_matches_c() {
    let rust = [
        size_of::<Fields>(),
        align_of::<Fields>(),
        offset_of!(Fields, pressed0),
        offset_of!(Fields, pressed1),
        size_of::<bool>(),
        size_of::<usize>(),
    ];
    for (i, (name, value)) in LAYOUT_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: a plain getter; `i` is in range.
        let c = unsafe { oracle_hotkey_pair_layout(i) };
        assert_eq!(value, c, "{name}");
    }
}

// ---------------------------------------------------------------------------
// The fake libobs side

/// One pair. The seams reach it through the pointer the callbacks get as
/// their data, as libobs's reach `struct obs_hotkey_pair`.
#[repr(C)]
#[derive(Debug)]
struct FakePair {
    pressed0: bool,
    pressed1: bool,
    /// Its index in the world, for the log.
    index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    C,
    Rust,
}

/// One call into the seams.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Call {
    Fields(usize),
    /// A func call: pair, half, hotkey, pressed, and the pair's latches as
    /// the func sees them.
    Func {
        pair: usize,
        half: usize,
        hotkey: usize,
        pressed: bool,
        latches: (bool, bool),
    },
    /// The func returns: pair, half, result, and the pair's latches as it
    /// leaves them.
    Return {
        pair: usize,
        half: usize,
        result: bool,
        latches: (bool, bool),
    },
}

/// One latch change: pair, half, value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Set {
    pair: usize,
    half: usize,
    value: bool,
}

/// One event of one half of one pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Event {
    pair: usize,
    half: usize,
    pressed: bool,
    /// The hotkey passed to the callback, passed on to the func; never
    /// dereferenced.
    hotkey: usize,
    /// The hotkey id passed to the callback, which ignores it.
    id: usize,
}

/// What one func call does, in this order: `before`, `trigger`, `after`,
/// then return `result`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Script {
    result: bool,
    before: Option<Set>,
    trigger: Option<Event>,
    after: Option<Set>,
}

#[derive(Default)]
struct Seams {
    side: Option<Side>,
    pairs: Vec<*mut FakePair>,
    log: Vec<Call>,
    /// One per func call; an empty queue returns false and does nothing
    /// else.
    scripts: VecDeque<Script>,
    /// The accessor returns this pointer as null (Rust side only).
    null_field: Option<usize>,
}

thread_local! {
    static SEAMS: RefCell<Seams> = RefCell::new(Seams::default());
}

fn with_seams<R>(f: impl FnOnce(&mut Seams) -> R) -> R {
    SEAMS.with(|s| f(&mut s.borrow_mut()))
}

/// # Safety
///
/// `p` is a live [`FakePair`].
unsafe fn latches_of(p: *mut FakePair) -> (bool, bool) {
    // SAFETY: per the caller.
    unsafe { ((*p).pressed0, (*p).pressed1) }
}

fn set_latch(s: Set) {
    let p = with_seams(|seams| seams.pairs[s.pair]);
    // SAFETY: a live pair of this world.
    unsafe {
        if s.half == 0 {
            (*p).pressed0 = s.value;
        } else {
            (*p).pressed1 = s.value;
        }
    }
}

/// The pair accessor (`obs-hotkey.c.h`).
///
/// # Safety
///
/// `pair` points at a live [`FakePair`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_pair_fields(pair: *mut obs_hotkey_pair) -> Fields {
    let p = pair.cast::<FakePair>();
    // SAFETY: a live FakePair, per the caller.
    let index = unsafe { (*p).index };
    let null_field = with_seams(|s| {
        s.log.push(Call::Fields(index));
        s.null_field
    });
    // SAFETY: as above; the pointers are to its own fields.
    let mut fields = unsafe {
        Fields {
            pressed0: &raw mut (*p).pressed0,
            pressed1: &raw mut (*p).pressed1,
        }
    };
    match null_field {
        Some(0) => fields.pressed0 = std::ptr::null_mut(),
        Some(_) => fields.pressed1 = std::ptr::null_mut(),
        None => {}
    }
    fields
}

/// The call wrapper (`obs-hotkey.c.h`): `func[half]` of the pair, which
/// here runs the next script.
///
/// # Safety
///
/// `pair` points at a live [`FakePair`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_pair_call_func(
    pair: *mut obs_hotkey_pair,
    half: usize,
    hotkey: *mut obs_hotkey_t,
    pressed: bool,
) -> bool {
    let p = pair.cast::<FakePair>();
    // SAFETY: a live FakePair, per the caller.
    let (index, latches) = unsafe { ((*p).index, latches_of(p)) };
    let script = with_seams(|s| {
        s.log.push(Call::Func {
            pair: index,
            half,
            hotkey: hotkey as usize,
            pressed,
            latches,
        });
        s.scripts.pop_front().unwrap_or_default()
    });
    if let Some(set) = script.before {
        set_latch(set);
    }
    if let Some(event) = script.trigger {
        fire(event);
    }
    if let Some(set) = script.after {
        set_latch(set);
    }
    // SAFETY: as above.
    let latches = unsafe { latches_of(p) };
    with_seams(|s| {
        s.log.push(Call::Return {
            pair: index,
            half,
            result: script.result,
            latches,
        })
    });
    script.result
}

/// Runs one half's callback of this world's side.
fn fire(e: Event) {
    let (side, p) = with_seams(|s| (s.side.expect("a side"), s.pairs[e.pair]));
    let data = p.cast::<c_void>();
    // SAFETY: a live pair; the hotkey is passed on, never dereferenced.
    unsafe {
        match (side, e.half) {
            (Side::C, 0) => oracle_obs_hotkey_pair_first_func(data, e.id, e.hotkey as _, e.pressed),
            (Side::C, _) => {
                oracle_obs_hotkey_pair_second_func(data, e.id, e.hotkey as _, e.pressed);
            }
            (Side::Rust, 0) => obs_hotkey_pair_first_func(data, e.id, e.hotkey as _, e.pressed),
            (Side::Rust, _) => obs_hotkey_pair_second_func(data, e.id, e.hotkey as _, e.pressed),
        }
    }
}

// ---------------------------------------------------------------------------
// Runs

#[derive(Debug, Clone, PartialEq, Eq)]
enum Step {
    /// An event, with the scripts of the func calls it makes.
    Event(Event, Vec<Script>),
    /// A latch change from outside.
    Set(Set),
}

/// What one step left: the call log and the latches of every pair.
type Trace = Vec<(Vec<Call>, Vec<(bool, bool)>)>;

fn run(side: Side, start: &[(bool, bool)], steps: &[Step]) -> Trace {
    let mut pairs: Vec<Box<FakePair>> = start
        .iter()
        .enumerate()
        .map(|(index, &(p0, p1))| {
            Box::new(FakePair {
                pressed0: p0,
                pressed1: p1,
                index,
            })
        })
        .collect();
    let ptrs: Vec<*mut FakePair> = pairs.iter_mut().map(|p| &raw mut **p).collect();
    with_seams(|s| {
        *s = Seams {
            side: Some(side),
            pairs: ptrs.clone(),
            ..Seams::default()
        }
    });
    let mut trace = Vec::new();
    for step in steps {
        match step {
            Step::Event(e, scripts) => {
                with_seams(|s| s.scripts = scripts.iter().cloned().collect());
                fire(*e);
            }
            Step::Set(set) => set_latch(*set),
        }
        let log = with_seams(|s| std::mem::take(&mut s.log));
        // SAFETY: the pairs are alive until the end of this function.
        let latches = ptrs.iter().map(|&p| unsafe { latches_of(p) }).collect();
        trace.push((log, latches));
    }
    with_seams(|s| *s = Seams::default());
    drop(pairs);
    trace
}

/// Runs both sides and compares them after every step.
fn check(start: &[(bool, bool)], steps: &[Step]) -> Result<(), String> {
    let c = run(Side::C, start, steps);
    let rust = run(Side::Rust, start, steps);
    for (i, (c, rust)) in c.iter().zip(&rust).enumerate() {
        if c != rust {
            return Err(format!(
                "step {i} of {steps:?} from {start:?}:\n  C:    {c:?}\n  Rust: {rust:?}"
            ));
        }
    }
    Ok(())
}

fn hotkey_of(pair: usize, half: usize) -> usize {
    0x1000 + pair * 0x20 + half * 0x10
}

fn ev(pair: usize, half: usize, pressed: bool) -> Event {
    Event {
        pair,
        half,
        pressed,
        hotkey: hotkey_of(pair, half),
        id: 12345,
    }
}

const STATES: [(bool, bool); 4] = [(false, false), (true, false), (false, true), (true, true)];

/// Every latch state, half, event and func result, with every thing the
/// func can do while it runs: nothing; set either latch to either value,
/// before or after a nested event; and a nested event of either half of
/// the pair, pressed or not, whose own func returns either result.
#[test]
fn every_single_event_matches_c() {
    let mut sets = vec![None];
    for half in 0..2 {
        for value in [false, true] {
            sets.push(Some(Set {
                pair: 0,
                half,
                value,
            }));
        }
    }
    let mut nested = vec![None];
    for half in 0..2 {
        for pressed in [false, true] {
            for result in [false, true] {
                nested.push(Some((ev(0, half, pressed), result)));
            }
        }
    }
    let mut runs = 0;
    for start in STATES {
        for half in 0..2 {
            for pressed in [false, true] {
                for result in [false, true] {
                    for &set in &sets {
                        for &inner in &nested {
                            for set_after in [false, true] {
                                let (before, after) =
                                    if set_after { (None, set) } else { (set, None) };
                                let mut scripts = vec![Script {
                                    result,
                                    before,
                                    trigger: inner.map(|(e, _)| e),
                                    after,
                                }];
                                if let Some((_, r)) = inner {
                                    scripts.push(Script {
                                        result: r,
                                        ..Script::default()
                                    });
                                }
                                let steps = [Step::Event(ev(0, half, pressed), scripts)];
                                check(&[start], &steps).unwrap();
                                runs += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(runs, 4 * 2 * 2 * 2 * 5 * 9 * 2);
}

/// Every sequence of four events (half, pressed, func result) from every
/// latch state: 4 x 8^4 runs.
#[test]
fn every_sequence_of_four_matches_c() {
    let mut choices = Vec::new();
    for half in 0..2 {
        for pressed in [false, true] {
            for result in [false, true] {
                choices.push((half, pressed, result));
            }
        }
    }
    let mut runs = 0;
    for start in STATES {
        for n in 0..choices.len().pow(4) {
            let steps: Vec<Step> = (0..4)
                .map(|k| {
                    let (half, pressed, result) = choices[(n / choices.len().pow(k)) % 8];
                    Step::Event(
                        ev(0, half, pressed),
                        vec![Script {
                            result,
                            ..Script::default()
                        }],
                    )
                })
                .collect();
            check(&[start], &steps).unwrap();
            runs += 1;
        }
    }
    assert_eq!(runs, 4 * 4096);
}

/// The scripted cases of the C characterizations, as sequences: a func
/// triggering its partner from inside its press (both latched, then every
/// event dropped), and a func latching the other half or its own while it
/// runs.
#[test]
fn characterized_sequences_match_c() {
    let t = |result| Script {
        result,
        ..Script::default()
    };
    let reentrant = vec![
        Script {
            result: true,
            trigger: Some(ev(0, 1, true)),
            ..Script::default()
        },
        t(true),
    ];
    let mut steps = vec![Step::Event(ev(0, 0, true), reentrant)];
    for (half, pressed) in [
        (0, false),
        (1, false),
        (0, true),
        (1, true),
        (1, false),
        (0, false),
    ] {
        steps.push(Step::Event(ev(0, half, pressed), vec![t(true)]));
    }
    check(&[(false, false)], &steps).unwrap();

    for (half, other, result, pressed) in [
        (0, 1, true, true),
        (1, 0, true, true),
        (0, 0, false, true),
        (0, 0, true, false),
    ] {
        let script = Script {
            result,
            before: Some(Set {
                pair: 0,
                half: other,
                value: true,
            }),
            ..Script::default()
        };
        check(
            &[(false, false)],
            &[Step::Event(ev(0, half, pressed), vec![script])],
        )
        .unwrap();
    }

    // The latch per pair: a latched pair does not block another pair.
    let steps = [
        Step::Event(ev(0, 0, true), vec![t(true)]),
        Step::Event(ev(1, 1, true), vec![t(true)]),
        Step::Event(ev(0, 1, true), vec![t(true)]),
        Step::Event(ev(1, 1, false), vec![t(false)]),
        Step::Event(ev(0, 0, false), vec![t(false)]),
    ];
    check(&[(false, false), (false, false)], &steps).unwrap();
}

/// Rust only: a null `data`, or a null latch pointer from the accessor,
/// does nothing (C dereferences it). The intentional difference.
#[test]
fn null_pair_does_nothing() {
    with_seams(|s| *s = Seams::default());
    // SAFETY: a null pair is the case under test.
    unsafe {
        obs_hotkey_pair_first_func(std::ptr::null_mut(), 0, std::ptr::null_mut(), true);
        obs_hotkey_pair_second_func(std::ptr::null_mut(), 0, std::ptr::null_mut(), false);
    }
    assert!(with_seams(|s| s.log.is_empty()));

    for null_field in [0, 1] {
        for half in 0..2 {
            let mut pair = Box::new(FakePair {
                pressed0: false,
                pressed1: false,
                index: 0,
            });
            let p = &raw mut *pair;
            with_seams(|s| {
                *s = Seams {
                    side: Some(Side::Rust),
                    pairs: vec![p],
                    null_field: Some(null_field),
                    scripts: VecDeque::from([Script {
                        result: true,
                        ..Script::default()
                    }]),
                    ..Seams::default()
                }
            });
            fire(ev(0, half, true));
            let log = with_seams(|s| std::mem::take(&mut s.log));
            assert_eq!(log, [Call::Fields(0)]);
            // SAFETY: alive.
            assert_eq!(unsafe { latches_of(p) }, (false, false));
            with_seams(|s| *s = Seams::default());
            drop(pair);
        }
    }
}

// ---------------------------------------------------------------------------
// Random runs

fn set_strategy(pairs: usize) -> impl Strategy<Value = Set> {
    (0..pairs, 0..2usize, any::<bool>()).prop_map(|(pair, half, value)| Set { pair, half, value })
}

fn event_strategy(pairs: usize) -> impl Strategy<Value = Event> {
    (
        0..pairs,
        0..2usize,
        any::<bool>(),
        prop_oneof![
            4 => Just(None),
            1 => Just(Some(0usize)),
            1 => any::<usize>().prop_map(Some),
        ],
        any::<usize>(),
    )
        .prop_map(|(pair, half, pressed, hotkey, id)| Event {
            pair,
            half,
            pressed,
            hotkey: hotkey.unwrap_or_else(|| hotkey_of(pair, half)),
            id,
        })
}

fn script_strategy(pairs: usize) -> impl Strategy<Value = Script> {
    (
        any::<bool>(),
        prop::option::weighted(0.2, set_strategy(pairs)),
        prop::option::weighted(0.15, event_strategy(pairs)),
        prop::option::weighted(0.2, set_strategy(pairs)),
    )
        .prop_map(|(result, before, trigger, after)| Script {
            result,
            before,
            trigger,
            after,
        })
}

fn step_strategy(pairs: usize) -> impl Strategy<Value = Step> {
    prop_oneof![
        8 => (event_strategy(pairs), prop::collection::vec(script_strategy(pairs), 0..4))
            .prop_map(|(e, scripts)| Step::Event(e, scripts)),
        1 => set_strategy(pairs).prop_map(Step::Set),
    ]
}

fn world_strategy() -> impl Strategy<Value = (Vec<(bool, bool)>, Vec<Step>)> {
    (1..=3usize).prop_flat_map(|pairs| {
        (
            prop::collection::vec((any::<bool>(), any::<bool>()), pairs),
            prop::collection::vec(step_strategy(pairs), 0..40),
        )
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(2000)))]

    /// Up to 3 pairs with random latches; up to 40 steps of events (any
    /// hotkey pointer and id) whose funcs return random results and may set
    /// latches and trigger events of any pair, nested, and latch changes
    /// from outside.
    #[test]
    fn random_sequences_match_c((start, steps) in world_strategy()) {
        if let Err(e) = check(&start, &steps) {
            prop_assert!(false, "{}", e);
        }
    }
}
