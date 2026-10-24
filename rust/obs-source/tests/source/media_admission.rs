//! Tier 1: `obs_source::media_admission`, the media action admission
//! decisions.
//!
//! The `steps_*` cases port `test/cmocka/test_source_media_admission_steps.c`
//! 1:1, under the same case names, in the same order, with the same inputs
//! and expectations. The C test's `enum_values` also checks the mirrors
//! against libobs's headers; that half is Tier 3's (the oracle compiles
//! them).
//!
//! The `edge_*` cases are new: every call value from -1000 to 1000 and the
//! ends of `i32` with every single output flag bit, against the rule
//! written out; every action type and dispatch value over the same range;
//! the steps in the callers' order with the values each one reads; a model
//! of `process_media_actions()` over random queues; and proptests of each
//! over full-range values.

use std::ffi::CStr;

use obs_source::media_admission::{
    Admit, CONTROLLABLE_MEDIA, Call, Dispatch, MEDIA_STATE_NONE, action, admit, admit_callback,
    admit_flags, dispatch, signal, signal_reads_pause, state_fallback, time_fallback,
};
use proptest::prelude::*;

/// `OBS_SOURCE_VIDEO`, `OBS_SOURCE_AUDIO`, `OBS_SOURCE_ASYNC` and
/// `OBS_SOURCE_ASYNC_VIDEO` (`obs-source.h`).
const OBS_SOURCE_VIDEO: u32 = 1 << 0;
const OBS_SOURCE_AUDIO: u32 = 1 << 1;
const OBS_SOURCE_ASYNC: u32 = 1 << 2;
const OBS_SOURCE_ASYNC_VIDEO: u32 = OBS_SOURCE_ASYNC | OBS_SOURCE_VIDEO;

/// The C test's `FLAGS`: none, the flag alone, every bit, every bit but
/// it, its neighbours alone, and the flag with others.
const FLAGS: [u32; 8] = [
    0,
    CONTROLLABLE_MEDIA,
    u32::MAX,
    !CONTROLLABLE_MEDIA,
    CONTROLLABLE_MEDIA << 1,
    CONTROLLABLE_MEDIA >> 1,
    OBS_SOURCE_VIDEO | OBS_SOURCE_AUDIO | OBS_SOURCE_ASYNC,
    CONTROLLABLE_MEDIA | OBS_SOURCE_ASYNC_VIDEO | OBS_SOURCE_AUDIO,
];

/// Calls, action types and dispatches outside their enums, as the C
/// test's.
const OUTSIDE_CALLS: [i32; 6] = [-1, 11, 12, 99, i32::MIN, i32::MAX];
const OUTSIDE_TYPES: [i32; 6] = [7, 8, -1, 100, i32::MIN, i32::MAX];
const OUTSIDE_DISPATCHES: [i32; 6] = [8, 9, -1, 99, i32::MIN, i32::MAX];

/// The callers' rule, written out: the calls that need a callback.
fn needs_callback(call: Option<Call>) -> bool {
    !matches!(call, Some(Call::Started | Call::Ended))
}

fn controllable(flags: u32) -> bool {
    flags & (1 << 13) != 0
}

fn call(value: i32) -> Option<Call> {
    Call::from_value(value)
}

