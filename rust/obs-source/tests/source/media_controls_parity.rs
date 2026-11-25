//! Tier 3: the original C `obs_source_media_*()` control calls against the
//! Rust exports, on noise-filled real-header sources with a real
//! `media_actions` queue: every lock, unlock and `obs_source_dosignal()`
//! call in order (which mutex, the queue length at the time, the global and
//! local signal names, the source), the appended action's type and value,
//! the callbacks (never called), every other byte, and the NULL log lines.
//! No intentional differences or excluded inputs. Each run allocates one
//! fixture and at most 32 queued actions.
use core::ffi::c_void;
use obs_source::ffi::source_media_controls::{
    Live, obs_source_media_ended, obs_source_media_next, obs_source_media_play_pause,
    obs_source_media_previous, obs_source_media_restart, obs_source_media_set_time,
    obs_source_media_started, obs_source_media_stop,
};
use obs_source::media_admission::{CONTROLLABLE_MEDIA, Call};
use obs_source::media_controls::{Action, ENDED_SIGNAL, STARTED_SIGNAL, Source};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type Plain = unsafe extern "C" fn(*mut c_void);
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);

/// `struct media_controls_calls` of the oracle.
#[repr(C)]
#[derive(Clone, Copy)]
struct Calls {
    play_pause: unsafe extern "C" fn(*mut c_void, bool),
    restart: Plain,
    stop: Plain,
    next: Plain,
    previous: Plain,
    set_time: unsafe extern "C" fn(*mut c_void, i64),
    started: Plain,
    ended: Plain,
}

const OPS: usize = 32;
const EVENTS: usize = 4;
const TRACE: usize = 1 + 2 * EVENTS + 5;

unsafe extern "C" {
    fn oracle_source_media_controls_run(
        calls: *const Calls,
        noise: u8,
        output_flags: u32,
        data: bool,
        callbacks: u32,
        kinds: *const u8,
        values: *const i64,
        count: usize,
        trace: *mut u64,
    ) -> bool;
    fn oracle_source_media_controls_originals(calls: *mut Calls);
    fn oracle_source_media_controls_null_logs(
        calls: *const Calls,
        get: GetLog,
        set: SetLog,
    ) -> bool;
}

const RUST: Calls = Calls {
    play_pause: obs_source_media_play_pause,
    restart: obs_source_media_restart,
    stop: obs_source_media_stop,
    next: obs_source_media_next,
    previous: obs_source_media_previous,
    set_time: obs_source_media_set_time,
    started: obs_source_media_started,
    ended: obs_source_media_ended,
};

fn originals() -> Calls {
    let mut calls = RUST;
    // SAFETY: the oracle writes all eight function pointers.
    unsafe { oracle_source_media_controls_originals(&raw mut calls) };
    calls
}

/// The callbacks bit of every queueing call.
const ALL_CALLBACKS: u32 = (1 << 0) | (1 << 1) | (1 << 2) | (1 << 3) | (1 << 4) | (1 << 7);

#[derive(Clone, Copy, Debug)]
struct Run<'a> {
    noise: u8,
    output_flags: u32,
    data: bool,
    callbacks: u32,
    ops: &'a [(u8, i64)],
}

/// The trace of one run with the given calls; `None` when the oracle found
/// a byte other than the queue and its mutex written (or could not build
/// the fixture, or an adapter missed its field).
fn trace(calls: Calls, run: Run<'_>) -> Option<Vec<u64>> {
    assert!(run.ops.len() <= OPS);
    let kinds: Vec<u8> = run.ops.iter().map(|op| op.0).collect();
    let values: Vec<i64> = run.ops.iter().map(|op| op.1).collect();
    let mut trace = vec![0; run.ops.len() * TRACE];
    // SAFETY: the C fixture owns a real-layout source and queue; kinds and
    // values hold count entries and trace TRACE per operation; every call
    // has the original ABI.
    let valid = unsafe {
        oracle_source_media_controls_run(
            &raw const calls,
            run.noise,
            run.output_flags,
            run.data,
            run.callbacks,
            kinds.as_ptr(),
            values.as_ptr(),
            run.ops.len(),
            trace.as_mut_ptr(),
        )
    };
    valid.then_some(trace)
}

