//! Tier 1: the safe core of `libobs/obs-hotkey-pair.c.h`, the hotkey pair
//! press latch.
//!
//! Two C tests characterize it, and every case of both is ported here one
//! to one, under the same name, in the same order, with the same steps and
//! the same expectations:
//! - `test/cmocka/test_hotkey_pair_steps.c` (4 cases, `steps_*`) compiles
//!   the C into itself and calls the two half callbacks with the latch set
//!   by hand, in all 32 combinations of latch state, half, press or release
//!   and func result;
//! - `test/cmocka/test_hotkey_pair_latch.c` (11 cases, `state_*`) drives a
//!   pair of a real libobs through `obs_hotkey_trigger_routed_callback()`,
//!   with funcs that return a set result for a press and for a release.
//!
//! [`Pair`] repeats what the C does around the core: the pair's two latches,
//! the two funcs (each logs its call and returns its result) and the
//! trigger, which runs a half's callback synchronously, so a func can
//! trigger its partner from inside its own call. What stays C
//! (`register_hotkey_pair_internal()`, the pair hash, the accessor and the
//! call wrapper) is checked by the C tests and by the parity test.
//!
//! The cases after those pin the edges of the safe API: the half table,
//! the order the latches are read and written in, a func that changes
//! either latch while it runs, and a proptest of the invariants over random
//! sequences.

use std::cell::RefCell;

use obs_hotkey::pair::{Half, Latches, Outcome, PairLatches, event};
use proptest::prelude::*;

const FIRST: Half = Half::First;
const SECOND: Half = Half::Second;

fn latches(first: bool, second: bool) -> PairLatches {
    PairLatches { first, second }
}

// ---------------------------------------------------------------------------
// test_hotkey_pair_steps.c

/// The C's `event()`: one event of one half, with a func that logs its
/// call and returns `result`. Returns the number of calls.
fn step(l: &mut PairLatches, half: Half, pressed: bool, result: bool) -> (usize, Vec<Half>) {
    let mut calls = Vec::new();
    event(l, half, pressed, |_| {
        calls.push(half);
        result
    });
    (calls.len(), calls)
}

/// `registered_callbacks`: the pair registers its two halves with the two
/// callbacks (half 0 and half 1, each other's partner), both unlatched.
#[test]
fn steps_registered_callbacks() {
    assert_eq!(Half::BOTH, [FIRST, SECOND]);
    assert_eq!(FIRST.index(), 0);
    assert_eq!(SECOND.index(), 1);
    assert_eq!(FIRST.other(), SECOND);
    assert_eq!(SECOND.other(), FIRST);
    let l = PairLatches::default();
    assert!(!l.latched(FIRST));
    assert!(!l.latched(SECOND));
}

/// Every state, half, event and func result. `called` is whether the
/// half's func runs; `after` is the latch after the event.
struct Row {
    p0: bool,
    p1: bool,
    half: Half,
    pressed: bool,
    result: bool,
    called: bool,
    p0_after: bool,
    p1_after: bool,
}

#[allow(clippy::too_many_arguments)] // one row of the C table, in its column order
const fn row(
    p0: u8,
    p1: u8,
    half: u8,
    pressed: bool,
    result: bool,
    called: bool,
    p0_after: u8,
    p1_after: u8,
) -> Row {
    Row {
        p0: p0 != 0,
        p1: p1 != 0,
        half: if half == 0 { FIRST } else { SECOND },
        pressed,
        result,
        called,
        p0_after: p0_after != 0,
        p1_after: p1_after != 0,
    }
}

