//! Tier 3: the shims against the original `libobs/obs-audio-discard.c.h`.
//!
//! The oracle (`rust/obs-c-oracle/oracle/audio_discard.c`) is the C file,
//! unmodified, with its three functions renamed. Both sides get the same
//! source fields and input deques (C `struct deque`s with bmem memory,
//! built the same way) through their C signatures, and are compared: the
//! step, the five fields (`audio_ts`, `last_size`, `pending`,
//! `pending_stop`, `timing_set`), and for each of the 8 deques the
//! `struct deque` fields (size, start, end, capacity) and every sample
//! left in it.
//!
//! Runs: the layout; chosen cases for each path of the three functions;
//! 20 000 random cases per function (0 to 8 channels, 0 to 3000 frames,
//! channels 1 to 7 holding at least channel 0's frames, timestamps before,
//! on, inside, at the end of and after one- and two-tick windows and near 0
//! and `u64::MAX`, rates from 8 kHz to 384 kHz and odd ones, the stop check
//! state at random and equal to the buffer).
//!
//! Excluded, the intentional differences, tested on the shim alone below: a
//! sample rate of 0 in `obs_audio_discard_ignore` (the C divides by 0); a
//! channel holding less than channel 0's count (the C underflows the
//! deque's size); null pointers. `test_audio_buffering_subsample` compiles
//! the C into itself, so this is what compares the swap; the real-mixer
//! tests run it through libobs.

use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use core::ptr;

use obs_c_oracle::audio_discard::{
    LAYOUT_NAMES, OracleAudioDiscardStep, OracleAudioIgnoreStep, oracle_audio_discard_layout,
    oracle_obs_audio_discard_if_stopped, oracle_obs_audio_discard_ignore,
    oracle_obs_audio_discard_tick,
};
use obs_core::audio_mix::{AUDIO_OUTPUT_FRAMES, MAX_AUDIO_CHANNELS};
use obs_core::ffi::audio_discard::{
    OBS_AUDIO_DISCARD_AHEAD, OBS_AUDIO_DISCARD_BEHIND, OBS_AUDIO_DISCARD_PENDING,
    OBS_AUDIO_DISCARD_RENDER_SOURCE, OBS_AUDIO_DISCARD_START_AT_END, OBS_AUDIO_STOP_CLEARED,
    OBS_AUDIO_STOP_NONE, OBS_AUDIO_STOP_PENDING, obs_audio_discard_if_stopped,
    obs_audio_discard_ignore, obs_audio_discard_step, obs_audio_discard_tick,
    obs_audio_ignore_step, obs_audio_input,
};
use obs_core::ffi::audio_mix::ts_info;
use obs_media_io::audio_io::audio_frames_to_ns;
use obs_util::ffi::deque::{deque, deque_free, deque_pop_front, deque_push_back};
use proptest::prelude::*;

const FRAMES: u64 = AUDIO_OUTPUT_FRAMES as u64;
const START: u64 = 1_000_000_000;
const F32: usize = 4;

#[test]
fn layout() {
    let values: [usize; 31] = [
        size_of::<obs_audio_input>(),
        align_of::<obs_audio_input>(),
        offset_of!(obs_audio_input, buf),
        offset_of!(obs_audio_input, audio_ts),
        offset_of!(obs_audio_input, last_size),
        offset_of!(obs_audio_input, pending),
        offset_of!(obs_audio_input, pending_stop),
        offset_of!(obs_audio_input, timing_set),
        size_of::<obs_audio_ignore_step>(),
        align_of::<obs_audio_ignore_step>(),
        offset_of!(obs_audio_ignore_step, rerender),
        offset_of!(obs_audio_ignore_step, no_timestamp),
        offset_of!(obs_audio_ignore_step, buffered),
        offset_of!(obs_audio_ignore_step, dropped),
        offset_of!(obs_audio_ignore_step, lag),
        offset_of!(obs_audio_ignore_step, warn),
        offset_of!(obs_audio_ignore_step, warn_lag),
        size_of::<obs_audio_discard_step>(),
        align_of::<obs_audio_discard_step>(),
        offset_of!(obs_audio_discard_step, path),
        offset_of!(obs_audio_discard_step, stop),
        size_of::<i32>(),
        size_of::<i32>(),
        OBS_AUDIO_STOP_NONE as usize,
        OBS_AUDIO_STOP_PENDING as usize,
        OBS_AUDIO_STOP_CLEARED as usize,
        OBS_AUDIO_DISCARD_RENDER_SOURCE as usize,
        OBS_AUDIO_DISCARD_AHEAD as usize,
        OBS_AUDIO_DISCARD_BEHIND as usize,
        OBS_AUDIO_DISCARD_START_AT_END as usize,
        OBS_AUDIO_DISCARD_PENDING as usize,
    ];
    for (which, (name, value)) in LAYOUT_NAMES.iter().zip(values).enumerate() {
        // SAFETY: a pure C function.
        assert_eq!(
            unsafe { oracle_audio_discard_layout(which) },
            value,
            "{name}"
        );
    }
}

