//! Tier 3: the `obs_source_media_*` admission shims against the original C
//! (`obs-c-oracle`, `oracle/source_media_admission.c`: the unmodified
//! `libobs/obs-source-media-admission.c.h` with its functions renamed).
//!
//! Every function takes and returns plain values, so a run compares the two
//! results: the C's `bool` as the byte it returns, the enums as their
//! values, the signal as the bytes of its name (or both NULL).
//!
//! Runs:
//! - the enum values, the mirrors of `OBS_SOURCE_CONTROLLABLE_MEDIA` and
//!   `OBS_MEDIA_STATE_NONE` against obs.h, and the sizes, against the C
//!   headers; the fallbacks;
//! - admission: the validity step; the flag step for every call value from
//!   -70000 to 70000, every 65521st `i32` and the ends, with no flag, the
//!   flag alone, every bit and every bit but it; every call value from -20
//!   to 20 and the ends with each of the 32 bits alone and every bit but
//!   it; the callback step;
//! - dispatch and signal: every action type from -70000 to 70000, every
//!   65521st `i32` and the ends; every dispatch value over the same range
//!   with both pauses;
//! - the callers' order, one copy on each side: the steps of each call
//!   (both validities, 10 flag sets, both callbacks) with the values each
//!   step reads, and `process_media_actions()` over every queued type from
//!   -3 to 10 and the ends, alone and in pairs, with both pauses;
//! - proptests (20,000 cases each): admission over any call value, flags
//!   and inputs; dispatch and signal over any type and dispatch value; the
//!   callers' order over random queues of any type.
//!
//! No input reaches an integer division or a 128-bit multiply on either
//! side, so the MSVC x64 `_udiv128` and `INT_MIN / -1` faults cannot come
//! up and no input is skipped there. No intentional differences: there is
//! no pointer to check.

use std::ffi::CStr;
use std::os::raw::{c_char, c_int};

use obs_c_oracle::source_media_admission as oracle;
use obs_source::ffi::source_media_admission as shim;
use obs_source::media_admission::{
    Admit, CONTROLLABLE_MEDIA, Call, Dispatch, MEDIA_STATE_NONE, action,
};
use proptest::prelude::*;

#[test]
fn values_match_the_c_headers() {
    let mut rust: Vec<i64> = vec![
        CONTROLLABLE_MEDIA.into(),
        CONTROLLABLE_MEDIA.into(),
        MEDIA_STATE_NONE.into(),
        MEDIA_STATE_NONE.into(),
        action::NONE.into(),
        action::PLAY_PAUSE.into(),
        action::RESTART.into(),
        action::STOP.into(),
        action::NEXT.into(),
        action::PREVIOUS.into(),
        action::SET_TIME.into(),
    ];
    rust.extend(Call::ALL.map(|c| i64::from(c.value())));
    rust.extend(
        [
            Admit::Reject,
            Admit::Accept,
            Admit::ReadFlags,
            Admit::ReadCallback,
        ]
        .map(|a| i64::from(a.value())),
    );
    rust.extend(Dispatch::ALL.map(|d| i64::from(d.value())));
    let int = i64::try_from(std::mem::size_of::<c_int>()).unwrap();
    rust.extend([int, int, int, int, int, 1]);
    assert_eq!(rust.len(), oracle::VALUE_NAMES.len());

    for (which, (name, value)) in oracle::VALUE_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: a pure lookup.
        let c = unsafe { oracle::oracle_source_media_admission_value(which) };
        assert_eq!(value, c, "{name}");
    }
    // SAFETY: a pure lookup, one past the end.
    let past = unsafe { oracle::oracle_source_media_admission_value(oracle::VALUE_NAMES.len()) };
    assert_eq!(past, i64::MIN, "the list has as many values as names");
}

#[test]
fn fallbacks() {
    // SAFETY: no arguments.
    let (time, state) = unsafe {
        (
            oracle::oracle_obs_source_media_time_fallback(),
            oracle::oracle_obs_source_media_state_fallback(),
        )
    };
    assert_eq!(shim::obs_source_media_time_fallback(), time);
    assert_eq!(shim::obs_source_media_state_fallback(), state);
}

// --------------------------------------------------------------------------
// One call on each side

fn check_admit(valid: bool) {
    // SAFETY: plain values.
    let c = unsafe { oracle::oracle_obs_source_media_admit(valid) };
    assert_eq!(shim::obs_source_media_admit(valid), c, "admit({valid})");
}