fn compare(run: Run<'_>) {
    let original = trace(originals(), run).unwrap_or_else(|| panic!("C: {run:?}"));
    let rust = trace(RUST, run).unwrap_or_else(|| panic!("Rust: {run:?}"));
    assert_eq!(rust, original, "{run:?}");
}

/// Every call, with pause both ways and set_time over the `i64` edges.
const SEQUENCE: [(u8, i64); 14] = [
    (0, 1),
    (0, 0),
    (1, 0),
    (2, 0),
    (3, 0),
    (4, 0),
    (5, 0),
    (5, -1),
    (5, i64::MIN),
    (5, i64::MAX),
    (5, 0x1_2345_6789),
    (6, 0),
    (7, 0),
    (0, 3),
];

#[test]
fn calls_match_c_over_the_grid() {
    for noise in [0, 0x5a, 0xff] {
        for output_flags in [0, CONTROLLABLE_MEDIA, !CONTROLLABLE_MEDIA, u32::MAX, 1 | 4] {
            for data in [false, true] {
                for callbacks in [0, ALL_CALLBACKS, 1, 1 << 7, 1 << 5, 0x5a & ALL_CALLBACKS] {
                    compare(Run {
                        noise,
                        output_flags,
                        data,
                        callbacks,
                        ops: &SEQUENCE,
                    });
                }
            }
        }
    }
}

#[test]
fn pinned_answers() {
    let run = Run {
        noise: 0,
        output_flags: CONTROLLABLE_MEDIA,
        data: true,
        callbacks: ALL_CALLBACKS,
        ops: &[(5, -7), (6, 0), (0, 1)],
    };
    let t = trace(RUST, run).unwrap();
    // set_time: the source's mutex locked with an empty queue, unlocked
    // with one action, a SET_TIME (6) of -7 appended, no callback.
    assert_eq!(&t[..5], [2, 1 | 0x20, 0, 2 | 0x20, 1]);
    let after = &t[1 + 2 * EVENTS..TRACE];
    assert_eq!(after, [1, 6, (-7_i64).cast_unsigned(), 0, 1]);
    // started: one obs_source_dosignal() of the source with no global name
    // and media_started; the queue untouched.
    let t = &t[TRACE..];
    assert_eq!(&t[..3], [1, 3 | 0x10 | 0x20, 1]);
    assert_eq!(&t[1 + 2 * EVENTS..TRACE], [1, 0xffff, 0, 0, 1]);
    // play_pause(true) appends after it.
    let t = &t[TRACE..];
    assert_eq!(&t[1 + 2 * EVENTS..TRACE], [2, 1, 1, 0, 1]);
}