/// The source fields and input deques one side starts a call with.
#[derive(Debug, Clone)]
struct Case {
    op: Op,
    channels: usize,
    /// Frames in each of the 8 deques.
    frames: [usize; MAX_AUDIO_CHANNELS],
    audio_ts: u64,
    last_size: usize,
    pending: bool,
    pending_stop: bool,
    timing_set: bool,
    rate: usize,
}

#[derive(Debug, Clone, Copy)]
enum Op {
    Ignore { start_ts: u64 },
    IfStopped,
    Tick { audio_render: bool, ts: ts_info },
}

/// One side's source: the fields and 8 deques, filled with ch * 10000 + i.
struct Source {
    bufs: Box<[deque; MAX_AUDIO_CHANNELS]>,
    audio_ts: u64,
    last_size: usize,
    pending: bool,
    pending_stop: bool,
    timing_set: bool,
}

/// A step, as one comparable tuple: rerender or path, no_timestamp or stop,
/// buffered, dropped, lag, warn, warn_lag.
type StepKey = (i64, i64, usize, usize, u64, bool, u64);

/// A deque at the end: its fields (size, start, end, capacity) and the bits
/// of every sample left in it.
type DequeEnd = ((usize, usize, usize, usize), Vec<u32>);

/// What a side ends with: the fields, and per deque its fields and samples.
#[derive(Debug, PartialEq)]
struct End {
    step: StepKey,
    fields: (u64, usize, bool, bool, bool),
    deques: Vec<DequeEnd>,
}

impl Source {
    fn new(case: &Case) -> Self {
        let mut bufs: Box<[deque; MAX_AUDIO_CHANNELS]> =
            Box::new(core::array::from_fn(|_| deque {
                data: ptr::null_mut(),
                size: 0,
                start_pos: 0,
                end_pos: 0,
                capacity: 0,
            }));
        for (ch, dq) in bufs.iter_mut().enumerate() {
            for i in 0..case.frames[ch] {
                let v = (ch * 10000 + i) as f32;
                // SAFETY: a valid deque; one f32.
                unsafe { deque_push_back(dq, ptr::from_ref(&v).cast::<c_void>(), F32) };
            }
        }
        Self {
            bufs,
            audio_ts: case.audio_ts,
            last_size: case.last_size,
            pending: case.pending,
            pending_stop: case.pending_stop,
            timing_set: case.timing_set,
        }
    }

    fn input(&mut self) -> obs_audio_input {
        obs_audio_input {
            buf: self.bufs.as_mut_ptr(),
            audio_ts: &mut self.audio_ts,
            last_size: &mut self.last_size,
            pending: &mut self.pending,
            pending_stop: &mut self.pending_stop,
            timing_set: &mut self.timing_set,
        }
    }