fn cstr(s: Option<&'static CStr>) -> Option<&'static str> {
    s.map(|s| s.to_str().unwrap())
}

// --- enum values ---------------------------------------------------------

#[test]
fn steps_enum_values() {
    // the mirrors obs-source.c checks against libobs
    assert_eq!(CONTROLLABLE_MEDIA, 1 << 13);
    assert_eq!(MEDIA_STATE_NONE, 0);
    assert_eq!(action::NONE, 0);
    assert_eq!(action::PLAY_PAUSE, 1);
    assert_eq!(action::RESTART, 2);
    assert_eq!(action::STOP, 3);
    assert_eq!(action::NEXT, 4);
    assert_eq!(action::PREVIOUS, 5);
    assert_eq!(action::SET_TIME, 6);

    let calls = [
        (Call::PlayPause, 0),
        (Call::Restart, 1),
        (Call::Stop, 2),
        (Call::Next, 3),
        (Call::Previous, 4),
        (Call::GetDuration, 5),
        (Call::GetTime, 6),
        (Call::SetTime, 7),
        (Call::GetState, 8),
        (Call::Started, 9),
        (Call::Ended, 10),
    ];
    for (c, value) in calls {
        assert_eq!(c.value(), value);
        assert_eq!(Call::from_value(value), Some(c));
    }
    assert_eq!(Call::ALL.map(Call::value), calls.map(|(_, v)| v));

    assert_eq!(Admit::Reject.value(), 0);
    assert_eq!(Admit::Accept.value(), 1);
    assert_eq!(Admit::ReadFlags.value(), 2);
    assert_eq!(Admit::ReadCallback.value(), 3);

    let dispatches = [
        (Dispatch::Return, 0),
        (Dispatch::PlayPause, 1),
        (Dispatch::Restart, 2),
        (Dispatch::Stop, 3),
        (Dispatch::Next, 4),
        (Dispatch::Previous, 5),
        (Dispatch::SetTime, 6),
        (Dispatch::Skip, 7),
    ];
    for (d, value) in dispatches {
        assert_eq!(d.value(), value);
        assert_eq!(Dispatch::from_value(value), Some(d));
    }
    assert_eq!(
        Dispatch::ALL.map(Dispatch::value),
        dispatches.map(|(_, v)| v)
    );
}

// --- admission -----------------------------------------------------------

#[test]
fn steps_admit_valid() {
    assert_eq!(admit(false), Admit::Reject);
    assert_eq!(admit(true), Admit::ReadFlags);
}

#[test]
fn steps_admit_flags_every_call() {
    for c in Call::ALL {
        for flags in FLAGS {
            let expected = if !controllable(flags) {
                Admit::Reject
            } else if needs_callback(Some(c)) {
                Admit::ReadCallback
            } else {
                Admit::Accept
            };
            assert_eq!(admit_flags(Some(c), flags), expected, "{c:?} {flags:#x}");
        }
    }

    // every single bit: only OBS_SOURCE_CONTROLLABLE_MEDIA admits
    for bit in 0..32 {
        let flags = 1_u32 << bit;
        let (cb, accept) = if bit == 13 {
            (Admit::ReadCallback, Admit::Accept)
        } else {
            (Admit::Reject, Admit::Reject)
        };
        assert_eq!(admit_flags(Some(Call::PlayPause), flags), cb);
        assert_eq!(admit_flags(Some(Call::Started), flags), accept);
    }
}

#[test]
fn steps_admit_flags_outside_enum() {
    for value in OUTSIDE_CALLS {
        let c = call(value);
        assert_eq!(c, None);
        assert_eq!(admit_flags(c, CONTROLLABLE_MEDIA), Admit::ReadCallback);
        assert_eq!(admit_flags(c, u32::MAX), Admit::ReadCallback);
        assert_eq!(admit_flags(c, 0), Admit::Reject);
        assert_eq!(admit_flags(c, !CONTROLLABLE_MEDIA), Admit::Reject);
    }
}

#[test]
fn steps_admit_callback() {
    assert_eq!(admit_callback(true), Admit::Accept);
    assert_eq!(admit_callback(false), Admit::Reject);
}

/// The steps in the callers' order (`media_admit()` and the callback check
/// of `obs-source.c`) for one call: the answer and the values read.
fn admit_in_order(
    c: Option<Call>,
    valid: bool,
    flags: u32,
    has_callback: bool,
) -> (Admit, u32, u32) {
    let (mut flag_reads, mut callback_reads) = (0, 0);
    let mut step = admit(valid);
    if step == Admit::ReadFlags {
        flag_reads += 1;
        step = admit_flags(c, flags);
    }
    if step == Admit::ReadCallback {
        callback_reads += 1;
        step = admit_callback(has_callback);
    }
    (step, flag_reads, callback_reads)
}

#[test]
fn steps_admission_every_input() {
    for c in Call::ALL {
        for valid in [false, true] {
            for flags in FLAGS {
                for cb in [false, true] {
                    let (step, flag_reads, callback_reads) =
                        admit_in_order(Some(c), valid, flags, cb);
                    let accepted = valid && controllable(flags) && (!needs_callback(Some(c)) || cb);
                    assert_eq!(
                        step,
                        if accepted {
                            Admit::Accept
                        } else {
                            Admit::Reject
                        }
                    );
                    assert_eq!(flag_reads, u32::from(valid));
                    assert_eq!(
                        callback_reads,
                        u32::from(valid && controllable(flags) && needs_callback(Some(c)))
                    );
                }
            }
        }
    }

    // characterized, not endorsed: started and ended need no callback
    for c in [Call::Started, Call::Ended] {
        assert_eq!(
            admit_in_order(Some(c), true, CONTROLLABLE_MEDIA, false),
            (Admit::Accept, 1, 0)
        );
    }
}

// --- fallbacks -----------------------------------------------------------

#[test]
fn steps_getter_fallbacks() {
    assert_eq!(time_fallback(), 0);
    assert_eq!(state_fallback(), MEDIA_STATE_NONE);
}

// --- process_media_actions() ---------------------------------------------

#[test]
fn steps_dispatch_every_type() {
    assert_eq!(dispatch(action::NONE), Dispatch::Return);
    assert_eq!(dispatch(action::PLAY_PAUSE), Dispatch::PlayPause);
    assert_eq!(dispatch(action::RESTART), Dispatch::Restart);
    assert_eq!(dispatch(action::STOP), Dispatch::Stop);
    assert_eq!(dispatch(action::NEXT), Dispatch::Next);
    assert_eq!(dispatch(action::PREVIOUS), Dispatch::Previous);
    assert_eq!(dispatch(action::SET_TIME), Dispatch::SetTime);
}

#[test]
fn steps_dispatch_outside_enum() {
    // characterized, not endorsed: dropped, the next action is taken
    for t in OUTSIDE_TYPES {
        assert_eq!(dispatch(t), Dispatch::Skip);
    }
}

/// The C test's `SIGNALS`, by pause: false, true.
fn expected_signal(d: Dispatch) -> [Option<&'static str>; 2] {
    match d {
        Dispatch::PlayPause => [Some("media_play"), Some("media_pause")],
        Dispatch::Restart => [Some("media_restart"); 2],
        Dispatch::Stop => [Some("media_stopped"); 2],
        Dispatch::Next => [Some("media_next"); 2],
        Dispatch::Previous => [Some("media_previous"); 2],
        // characterized, not endorsed: a set-time emits nothing
        Dispatch::Return | Dispatch::SetTime | Dispatch::Skip => [None; 2],
    }
}

#[test]
fn steps_signal_every_dispatch() {
    for d in Dispatch::ALL {
        assert_eq!(signal_reads_pause(Some(d)), d == Dispatch::PlayPause);
        let [play, pause] = expected_signal(d);
        assert_eq!(cstr(signal(Some(d), false)), play, "{d:?}");
        assert_eq!(cstr(signal(Some(d), true)), pause, "{d:?}");
    }
}

#[test]
fn steps_signal_outside_enum() {
    for value in OUTSIDE_DISPATCHES {
        let d = Dispatch::from_value(value);
        assert_eq!(d, None);
        assert!(!signal_reads_pause(d));
        assert_eq!(signal(d, false), None);
        assert_eq!(signal(d, true), None);
    }
}

/// `process_media_actions()` written out with the port in its order: the
/// callback, then the signal, until the queue is empty or a
/// `MEDIA_ACTION_NONE` comes up.
fn run_queue(queue: &[(i32, bool)]) -> String {
    let mut trace = Vec::new();
    let mut next = queue.iter();
    loop {
        let (t, pause) = next.next().copied().unwrap_or((action::NONE, false));
        let d = dispatch(t);
        let callback = match d {
            Dispatch::Return => {
                trace.push("return");
                return trace.join(" ");
            }
            Dispatch::PlayPause => Some("play_pause"),
            Dispatch::Restart => Some("restart"),
            Dispatch::Stop => Some("stop"),
            Dispatch::Next => Some("next"),
            Dispatch::Previous => Some("previous"),
            Dispatch::SetTime => Some("set_time"),
            Dispatch::Skip => None,
        };
        trace.extend(callback);
        trace.extend(cstr(signal(Some(d), signal_reads_pause(Some(d)) && pause)));
    }
}

#[test]
fn steps_process_every_action() {
    let one = [
        ((action::PLAY_PAUSE, false), "play_pause media_play return"),
        ((action::PLAY_PAUSE, true), "play_pause media_pause return"),
        ((action::RESTART, false), "restart media_restart return"),
        ((action::STOP, false), "stop media_stopped return"),
        ((action::NEXT, false), "next media_next return"),
        ((action::PREVIOUS, false), "previous media_previous return"),
        ((action::SET_TIME, false), "set_time return"),
        // the pause is not read for any other action
        ((action::RESTART, true), "restart media_restart return"),
        ((action::SET_TIME, true), "set_time return"),
        // characterized, not endorsed: skipped, the loop goes on
        ((7, true), "return"),
        ((-1, false), "return"),
        ((i32::MAX, false), "return"),
        // a queued MEDIA_ACTION_NONE ends the loop like an empty queue
        ((action::NONE, false), "return"),
    ];
    for (a, trace) in one {
        assert_eq!(run_queue(&[a]), trace, "{a:?}");
    }

    assert_eq!(run_queue(&[]), "return");

    let mixed = [
        (action::STOP, false),
        (99, true),
        (action::PLAY_PAUSE, true),
        (action::SET_TIME, false),
        (action::NONE, false),
        (action::NEXT, false),
    ];
    assert_eq!(
        run_queue(&mixed),
        "stop media_stopped play_pause media_pause set_time return"
    );
}

// --- edge cases ----------------------------------------------------------

/// Values around every enum and the ends of `i32`.
fn edge_values() -> impl Iterator<Item = i32> {
    (-1000..=1000).chain([
        i32::MIN,
        i32::MIN + 1,
        i32::MAX - 1,
        i32::MAX,
        1 << 13,
        -(1 << 13),
        0x100,
        0x1_0000,
    ])
}

/// [`admit_flags`] written out.
fn model_admit_flags(c: Option<Call>, flags: u32) -> Admit {
    if !controllable(flags) {
        Admit::Reject
    } else if needs_callback(c) {
        Admit::ReadCallback
    } else {
        Admit::Accept
    }
}

#[test]
fn edge_call_values() {
    for value in edge_values() {
        let c = call(value);
        assert_eq!(c.is_some(), (0..=10).contains(&value), "{value}");
        if let Some(c) = c {
            assert_eq!(c.value(), value);
        }
    }
}

#[test]
fn edge_admit_flags_every_bit() {
    for value in edge_values() {
        let c = call(value);
        for bit in 0..32 {
            let flags = 1_u32 << bit;
            assert_eq!(admit_flags(c, flags), model_admit_flags(c, flags));
            assert_eq!(admit_flags(c, !flags), model_admit_flags(c, !flags));
        }
    }
}

#[test]
fn edge_admission_reads_in_order() {
    for value in edge_values() {
        let c = call(value);
        for valid in [false, true] {
            for flags in FLAGS {
                for cb in [false, true] {
                    let (step, flag_reads, callback_reads) = admit_in_order(c, valid, flags, cb);
                    // never READ_* at the end: every step is answered
                    assert!(matches!(step, Admit::Accept | Admit::Reject));
                    assert!(flag_reads <= u32::from(valid));
                    assert!(callback_reads <= flag_reads);
                    let accepted = valid && controllable(flags) && (!needs_callback(c) || cb);
                    assert_eq!(
                        step == Admit::Accept,
                        accepted,
                        "{value} {valid} {flags:#x} {cb}"
                    );
                }
            }
        }
    }
}

/// [`dispatch`] written out from `enum media_action_type`.
fn model_dispatch(t: i32) -> Dispatch {
    match t {
        0 => Dispatch::Return,
        1 => Dispatch::PlayPause,
        2 => Dispatch::Restart,
        3 => Dispatch::Stop,
        4 => Dispatch::Next,
        5 => Dispatch::Previous,
        6 => Dispatch::SetTime,
        _ => Dispatch::Skip,
    }
}

#[test]
fn edge_dispatch_and_signal_values() {
    for value in edge_values() {
        assert_eq!(dispatch(value), model_dispatch(value), "{value}");

        let d = Dispatch::from_value(value);
        assert_eq!(d.is_some(), (0..=7).contains(&value), "{value}");
        if let Some(d) = d {
            assert_eq!(d.value(), value);
        }
        assert_eq!(signal_reads_pause(d), d == Some(Dispatch::PlayPause));
        for pause in [false, true] {
            let expected = d.and_then(|d| expected_signal(d)[usize::from(pause)]);
            assert_eq!(cstr(signal(d, pause)), expected, "{value} {pause}");
        }
    }
}

#[test]
fn edge_signal_names_are_static() {
    // the same pointer every time: the C returns string literals and the
    // signal handler looks them up by name
    for d in Dispatch::ALL {
        for pause in [false, true] {
            let a = signal(Some(d), pause).map(CStr::as_ptr);
            let b = signal(Some(d), pause).map(CStr::as_ptr);
            assert_eq!(a, b);
        }
    }
}

/// `process_media_actions()` written out from the C, for the proptest.
fn model_queue(queue: &[(i32, bool)]) -> String {
    let mut trace = Vec::new();
    for &(t, pause) in queue.iter().chain(std::iter::once(&(0, false))) {
        let (callback, sig) = match t {
            0 => {
                trace.push("return");
                return trace.join(" ");
            }
            1 => (
                Some("play_pause"),
                Some(if pause { "media_pause" } else { "media_play" }),
            ),
            2 => (Some("restart"), Some("media_restart")),
            3 => (Some("stop"), Some("media_stopped")),
            4 => (Some("next"), Some("media_next")),
            5 => (Some("previous"), Some("media_previous")),
            6 => (Some("set_time"), None),
            _ => (None, None),
        };
        trace.extend(callback);
        trace.extend(sig);
    }
    unreachable!("the queue ends in MEDIA_ACTION_NONE")
}

fn any_type() -> impl Strategy<Value = i32> {
    prop_oneof![0..8_i32, -2..2_i32, any::<i32>()]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4000))]

    #[test]
    fn edge_proptest_admission(value in prop_oneof![0..11_i32, any::<i32>()], valid: bool,
                               flags: u32, cb: bool) {
        let c = call(value);
        prop_assert_eq!(admit_flags(c, flags), model_admit_flags(c, flags));
        let (step, flag_reads, callback_reads) = admit_in_order(c, valid, flags, cb);
        let accepted = valid && controllable(flags) && (!needs_callback(c) || cb);
        prop_assert_eq!(step == Admit::Accept, accepted);
        prop_assert_eq!(flag_reads, u32::from(valid));
        prop_assert_eq!(callback_reads, u32::from(valid && controllable(flags) && needs_callback(c)));
    }

    #[test]
    fn edge_proptest_dispatch(t in any_type(), pause: bool) {
        let d = dispatch(t);
        prop_assert_eq!(d, model_dispatch(t));
        prop_assert_eq!(signal_reads_pause(Some(d)), d == Dispatch::PlayPause);
        prop_assert_eq!(cstr(signal(Some(d), pause)), expected_signal(d)[usize::from(pause)]);
    }

    #[test]
    fn edge_proptest_queue(queue in proptest::collection::vec((any_type(), any::<bool>()), 0..12)) {
        prop_assert_eq!(run_queue(&queue), model_queue(&queue));
    }
}