#[rustfmt::skip]
const STEPS: [Row; 32] = [
    //   p0 p1 half pressed result called   p0' p1'

    // Neither half latched: the func runs; a true result latches a press.
    row(0, 0, 0, true,  false, true,  0, 0),
    row(0, 0, 0, true,  true,  true,  1, 0),
    row(0, 0, 0, false, false, true,  0, 0),
    row(0, 0, 0, false, true,  true,  0, 0),
    row(0, 0, 1, true,  false, true,  0, 0),
    row(0, 0, 1, true,  true,  true,  0, 1),
    row(0, 0, 1, false, false, true,  0, 0),
    row(0, 0, 1, false, true,  true,  0, 0),

    // The first half latched. Its release clears the latch, no call. Its
    // press calls the func again, and the half stays latched whatever the
    // func returns (characterized, not endorsed: a press acted on twice).
    // Every event of the second half is dropped.
    row(1, 0, 0, true,  false, true,  1, 0),
    row(1, 0, 0, true,  true,  true,  1, 0),
    row(1, 0, 0, false, false, false, 0, 0),
    row(1, 0, 0, false, true,  false, 0, 0),
    row(1, 0, 1, true,  false, false, 1, 0),
    row(1, 0, 1, true,  true,  false, 1, 0),
    row(1, 0, 1, false, false, false, 1, 0),
    row(1, 0, 1, false, true,  false, 1, 0),

    // The second half latched: the mirror image.
    row(0, 1, 0, true,  false, false, 0, 1),
    row(0, 1, 0, true,  true,  false, 0, 1),
    row(0, 1, 0, false, false, false, 0, 1),
    row(0, 1, 0, false, true,  false, 0, 1),
    row(0, 1, 1, true,  false, true,  0, 1),
    row(0, 1, 1, true,  true,  true,  0, 1),
    row(0, 1, 1, false, false, false, 0, 0),
    row(0, 1, 1, false, true,  false, 0, 0),

    // Both latched (characterized, not endorsed): each half sees the
    // other's latch first, so every event is dropped and the pair stays
    // stuck.
    row(1, 1, 0, true,  false, false, 1, 1),
    row(1, 1, 0, true,  true,  false, 1, 1),
    row(1, 1, 0, false, false, false, 1, 1),
    row(1, 1, 0, false, true,  false, 1, 1),
    row(1, 1, 1, true,  false, false, 1, 1),
    row(1, 1, 1, true,  true,  false, 1, 1),
    row(1, 1, 1, false, false, false, 1, 1),
    row(1, 1, 1, false, true,  false, 1, 1),
];

/// `every_step`.
#[test]
fn steps_every_step() {
    for (i, s) in STEPS.iter().enumerate() {
        let mut l = latches(s.p0, s.p1);
        let (n, calls) = step(&mut l, s.half, s.pressed, s.result);
        assert_eq!(n, usize::from(s.called), "step {i}: calls");
        assert_eq!(l, latches(s.p0_after, s.p1_after), "step {i}: latch");
        if s.called {
            assert_eq!(calls, [s.half], "step {i}: the half's own func");
        }
    }
}

/// `latch_written_after_func`: a half writes only its own latch, and only
/// after its func returned. A func that latches the other half from inside
/// its call leaves both latched; one that latches its own half and returns
/// false keeps that; a true result overwrites what the func set.
#[test]
fn steps_latch_written_after_func() {
    let mut l = PairLatches::default();
    let mut calls = 0;
    event(&mut l, FIRST, true, |l| {
        calls += 1;
        l.set_latched(SECOND, true);
        true
    });
    assert_eq!(calls, 1);
    assert_eq!(l, latches(true, true));

    let mut l = PairLatches::default();
    event(&mut l, SECOND, true, |l| {
        l.set_latched(FIRST, true);
        true
    });
    assert_eq!(l, latches(true, true));

    let mut l = PairLatches::default();
    event(&mut l, FIRST, true, |l| {
        l.set_latched(FIRST, true);
        false
    });
    assert_eq!(l, latches(true, false));

    // A true result overwrites what the func set: a release latches false.
    let mut l = PairLatches::default();
    event(&mut l, FIRST, false, |l| {
        l.set_latched(FIRST, true);
        true
    });
    assert_eq!(l, latches(false, false));
}