    fn end(mut self, step: StepKey) -> End {
        let mut deques = Vec::new();
        for dq in self.bufs.iter_mut() {
            let fields = (dq.size, dq.start_pos, dq.end_pos, dq.capacity);
            let mut samples = Vec::new();
            while dq.size >= F32 {
                let mut v = 0f32;
                // SAFETY: a valid deque holding at least one f32.
                unsafe { deque_pop_front(dq, ptr::from_mut(&mut v).cast::<c_void>(), F32) };
                samples.push(v.to_bits());
            }
            // SAFETY: a valid deque with bmem memory.
            unsafe { deque_free(dq) };
            deques.push((fields, samples));
        }
        End {
            step,
            fields: (
                self.audio_ts,
                self.last_size,
                self.pending,
                self.pending_stop,
                self.timing_set,
            ),
            deques,
        }
    }
}

fn ignore_step(s: obs_audio_ignore_step) -> StepKey {
    (
        i64::from(s.rerender),
        i64::from(s.no_timestamp),
        s.buffered,
        s.dropped,
        s.lag,
        s.warn,
        s.warn_lag,
    )
}

fn oracle_ignore_step(s: OracleAudioIgnoreStep) -> StepKey {
    (
        i64::from(s.rerender),
        i64::from(s.no_timestamp),
        s.buffered,
        s.dropped,
        s.lag,
        s.warn,
        s.warn_lag,
    )
}

fn discard_step(path: i32, stop: i32) -> StepKey {
    (i64::from(path), i64::from(stop), 0, 0, 0, false, 0)
}

fn run_shim(case: &Case) -> End {
    let mut src = Source::new(case);
    let input = src.input();
    // SAFETY: valid fields and 8 valid deques, `channels` <= 8.
    let step = unsafe {
        match case.op {
            Op::Ignore { start_ts } => ignore_step(obs_audio_discard_ignore(
                &input,
                case.channels,
                case.rate,
                start_ts,
            )),
            Op::IfStopped => discard_step(0, obs_audio_discard_if_stopped(&input, case.channels)),
            Op::Tick { audio_render, ts } => {
                let s = obs_audio_discard_tick(&input, audio_render, case.channels, case.rate, &ts);
                discard_step(s.path, s.stop)
            }
        }
    };
    src.end(step)
}

fn run_oracle(case: &Case) -> End {
    let mut src = Source::new(case);
    let input = src.input();
    let input = ptr::from_ref(&input).cast::<c_void>();
    // SAFETY: as run_shim; the layout test checks the struct is the C's.
    let step = unsafe {
        match case.op {
            Op::Ignore { start_ts } => oracle_ignore_step(oracle_obs_audio_discard_ignore(
                input,
                case.channels,
                case.rate,
                start_ts,
            )),
            Op::IfStopped => {
                discard_step(0, oracle_obs_audio_discard_if_stopped(input, case.channels))
            }
            Op::Tick { audio_render, ts } => {
                let OracleAudioDiscardStep { path, stop } = oracle_obs_audio_discard_tick(
                    input,
                    audio_render,
                    case.channels,
                    case.rate,
                    ptr::from_ref(&ts).cast::<c_void>(),
                );
                discard_step(path, stop)
            }
        }
    };
    src.end(step)
}

/// The C is defined for the case: no rate of 0 where ignore divides by it,
/// and no channel holding less than channel 0 (each pops channel 0's
/// count).
fn c_defined(case: &Case) -> bool {
    let rate_ok = !matches!(case.op, Op::Ignore { .. }) || case.rate != 0;
    let even = case.frames[..case.channels]
        .iter()
        .all(|&n| n >= case.frames[0]);
    rate_ok && even
}

fn compare(case: &Case) {
    assert!(c_defined(case), "{case:?}");
    assert_eq!(run_shim(case), run_oracle(case), "{case:?}");
}

fn case(op: Op, channels: usize, frames: usize, audio_ts: u64) -> Case {
    Case {
        op,
        channels,
        frames: [frames; MAX_AUDIO_CHANNELS],
        audio_ts,
        last_size: 0,
        pending: false,
        pending_stop: false,
        timing_set: true,
        rate: 48000,
    }
}

fn tick(rate: usize) -> ts_info {
    ts_info {
        start: START,
        end: START + audio_frames_to_ns(rate, FRAMES),
    }
}