fn check_admit_flags(call: c_int, flags: u32) {
    // SAFETY: plain values; the C takes any int for the enum.
    let c = unsafe { oracle::oracle_obs_source_media_admit_flags(call, flags) };
    assert_eq!(
        shim::obs_source_media_admit_flags(call, flags),
        c,
        "admit_flags({call}, {flags:#x})"
    );
}

fn check_admit_callback(has_callback: bool) {
    // SAFETY: plain values.
    let c = unsafe { oracle::oracle_obs_source_media_admit_callback(has_callback) };
    assert_eq!(
        shim::obs_source_media_admit_callback(has_callback),
        c,
        "admit_callback({has_callback})"
    );
}

fn check_dispatch(action_type: c_int) {
    // SAFETY: plain values.
    let c = unsafe { oracle::oracle_obs_source_media_dispatch(action_type) };
    assert_eq!(
        shim::obs_source_media_dispatch(action_type),
        c,
        "dispatch({action_type})"
    );
}

/// The name a signal pointer points at, `None` for NULL.
fn name(ptr: *const c_char) -> Option<Vec<u8>> {
    // SAFETY: both sides return NULL or a static NUL-terminated string.
    (!ptr.is_null()).then(|| unsafe { CStr::from_ptr(ptr) }.to_bytes().to_vec())
}

fn c_signal(dispatch: c_int, pause: bool) -> Option<Vec<u8>> {
    // SAFETY: plain values; the result is NULL or a string literal.
    name(unsafe { oracle::oracle_obs_source_media_signal(dispatch, pause) })
}

fn rs_signal(dispatch: c_int, pause: bool) -> Option<Vec<u8>> {
    name(shim::obs_source_media_signal(dispatch, pause))
}

fn check_signal(dispatch: c_int) {
    // SAFETY: plain values.
    let c = unsafe { oracle::oracle_obs_source_media_signal_reads_pause(dispatch) };
    assert_eq!(
        u8::from(shim::obs_source_media_signal_reads_pause(dispatch)),
        c,
        "signal_reads_pause({dispatch})"
    );
    for pause in [false, true] {
        assert_eq!(
            rs_signal(dispatch, pause),
            c_signal(dispatch, pause),
            "signal({dispatch}, {pause})"
        );
    }
}

/// -70000 to 70000, every 65521st `i32` and the ends.
fn wide_values() -> impl Iterator<Item = c_int> {
    (-70_000..=70_000)
        .chain((i32::MIN..=i32::MAX).step_by(65_521))
        .chain([i32::MIN, i32::MIN + 1, i32::MAX - 1, i32::MAX])
}

const FLAG_SETS: [u32; 10] = [
    0,
    CONTROLLABLE_MEDIA,
    u32::MAX,
    !CONTROLLABLE_MEDIA,
    CONTROLLABLE_MEDIA << 1,
    CONTROLLABLE_MEDIA >> 1,
    0b111,
    CONTROLLABLE_MEDIA | 0b111,
    0x8000_0000,
    CONTROLLABLE_MEDIA | 0x8000_0000,
];

#[test]
fn admission_every_value() {
    for valid in [false, true] {
        check_admit(valid);
        check_admit_callback(valid);
    }
    for call in wide_values() {
        for flags in [0, CONTROLLABLE_MEDIA, u32::MAX, !CONTROLLABLE_MEDIA] {
            check_admit_flags(call, flags);
        }
    }
    for call in (-20..=20).chain([i32::MIN, i32::MAX]) {
        for bit in 0..32 {
            check_admit_flags(call, 1 << bit);
            check_admit_flags(call, !(1 << bit));
        }
    }
}

#[test]
fn dispatch_and_signal_every_value() {
    for value in wide_values() {
        check_dispatch(value);
        check_signal(value);
    }
}

// --------------------------------------------------------------------------
// The callers' order, one copy on each side