/// `pair_fields`: a latch set from outside is the one the callbacks read,
/// and a latch set by a callback shows through. The call wrapper itself
/// stays C (`obs_hotkey_pair_call_func()`); here, the func is called with
/// the same latches and its result is the one returned.
#[test]
fn steps_pair_fields() {
    let mut l = PairLatches::default();
    l.set_latched(SECOND, true);
    assert_eq!(step(&mut l, FIRST, true, true).0, 0);
    assert_eq!(step(&mut l, SECOND, false, true).0, 0);
    assert!(!l.second);

    assert_eq!(step(&mut l, FIRST, true, true).0, 1);
    assert!(l.latched(FIRST));
    assert!(!l.latched(SECOND));

    // The func gets the latches as they are, and its result comes back.
    let mut l = PairLatches::default();
    for half in Half::BOTH {
        for r in [false, true] {
            let before = l;
            let outcome = event(&mut l, half, false, |seen| {
                assert_eq!(*seen, before);
                r
            });
            assert_eq!(outcome, Outcome::Called(r));
            assert_eq!(l, PairLatches::default());
        }
    }
}

// ---------------------------------------------------------------------------
// test_hotkey_pair_latch.c

/// A pair as libobs and the test's funcs make it: the two latches, and a
/// func per half that logs (half, pressed) and returns `result_press` for
/// a press and `result_release` for a release. `trigger` runs one half's
/// callback, as `obs_hotkey_trigger_routed_callback()` does.
#[derive(Default)]
struct Pair {
    latches: PairLatches,
    result_press: [bool; 2],
    result_release: [bool; 2],
    calls: Vec<(Half, bool)>,
    /// A func that, on a press of the first half, first triggers a press of
    /// the second.
    reentrant: bool,
}

impl Latches for Pair {
    fn latched(&self, half: Half) -> bool {
        self.latches.latched(half)
    }
    fn set_latched(&mut self, half: Half, latched: bool) {
        self.latches.set_latched(half, latched);
    }
}

impl Pair {
    /// `setup()`: the frontend's funcs, true for a press they act on and
    /// false for a release.
    fn new() -> Self {
        let mut p = Pair::default();
        p.funcs_return(FIRST, true, false);
        p.funcs_return(SECOND, true, false);
        p
    }

    fn funcs_return(&mut self, half: Half, on_press: bool, on_release: bool) {
        self.result_press[half.index()] = on_press;
        self.result_release[half.index()] = on_release;
    }

    fn func(&mut self, half: Half, pressed: bool) -> bool {
        self.calls.push((half, pressed));
        if self.reentrant && half == FIRST && pressed {
            self.trigger(SECOND, true);
        }
        if pressed {
            self.result_press[half.index()]
        } else {
            self.result_release[half.index()]
        }
    }

    fn trigger(&mut self, half: Half, pressed: bool) {
        event(self, half, pressed, |p| p.func(half, pressed));
    }

    fn press(&mut self, half: Half) {
        self.trigger(half, true);
    }

    fn release(&mut self, half: Half) {
        self.trigger(half, false);
    }

    /// The func calls since the last check were exactly these.
    #[track_caller]
    fn expect(&mut self, expected: &[(Half, bool)]) {
        assert_eq!(std::mem::take(&mut self.calls), expected);
    }
}

fn p(half: Half) -> (Half, bool) {
    (half, true)
}
fn r(half: Half) -> (Half, bool) {
    (half, false)
}

/// A press calls the func; it returns true, so the half latches, and its
/// release clears the latch without calling the func.
#[test]
fn state_press_and_release_first() {
    let mut pair = Pair::new();
    pair.press(FIRST);
    pair.expect(&[p(FIRST)]);
    pair.release(FIRST);
    pair.expect(&[]);
    pair.press(FIRST);
    pair.expect(&[p(FIRST)]);
    pair.release(FIRST);
    pair.expect(&[]);
}

#[test]
fn state_press_and_release_second() {
    let mut pair = Pair::new();
    pair.press(SECOND);
    pair.expect(&[p(SECOND)]);
    pair.release(SECOND);
    pair.expect(&[]);
    pair.press(SECOND);
    pair.expect(&[p(SECOND)]);
    pair.release(SECOND);
    pair.expect(&[]);
}