#[test]
fn chosen_cases() {
    let sample = audio_frames_to_ns(48000, 1);
    let ignore = |start_ts| Op::Ignore { start_ts };
    let at = |ts| Op::Tick {
        audio_render: false,
        ts,
    };
    let mut cases = vec![
        // ignore: no timestamp, the round-up, the overshoot, the restart
        case(ignore(START), 2, 100, 0),
        case(ignore(START), 2, 100, START - sample),
        case(ignore(START), 2, 100, START - 30000),
        case(ignore(START), 1, 100, START - 1),
        case(ignore(START), 2, 10, START - 1_000_000),
        case(ignore(START), 2, 0, START - 1000),
        // stop check: empty, new size, unchanged once and twice
        case(Op::IfStopped, 2, 0, START),
        case(Op::IfStopped, 2, 10, START),
        // discard: render source, ahead, behind, the tick, a partial tick
        Case {
            op: Op::Tick {
                audio_render: true,
                ts: tick(48000),
            },
            ..case(Op::IfStopped, 2, 2000, START)
        },
        case(at(tick(48000)), 2, 2000, tick(48000).end),
        case(at(tick(48000)), 2, 10, START - 2),
        case(at(tick(48000)), 8, 1500, START),
        case(at(tick(48000)), 8, 1500, START - 1),
        case(
            at(tick(48000)),
            2,
            1500,
            START + audio_frames_to_ns(48000, 512),
        ),
        case(at(tick(48000)), 2, 100, START),
    ];
    // 44.1 kHz: the 1 ns adjustment.
    cases.push(Case {
        rate: 44100,
        ..case(
            ignore(START),
            2,
            100,
            START - audio_frames_to_ns(44100, 1) - 1,
        )
    });
    // ignore while pending, with nothing buffered: no warning.
    cases.push(Case {
        pending: true,
        ..case(ignore(START), 2, 0, START - 1000)
    });
    // The stop check, unchanged once and twice, on its own and behind.
    for (pending_stop, op) in [
        (false, Op::IfStopped),
        (true, Op::IfStopped),
        (false, at(tick(48000))),
        (true, at(tick(48000))),
    ] {
        cases.push(Case {
            last_size: 10 * F32,
            pending: true,
            pending_stop,
            ..case(op, 2, 10, START - 2)
        });
    }
    // A start point on 1024, and past it, in wide windows at 8 kHz.
    let tick8 = audio_frames_to_ns(8000, FRAMES);
    for (len, at_ts) in [(2, START + tick8), (3, START + tick8 + 10)] {
        cases.push(Case {
            rate: 8000,
            ..case(
                at(ts_info {
                    start: START,
                    end: START + len * tick8,
                }),
                2,
                5000,
                at_ts,
            )
        });
    }
    // A tick at time 0: ts.start - 1 wraps.
    cases.push(case(
        at(ts_info {
            start: 0,
            end: tick(48000).end - START,
        }),
        2,
        2000,
        5,
    ));
    // Channels 1 to 7 holding more than channel 0.
    let mut uneven = case(ignore(START), 4, 10, START - 1_000_000);
    uneven.frames = [10, 20, 30, 40, 0, 0, 0, 0];
    cases.push(uneven.clone());
    uneven.audio_ts = 0;
    cases.push(uneven.clone());
    uneven.op = Op::IfStopped;
    uneven.last_size = 10 * F32;
    uneven.pending_stop = true;
    cases.push(uneven);

    for c in &cases {
        compare(c);
    }
}

/// The shim alone: a channel holding less than channel 0's count is
/// emptied (the C underflows the deque's size).
#[test]
fn shim_cuts_a_short_channel() {
    let mut c = case(Op::Ignore { start_ts: START }, 2, 100, START - 1_000_000);
    c.frames[1] = 5;
    let end = run_shim(&c);
    assert_eq!(end.deques[0].1.len(), 100 - 48);
    assert_eq!(end.deques[1].1.len(), 0);
    assert_eq!(end.deques[1].0, (0, 0, 0, end.deques[1].0.3));
}