/// The admission steps of one call in `media_admit()` and the callback
/// check's order, with the values read, through the shims (`rs`) or the
/// oracle.
fn admit_in_order(rs: bool, call: c_int, valid: bool, flags: u32, has_callback: bool) -> String {
    let admit = |v| {
        if rs {
            shim::obs_source_media_admit(v)
        } else {
            // SAFETY: plain values.
            unsafe { oracle::oracle_obs_source_media_admit(v) }
        }
    };
    let admit_flags = |c, f| {
        if rs {
            shim::obs_source_media_admit_flags(c, f)
        } else {
            // SAFETY: plain values.
            unsafe { oracle::oracle_obs_source_media_admit_flags(c, f) }
        }
    };
    let admit_callback = |cb| {
        if rs {
            shim::obs_source_media_admit_callback(cb)
        } else {
            // SAFETY: plain values.
            unsafe { oracle::oracle_obs_source_media_admit_callback(cb) }
        }
    };
    let read_flags = Admit::ReadFlags.value();
    let read_callback = Admit::ReadCallback.value();

    let mut trace = Vec::new();
    let mut step = admit(valid);
    if step == read_flags {
        trace.push("flags".to_owned());
        step = admit_flags(call, flags);
    }
    if step == read_callback {
        trace.push("callback".to_owned());
        step = admit_callback(has_callback);
    }
    trace.push(format!("= {step}"));
    trace.join(" ")
}

fn check_admit_in_order(call: c_int, valid: bool, flags: u32, has_callback: bool) {
    assert_eq!(
        admit_in_order(true, call, valid, flags, has_callback),
        admit_in_order(false, call, valid, flags, has_callback),
        "call {call} valid {valid} flags {flags:#x} callback {has_callback}"
    );
}

/// `process_media_actions()` over a queue in its order, the callback, then
/// the signal, through the shims (`rs`) or the oracle.
fn run_queue(rs: bool, queue: &[(c_int, bool)]) -> String {
    let dispatch = |t| {
        if rs {
            shim::obs_source_media_dispatch(t)
        } else {
            // SAFETY: plain values.
            unsafe { oracle::oracle_obs_source_media_dispatch(t) }
        }
    };
    let reads_pause = |d| {
        if rs {
            shim::obs_source_media_signal_reads_pause(d)
        } else {
            // SAFETY: plain values.
            unsafe { oracle::oracle_obs_source_media_signal_reads_pause(d) != 0 }
        }
    };
    let signal = |d, p| if rs { rs_signal(d, p) } else { c_signal(d, p) };

    let mut trace = Vec::new();
    for &(t, pause) in queue.iter().chain(std::iter::once(&(action::NONE, false))) {
        let d = dispatch(t);
        trace.push(format!("dispatch {d}"));
        if d == Dispatch::Return.value() {
            return trace.join(" ");
        }
        if let Some(name) = signal(d, reads_pause(d) && pause) {
            trace.push(String::from_utf8(name).unwrap());
        }
    }
    unreachable!("the queue ends in MEDIA_ACTION_NONE")
}

fn check_queue(queue: &[(c_int, bool)]) {
    assert_eq!(run_queue(true, queue), run_queue(false, queue), "{queue:?}");
}

#[test]
fn callers_order() {
    for call in (-3..=14).chain([i32::MIN, i32::MAX]) {
        for valid in [false, true] {
            for flags in FLAG_SETS {
                for cb in [false, true] {
                    check_admit_in_order(call, valid, flags, cb);
                }
            }
        }
    }

    let types: Vec<c_int> = (-3..=10).chain([i32::MIN, i32::MAX]).collect();
    check_queue(&[]);
    for &a in &types {
        for pa in [false, true] {
            check_queue(&[(a, pa)]);
            for &b in &types {
                for pb in [false, true] {
                    check_queue(&[(a, pa), (b, pb)]);
                }
            }
        }
    }
}

fn any_type() -> impl Strategy<Value = c_int> {
    prop_oneof![0..8_i32, -2..12_i32, any::<i32>()]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20_000))]

    #[test]
    fn proptest_admission(call in prop_oneof![0..11_i32, any::<i32>()], valid: bool,
                          flags in prop_oneof![any::<u32>(), Just(CONTROLLABLE_MEDIA)],
                          cb: bool) {
        check_admit_flags(call, flags);
        check_admit_in_order(call, valid, flags, cb);
    }

    #[test]
    fn proptest_dispatch_and_signal(t in any_type(), d in any_type()) {
        check_dispatch(t);
        check_signal(d);
    }

    #[test]
    fn proptest_queue(queue in proptest::collection::vec((any_type(), any::<bool>()), 0..12)) {
        check_queue(&queue);
    }
}