/// A press whose func returns false does not latch: the release calls the
/// func too, and the other half is not blocked.
#[test]
fn state_press_not_acted_on() {
    let mut pair = Pair::new();
    pair.funcs_return(FIRST, false, false);
    pair.press(FIRST);
    pair.expect(&[p(FIRST)]);
    pair.press(SECOND);
    pair.expect(&[p(SECOND)]);
    pair.release(SECOND);
    pair.expect(&[]);
    pair.release(FIRST);
    pair.expect(&[r(FIRST)]);
}

/// A release without a press calls the func, whatever it returns; a true
/// result latches nothing (the latch takes false).
#[test]
fn state_release_without_press() {
    let mut pair = Pair::new();
    pair.release(FIRST);
    pair.expect(&[r(FIRST)]);
    pair.funcs_return(FIRST, true, true);
    pair.release(FIRST);
    pair.expect(&[r(FIRST)]);
    pair.press(SECOND);
    pair.expect(&[p(SECOND)]);
    pair.release(SECOND);
    pair.expect(&[]);
}

/// A second press of a latched half calls the func again, and a false
/// result keeps the half latched (characterized, not endorsed).
#[test]
fn state_press_while_latched() {
    let mut pair = Pair::new();
    pair.press(FIRST);
    pair.expect(&[p(FIRST)]);
    pair.funcs_return(FIRST, false, false);
    pair.press(FIRST);
    pair.expect(&[p(FIRST)]);
    pair.press(SECOND);
    pair.expect(&[]);
    pair.release(FIRST);
    pair.expect(&[]);
    pair.release(FIRST);
    pair.expect(&[r(FIRST)]);
}

#[test]
fn state_second_blocked_while_first_latched() {
    let mut pair = Pair::new();
    pair.press(FIRST);
    pair.expect(&[p(FIRST)]);
    pair.press(SECOND);
    pair.expect(&[]);
    pair.release(SECOND);
    pair.expect(&[]);
    pair.release(FIRST);
    pair.expect(&[]);
    pair.press(SECOND);
    pair.expect(&[p(SECOND)]);
    pair.release(SECOND);
    pair.expect(&[]);
}

#[test]
fn state_first_blocked_while_second_latched() {
    let mut pair = Pair::new();
    pair.press(SECOND);
    pair.expect(&[p(SECOND)]);
    pair.press(FIRST);
    pair.expect(&[]);
    pair.release(FIRST);
    pair.expect(&[]);
    pair.release(SECOND);
    pair.expect(&[]);
    pair.press(FIRST);
    pair.expect(&[p(FIRST)]);
    pair.release(FIRST);
    pair.expect(&[]);
}

/// The second half pressed while the first is latched, and released after
/// the first: its press was dropped, its release reaches its func
/// (characterized, not endorsed).
#[test]
fn state_second_released_after_first() {
    let mut pair = Pair::new();
    pair.press(FIRST);
    pair.expect(&[p(FIRST)]);
    pair.press(SECOND);
    pair.expect(&[]);
    pair.release(FIRST);
    pair.expect(&[]);
    pair.release(SECOND);
    pair.expect(&[r(SECOND)]);
}

/// One key on both halves used as a toggle, with funcs that act only when
/// they apply.
#[test]
fn state_one_key_toggle() {
    let mut pair = Pair::new();
    let cycle = |pair: &mut Pair| {
        pair.press(FIRST);
        pair.press(SECOND);
        pair.release(FIRST);
        pair.release(SECOND);
    };
    cycle(&mut pair);
    pair.expect(&[p(FIRST), r(SECOND)]);

    pair.funcs_return(FIRST, false, false);
    cycle(&mut pair);
    pair.expect(&[p(FIRST), p(SECOND)]);

    pair.funcs_return(FIRST, true, false);
    pair.funcs_return(SECOND, false, false);
    cycle(&mut pair);
    pair.expect(&[p(FIRST), r(SECOND)]);
}