/// The shim alone: a rate of 0 in ignore (the C divides by 0).
#[test]
fn shim_rate_zero_in_ignore() {
    let mut c = case(Op::Ignore { start_ts: START }, 2, 100, START - 1000);
    c.rate = 0;
    let end = run_shim(&c);
    assert_eq!(end.fields.0, 0);
    assert!(end.fields.2);
    assert!(!end.fields.4);
    assert_eq!(end.deques[0].1.len(), 99);
}

/// The shim alone: null pointers do nothing.
#[test]
fn shim_null_pointers() {
    let ts = tick(48000);
    // SAFETY: null pointers are allowed and do nothing.
    unsafe {
        assert_eq!(
            obs_audio_discard_ignore(ptr::null(), 2, 48000, START),
            obs_audio_ignore_step::default()
        );
        assert_eq!(
            obs_audio_discard_if_stopped(ptr::null(), 2),
            OBS_AUDIO_STOP_NONE
        );
        assert_eq!(
            obs_audio_discard_tick(ptr::null(), false, 2, 48000, &ts),
            obs_audio_discard_step::default()
        );
        let mut src = Source::new(&case(Op::IfStopped, 2, 10, START));
        let mut input = src.input();
        assert_eq!(
            obs_audio_discard_tick(&input, false, 2, 48000, ptr::null()),
            obs_audio_discard_step::default()
        );
        input.last_size = ptr::null_mut();
        assert_eq!(obs_audio_discard_if_stopped(&input, 2), OBS_AUDIO_STOP_NONE);
        let end = src.end(discard_step(0, 0));
        assert_eq!(end.deques[0].1.len(), 10);
    }
}

fn rate() -> impl Strategy<Value = usize> {
    prop_oneof![
        Just(44100usize),
        Just(48000usize),
        Just(8000usize),
        Just(96000usize),
        1usize..384_000
    ]
}

fn timestamp(start: u64, end: u64) -> impl Strategy<Value = u64> {
    prop_oneof![
        Just(0u64),
        Just(start),
        Just(start.wrapping_sub(1)),
        Just(end),
        Just(end.wrapping_sub(1)),
        start.saturating_sub(50_000_000)..=start,
        start..=end.max(start),
        end..=end.saturating_add(50_000_000),
        any::<u64>(),
    ]
}

fn case_strategy() -> impl Strategy<Value = Case> {
    (
        0usize..=MAX_AUDIO_CHANNELS,
        0usize..3000,
        prop::array::uniform8(0usize..200),
        rate(),
        prop_oneof![
            Just(START),
            Just(0u64),
            Just(u64::MAX - 1_000_000_000),
            any::<u64>()
        ],
        1u64..=2,
        any::<[bool; 5]>(),
        0u8..3,
        0u8..3,
    )
        .prop_flat_map(
            |(channels, f0, extra, rate, start, ticks, flags, last, op)| {
                let end = start.wrapping_add(ticks * audio_frames_to_ns(rate, FRAMES));
                (
                    Just((channels, f0, extra, rate, start, end, flags, last, op)),
                    timestamp(start, end),
                    any::<usize>(),
                )
            },
        )
        .prop_map(
            |((channels, f0, extra, rate, start, end, flags, last, op), audio_ts, any_last)| {
                let mut frames = [0usize; MAX_AUDIO_CHANNELS];
                for (ch, n) in frames.iter_mut().enumerate() {
                    *n = if ch == 0 { f0 } else { f0 + extra[ch] };
                }
                let last_size = match last {
                    0 => 0,
                    1 => f0 * F32,
                    _ => any_last,
                };
                let op = match op {
                    0 => Op::Ignore { start_ts: start },
                    1 => Op::IfStopped,
                    _ => Op::Tick {
                        audio_render: flags[4],
                        ts: ts_info { start, end },
                    },
                };
                Case {
                    op,
                    channels,
                    frames,
                    audio_ts,
                    last_size,
                    pending: flags[0],
                    pending_stop: flags[1],
                    timing_set: flags[2],
                    rate,
                }
            },
        )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20_000))]

    #[test]
    fn random_cases_match(c in case_strategy()) {
        prop_assume!(c_defined(&c));
        prop_assert_eq!(run_shim(&c), run_oracle(&c), "{:?}", c);
    }
}