/// Mutants over the real adapters; each must disagree with the original C
/// somewhere. They run only on the oracle's live fixture source.
fn fixture(source: *mut c_void) -> Live {
    assert!(!source.is_null());
    // SAFETY: the oracle passes its live fixture source, which outlives the
    // call.
    unsafe { Live::new(source) }
}
unsafe extern "C" fn play_pause_without_callback(source: *mut c_void, pause: bool) {
    let s = fixture(source);
    if s.has_data() && s.output_flags() & CONTROLLABLE_MEDIA != 0 {
        s.queue(Action::PlayPause(pause));
    }
}
unsafe extern "C" fn play_pause_ignores_pause(source: *mut c_void, _pause: bool) {
    let s = fixture(source);
    if s.has_data() && s.output_flags() & CONTROLLABLE_MEDIA != 0 && s.has_callback(Call::PlayPause)
    {
        s.queue(Action::PlayPause(false));
    }
}
unsafe extern "C" fn restart_without_data(source: *mut c_void) {
    let s = fixture(source);
    if s.output_flags() & CONTROLLABLE_MEDIA != 0 && s.has_callback(Call::Restart) {
        s.queue(Action::Restart);
    }
}
unsafe extern "C" fn stop_reads_restart_callback(source: *mut c_void) {
    let s = fixture(source);
    if s.has_data() && s.output_flags() & CONTROLLABLE_MEDIA != 0 && s.has_callback(Call::Restart) {
        s.queue(Action::Stop);
    }
}
unsafe extern "C" fn next_queues_previous(source: *mut c_void) {
    let s = fixture(source);
    if s.has_data() && s.output_flags() & CONTROLLABLE_MEDIA != 0 && s.has_callback(Call::Next) {
        s.queue(Action::Previous);
    }
}
unsafe extern "C" fn set_time_truncated(source: *mut c_void, ms: i64) {
    let s = fixture(source);
    if s.has_data() && s.output_flags() & CONTROLLABLE_MEDIA != 0 && s.has_callback(Call::SetTime) {
        s.queue(Action::SetTime(ms & 0xffff_ffff));
    }
}
unsafe extern "C" fn started_needs_data(source: *mut c_void) {
    let s = fixture(source);
    if s.has_data() && s.output_flags() & CONTROLLABLE_MEDIA != 0 {
        s.signal(STARTED_SIGNAL);
    }
}
unsafe extern "C" fn ended_needs_callback(source: *mut c_void) {
    let s = fixture(source);
    if s.output_flags() & CONTROLLABLE_MEDIA != 0 && s.has_callback(Call::Ended) {
        s.signal(ENDED_SIGNAL);
    }
}
unsafe extern "C" fn ended_signals_started(source: *mut c_void) {
    let s = fixture(source);
    if s.output_flags() & CONTROLLABLE_MEDIA != 0 {
        s.signal(STARTED_SIGNAL);
    }
}

#[test]
fn rejects_mutants() {
    let mutants = [
        Calls {
            play_pause: play_pause_without_callback,
            ..RUST
        },
        Calls {
            play_pause: play_pause_ignores_pause,
            ..RUST
        },
        Calls {
            restart: restart_without_data,
            ..RUST
        },
        Calls {
            stop: stop_reads_restart_callback,
            ..RUST
        },
        Calls {
            next: next_queues_previous,
            ..RUST
        },
        Calls {
            set_time: set_time_truncated,
            ..RUST
        },
        Calls {
            started: started_needs_data,
            ..RUST
        },
        Calls {
            ended: ended_needs_callback,
            ..RUST
        },
        Calls {
            ended: ended_signals_started,
            ..RUST
        },
    ];
    for (i, mutant) in mutants.into_iter().enumerate() {
        let caught = [false, true].into_iter().any(|data| {
            [0, ALL_CALLBACKS, 1 << 1].into_iter().any(|callbacks| {
                let run = Run {
                    noise: 0x5a,
                    output_flags: CONTROLLABLE_MEDIA,
                    data,
                    callbacks,
                    ops: &SEQUENCE,
                };
                trace(mutant, run) != trace(originals(), run)
            })
        });
        assert!(caught, "mutant {i} agreed with the original C");
    }
}

#[test]
fn null_diagnostics_match_c() {
    if !crate::output::isolated(concat!(module_path!(), "::null_diagnostics_match_c")) {
        return;
    }
    // SAFETY: the isolated child owns the process log handler, which the C
    // probe restores; the callbacks use the base shim's opaque pointer ABI.
    let calls = RUST;
    unsafe {
        assert!(oracle_source_media_controls_null_logs(
            &raw const calls,
            base_get_log_handler,
            base_set_log_handler,
        ));
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn arbitrary_runs_match_c(
        noise in any::<u8>(),
        output_flags in prop_oneof![Just(0_u32), Just(CONTROLLABLE_MEDIA), any::<u32>()],
        data in any::<bool>(),
        callbacks in 0..=0xff_u32,
        ops in prop::collection::vec((0..8_u8, any::<i64>()), 0..=OPS),
    ) {
        compare(Run { noise, output_flags, data, callbacks, ops: &ops });
    }
}