/// The latch is per pair: a latched pair does not block another pair.
#[test]
fn state_latch_per_pair() {
    let mut pair = Pair::new();
    let mut other = PairLatches::default();
    let (mut other_first, mut other_second) = (0, 0);
    let mut other_event = |l: &mut PairLatches, half: Half, pressed: bool| {
        event(l, half, pressed, |_| {
            if pressed {
                match half {
                    Half::First => other_first += 1,
                    Half::Second => other_second += 1,
                }
            }
            pressed
        });
    };

    pair.press(FIRST);
    pair.expect(&[p(FIRST)]);
    other_event(&mut other, SECOND, true);
    pair.press(SECOND);
    pair.expect(&[]);
    other_event(&mut other, SECOND, false);
    pair.release(FIRST);
    pair.expect(&[]);
    assert_eq!((other_first, other_second), (0, 1));
}

/// A func that triggers its partner from inside its press latches both
/// halves, and every later event of either half is dropped (characterized,
/// not endorsed). It also pins that each half writes only its own latch
/// after its func returns.
#[test]
fn state_reentrant_press_latches_both() {
    let mut pair = Pair::new();
    pair.reentrant = true;
    pair.press(FIRST);
    pair.expect(&[p(FIRST), p(SECOND)]);
    pair.reentrant = false;
    assert_eq!(pair.latches, latches(true, true));

    pair.release(FIRST);
    pair.release(SECOND);
    pair.press(FIRST);
    pair.press(SECOND);
    pair.release(SECOND);
    pair.release(FIRST);
    pair.expect(&[]);
    assert_eq!(pair.latches, latches(true, true));
}

// ---------------------------------------------------------------------------
// Edges of the safe API

/// One access of the latches, or the func call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Access {
    Read(Half),
    Write(Half, bool),
    Call,
}

/// Latches that log every read and write (a cell, as a read has only
/// `&self`).
#[derive(Default)]
struct Logged {
    latches: PairLatches,
    log: RefCell<Vec<Access>>,
}

impl Latches for Logged {
    fn latched(&self, half: Half) -> bool {
        self.log.borrow_mut().push(Access::Read(half));
        self.latches.latched(half)
    }
    fn set_latched(&mut self, half: Half, latched: bool) {
        self.log.get_mut().push(Access::Write(half, latched));
        self.latches.set_latched(half, latched);
    }
}

impl Logged {
    /// One event with a func that changes the latches to `during` (if any)
    /// and returns `result`: the access log, the outcome and the latches.
    fn run(
        start: PairLatches,
        half: Half,
        pressed: bool,
        during: Option<PairLatches>,
        result: bool,
    ) -> (Vec<Access>, Outcome, PairLatches) {
        let mut l = Logged {
            latches: start,
            log: RefCell::new(Vec::new()),
        };
        let outcome = event(&mut l, half, pressed, |l| {
            l.log.get_mut().push(Access::Call);
            if let Some(d) = during {
                // Not through set_latched: the func's writes are not the
                // core's.
                l.latches = d;
            }
            result
        });
        (l.log.into_inner(), outcome, l.latches)
    }
}

/// The C reads the other half's latch, then this half's (only when the
/// other is not latched), before the func; it writes only this half's
/// latch, after the func: false for the release of a latched half, the
/// event's pressed value after a true result.
#[test]
fn latch_access_order() {
    use Access::*;
    for half in Half::BOTH {
        let o = half.other();
        for start in [(false, false), (true, false), (false, true), (true, true)] {
            for pressed in [false, true] {
                for result in [false, true] {
                    let mut s = PairLatches::default();
                    s.set_latched(FIRST, start.0);
                    s.set_latched(SECOND, start.1);
                    let (log, outcome, end) = Logged::run(s, half, pressed, None, result);
                    let ctx = format!("{half:?} {start:?} pressed {pressed} result {result}");
                    if s.latched(o) {
                        assert_eq!(log, [Read(o)], "{ctx}");
                        assert_eq!(outcome, Outcome::Dropped, "{ctx}");
                        assert_eq!(end, s, "{ctx}");
                    } else if s.latched(half) && !pressed {
                        assert_eq!(log, [Read(o), Read(half), Write(half, false)], "{ctx}");
                        assert_eq!(outcome, Outcome::Cleared, "{ctx}");
                    } else if result {
                        assert_eq!(
                            log,
                            [Read(o), Read(half), Call, Write(half, pressed)],
                            "{ctx}"
                        );
                        assert_eq!(outcome, Outcome::Called(true), "{ctx}");
                    } else {
                        assert_eq!(log, [Read(o), Read(half), Call], "{ctx}");
                        assert_eq!(outcome, Outcome::Called(false), "{ctx}");
                        assert_eq!(end, s, "{ctx}");
                    }
                }
            }
        }
    }
}

