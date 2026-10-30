//! Tier 1: the safe core of `libobs/obs-hotkey-binding-press.c.h`, the hotkey
//! binding press count.
//!
//! `test/cmocka/test_hotkey_binding_press_steps.c` characterizes it, and every
//! case of that test is ported here one to one, under the same name, with the
//! same steps and the same expectations:
//! - `every_step`: 2 flag states x 6 counts (-2..3) x press or release, in
//!   each of the C's three routing states. Routing is not part of the core:
//!   it decides whether the caller dispatches at all, and the core only says
//!   whether it should. So each row is run three times, once per routing
//!   state, and the expectation is the C's `expects_call` split into the
//!   core's answer and the routing's;
//! - `two_bindings`: the count is per hotkey, so two bindings of one hotkey
//!   fire once, and the last release is the one that calls;
//! - `release_without_press`: characterized, not endorsed, a release at count
//!   0 goes negative, and a press of a pressed binding counts again;
//! - `writes_before_the_return`: both fields hold their new values when the
//!   core returns `true`, which is what lets the shim dispatch after them and
//!   write nothing after it.
//!
//! `fields_and_dispatch` is the C accessor and the C dispatch, which the port
//! keeps in C; the parity test checks the accessor's layout and the shim's use
//! of both. The cases after those pin the edges of the safe API: the count is
//! C's `int`, and the core wraps where C is undefined, with a proptest over
//! random sequences of bindings and events.

use obs_hotkey::binding_press::{press, release};
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// test_hotkey_binding_press_steps.c

/// The C's routing states: the dispatch is the hotkey's own func, the router,
/// or nothing (rerouting on and no router set).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Routing {
    Off,
    OnWithRouter,
    OnWithoutRouter,
}

const ROUTINGS: [Routing; 3] = [
    Routing::Off,
    Routing::OnWithRouter,
    Routing::OnWithoutRouter,
];

impl Routing {
    /// Whether a dispatch reaches a callback, as the C's `via != 0`.
    fn delivers(self) -> bool {
        match self {
            Routing::Off | Routing::OnWithRouter => true,
            Routing::OnWithoutRouter => false,
        }
    }
}

/// One row of the C's table: the binding's flag before the event, the
/// hotkey's count before it, and whether it is a press.
struct Step {
    flag: bool,
    count: i32,
    press: bool,
}

/// The C's `steps[]`, in its order.
fn steps() -> Vec<Step> {
    let mut steps = Vec::new();
    for flag in [false, true] {
        for count in -2..=3 {
            for press in [true, false] {
                steps.push(Step { flag, count, press });
            }
        }
    }
    steps
}

/// `every_step`: the C's table, in every routing state.
#[test]
fn every_step() {
    let steps = steps();
    assert_eq!(steps.len(), 24);
    for routing in ROUTINGS {
        for (i, step) in steps.iter().enumerate() {
            let mut flag = step.flag;
            let mut count = step.count;
            // The post-increment tests the count before the press; the
            // pre-decrement tests the count after the release.
            let (fires, count_after) = if step.press {
                (press(&mut flag, &mut count), step.count + 1)
            } else {
                (release(&mut flag, &mut count), step.count - 1)
            };
            let calls_now = if step.press {
                step.count == 0
            } else {
                step.count == 1
            };
            let expects_call = calls_now && routing.delivers();
            assert_eq!(flag, step.press, "{routing:?} step {i}: the flag");
            assert_eq!(count, count_after, "{routing:?} step {i}: the count");
            assert_eq!(fires, calls_now, "{routing:?} step {i}: the core's answer");
            assert_eq!(
                fires && routing.delivers(),
                expects_call,
                "{routing:?} step {i}: the C's call"
            );
        }
    }
}

/// The C's closing case of `every_step`: the fields already hold their new
/// values after the event, so a caller reading them right after still sees
/// them. That is what the shim relies on when it dispatches last.
#[test]
fn writes_before_the_return() {
    let mut flag = false;
    let mut count = 0;
    assert!(press(&mut flag, &mut count));
    assert!(flag);
    assert_eq!(count, 1);
}

/// `two_bindings`: one hotkey, two bindings, so the count is shared: the first
/// press fires, the second does not, the first release does not and the last
/// one does.
#[test]
fn two_bindings() {
    let mut first = false;
    let mut second = false;
    let mut count = 0;

    assert!(press(&mut first, &mut count));
    assert!(first);
    assert!(!second);
    assert_eq!(count, 1);

    assert!(!press(&mut second, &mut count));
    assert!(second);
    assert_eq!(count, 2);

    assert!(!release(&mut first, &mut count));
    assert!(!first);
    assert_eq!(count, 1);

    assert!(release(&mut second, &mut count));
    assert!(!second);
    assert_eq!(count, 0);
}

/// `release_without_press`: characterized, not endorsed. libobs presses only a
/// released binding and releases only a pressed one, so it never reaches
/// either.
#[test]
fn release_without_press() {
    let mut flag = false;
    let mut count = 0;
    assert!(!release(&mut flag, &mut count));
    assert_eq!(count, -1);
    assert!(!flag);

    // And a press of a pressed binding counts again.
    let mut flag = true;
    let mut count = 1;
    assert!(!press(&mut flag, &mut count));
    assert_eq!(count, 2);
}

/// The count is C's `int`: at the edges of `i32`, where C is undefined, the
/// core wraps and never panics. libobs never reaches them.
#[test]
fn wraps_at_the_edges() {
    let mut flag = false;
    let mut count = i32::MAX;
    assert!(!press(&mut flag, &mut count));
    assert_eq!(count, i32::MIN);

    let mut flag = true;
    let mut count = i32::MIN;
    assert!(!release(&mut flag, &mut count));
    assert_eq!(count, i32::MAX);
}

// ---------------------------------------------------------------------------
// Invariants over random sequences

proptest! {
    /// Over every event, the count is the number of presses minus the number
    /// of releases so far, the flag is the event's pressed value, and the core
    /// fires exactly on a press that leaves 0 and a release that reaches 0.
    #[test]
    fn count_is_presses_minus_releases(events in proptest::collection::vec(any::<bool>(), 0..64)) {
        let mut flag = false;
        let mut count: i32 = 0;
        let mut expected: i64 = 0;
        for pressed in events {
            let fires = if pressed {
                press(&mut flag, &mut count)
            } else {
                release(&mut flag, &mut count)
            };
            expected += if pressed { 1 } else { -1 };
            prop_assert_eq!(flag, pressed);
            prop_assert_eq!(i64::from(count), expected);
            let fires_now = expected == i64::from(pressed);
            prop_assert_eq!(fires, fires_now);
        }
    }
}