/// Whatever the func leaves in either latch is kept; only a true result
/// then sets this half's latch. Nothing read before the call is written
/// back. Every start state, half, event, result, and every latch state the
/// func can leave.
#[test]
fn func_changes_kept() {
    let all = [
        latches(false, false),
        latches(true, false),
        latches(false, true),
        latches(true, true),
    ];
    for half in Half::BOTH {
        for start in all {
            for pressed in [false, true] {
                for result in [false, true] {
                    for during in all {
                        let (_, outcome, end) =
                            Logged::run(start, half, pressed, Some(during), result);
                        let ctx = format!(
                            "{half:?} {start:?} pressed {pressed} result {result} during {during:?}"
                        );
                        let mut want = start;
                        if start.latched(half.other()) {
                            assert_eq!(outcome, Outcome::Dropped, "{ctx}");
                        } else if start.latched(half) && !pressed {
                            assert_eq!(outcome, Outcome::Cleared, "{ctx}");
                            want.set_latched(half, false);
                        } else {
                            assert_eq!(outcome, Outcome::Called(result), "{ctx}");
                            want = during;
                            if result {
                                want.set_latched(half, pressed);
                            }
                        }
                        assert_eq!(end, want, "{ctx}");
                    }
                }
            }
        }
    }
}

/// A dropped or cleared event never calls the func.
#[test]
fn dropped_and_cleared_never_call() {
    let mut l = latches(false, true);
    let out = event(&mut l, FIRST, true, |_| panic!("called"));
    assert_eq!(out, Outcome::Dropped);
    let mut l = latches(true, false);
    let out = event(&mut l, FIRST, false, |_| panic!("called"));
    assert_eq!(out, Outcome::Cleared);
    assert_eq!(l, latches(false, false));
}

/// The plain latches keep the two halves apart.
#[test]
fn plain_latches_per_half() {
    let mut l = PairLatches::default();
    l.set_latched(FIRST, true);
    assert_eq!(l, latches(true, false));
    assert!(l.latched(FIRST) && !l.latched(SECOND));
    l.set_latched(SECOND, true);
    l.set_latched(FIRST, false);
    assert_eq!(l, latches(false, true));
}

proptest! {
    /// Random events with random results, no func re-entering: the two
    /// halves are never latched at once; an event is dropped exactly when
    /// the other half was latched; a latch is set only by a press of its
    /// half that the func acted on; and each call is one Called outcome.
    #[test]
    fn random_sequences_hold_invariants(
        events in prop::collection::vec((any::<bool>(), any::<bool>(), any::<bool>()), 0..64),
    ) {
        let mut l = PairLatches::default();
        for (second, pressed, result) in events {
            let half = if second { SECOND } else { FIRST };
            let before = l;
            let mut calls = 0;
            let outcome = event(&mut l, half, pressed, |_| {
                calls += 1;
                result
            });
            prop_assert!(!(l.first && l.second));
            prop_assert_eq!(outcome == Outcome::Dropped, before.latched(half.other()));
            prop_assert_eq!(calls, usize::from(matches!(outcome, Outcome::Called(_))));
            prop_assert_eq!(l.latched(half.other()), before.latched(half.other()));
            if l.latched(half) && !before.latched(half) {
                prop_assert!(pressed && result);
            }
        }
    }
}
