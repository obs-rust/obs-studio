//! Tier 1: the safe core of `libobs/obs-audio-discard.c.h`.
//!
//! The first 11 tests mirror the `obs-audio-discard.c` cases of
//! `test/cmocka/test_audio_buffering_subsample.c` one to one, with the same
//! names, in the same order, with the same inputs and the same expected
//! values. The input deques are [`Input::bufs`], one [`Vec`] per channel
//! whose front is index 0, and [`Input::apply`] pops what the core returns
//! as the shim does. The twelfth cmocka case,
//! `source_wrappers_use_the_source_fields`, checks the C wrappers in
//! `obs-audio.c` and has no counterpart here.
//!
//! The cases after those (`edge_*`) pin the edges of the safe API, and a
//! proptest checks that no input panics and the invariants the issue lists.

use obs_core::audio_discard::{
    DiscardPath, DiscardStep, IgnoreStep, InputState, Pop, StopCheck, discard_tick, if_stopped,
    ignore,
};
use obs_core::audio_mix::{AUDIO_OUTPUT_FRAMES, MAX_AUDIO_CHANNELS, TsInfo};
use obs_media_io::audio_io::audio_frames_to_ns;
use proptest::prelude::*;

const START: u64 = 1_000_000_000;
const FRAMES: u64 = AUDIO_OUTPUT_FRAMES as u64;
const F32: usize = 4;

/// The source's input: the deques and the fields the core reads.
#[derive(Debug, Clone)]
struct Input {
    bufs: Vec<Vec<f32>>,
    state: InputState,
    timing_set: bool,
}

impl Input {
    /// `frames` floats per channel, value ch * 10000 + index, so the front
    /// value says how many were popped.
    fn new(channels: usize, frames: usize, audio_ts: u64) -> Self {
        Self {
            bufs: (0..channels)
                .map(|ch| (0..frames).map(|i| (ch * 10000 + i) as f32).collect())
                .collect(),
            state: InputState {
                audio_ts,
                ..InputState::default()
            },
            timing_set: true,
        }
    }

    /// `audio_input_buf[0].size`.
    fn size(&self) -> usize {
        self.bufs.first().map_or(0, |b| b.len() * F32)
    }

    fn frames_left(&self, ch: usize) -> usize {
        self.bufs[ch].len()
    }

    /// Index of the front sample of channel ch: the number popped.
    fn front_index(&self, ch: usize) -> usize {
        self.bufs[ch][0] as usize - ch * 10000
    }

    /// What the shim does with a [`Pop`] on the first `channels` deques.
    fn apply(&mut self, pop: Pop) {
        for (ch, buf) in self.bufs.iter_mut().enumerate() {
            match pop {
                Pop::None => {}
                Pop::Bytes(n) => {
                    buf.drain(..(n / F32).min(buf.len()));
                }
                Pop::Channel0(n) if ch == 0 => {
                    buf.drain(..(n / F32).min(buf.len()));
                }
                Pop::Channel0(_) => {}
                Pop::EachAll => buf.clear(),
            }
        }
    }

    fn ignore(&mut self, rate: usize, start_ts: u64) -> IgnoreStep {
        let size = self.size();
        let (step, pop) = ignore(&mut self.state, size, rate, start_ts);
        self.apply(pop);
        if step.restarted {
            self.timing_set = false;
        }
        step
    }

    fn if_stopped(&mut self) -> StopCheck {
        let size = self.size();
        let (check, pop) = if_stopped(&mut self.state, size);
        self.apply(pop);
        check
    }

    fn discard(&mut self, audio_render: bool, rate: usize, ts: TsInfo) -> DiscardStep {
        let size = self.size();
        let (step, pop) = discard_tick(&mut self.state, audio_render, size, rate, ts);
        self.apply(pop);
        step
    }
}

fn tick(rate: usize) -> TsInfo {
    TsInfo {
        start: START,
        end: START + audio_frames_to_ns(rate, FRAMES),
    }
}

#[test]
fn ignore_without_timestamp_clears_channel_0() {
    // Each channel pops channel 0's size, read again for each channel, so
    // once channel 0 is empty the other channels keep their audio.
    let mut c = Input::new(2, 100, 0);
    c.state.last_size = 400;
    let step = c.ignore(48000, START);
    assert!(step.no_timestamp);
    assert!(!step.rerender);
    assert!(!step.warn);
    assert_eq!(step.buffered, 100);
    assert_eq!(c.frames_left(0), 0);
    assert_eq!(c.frames_left(1), 100);
    assert_eq!(c.state.last_size, 0);
    assert_eq!(c.state.audio_ts, 0);
    // not a restart
    assert!(!c.state.pending);
    assert!(c.timing_set);
}

#[test]
fn ignore_drop_rounds_up() {
    let sample = audio_frames_to_ns(48000, 1);

    // Exactly one sample behind: one dropped, in sync, lag 0.
    let mut c = Input::new(2, 100, START - sample);
    c.state.last_size = 400;
    let step = c.ignore(48000, START);
    assert!(step.rerender);
    assert!(!step.warn);
    assert_eq!(step.buffered, 100);
    assert_eq!(step.dropped, 1);
    assert_eq!(step.lag, 0);
    assert_eq!(c.state.audio_ts, START);
    assert_eq!(c.front_index(0), 1);
    assert_eq!(c.front_index(1), 1);
    assert_eq!(c.state.last_size, 0);

    // A partial sample more: two dropped, audio_ts past start_ts.
    let mut c = Input::new(2, 100, START - 30000);
    let step = c.ignore(48000, START);
    assert!(step.rerender);
    assert_eq!(step.dropped, 2);
    assert_eq!(
        c.state.audio_ts,
        START - 30000 + audio_frames_to_ns(48000, 2)
    );
    assert_eq!(c.front_index(0), 2);
}

#[test]
fn ignore_lag_wraps_when_the_drop_overshoots() {
    // 1 ns behind: one sample (20833 ns at 48 kHz) is dropped, audio_ts
    // lands after start_ts, and the logged lag wraps.
    let mut c = Input::new(1, 100, START - 1);
    let step = c.ignore(48000, START);
    assert!(step.rerender);
    assert_eq!(step.dropped, 1);
    assert_eq!(c.state.audio_ts, START - 1 + audio_frames_to_ns(48000, 1));
    assert_eq!(
        step.lag,
        0u64.wrapping_sub(audio_frames_to_ns(48000, 1) - 1)
    );
}

#[test]
fn ignore_adjusts_one_ns_short() {
    // At 44.1 kHz one sample is 22675 ns (rounded down): 22676 ns behind,
    // one dropped, audio_ts lands on start_ts - 1 and is moved to start_ts.
    // The lag is logged before the adjustment.
    let sample = audio_frames_to_ns(44100, 1);
    let mut c = Input::new(2, 100, START - sample - 1);
    let step = c.ignore(44100, START);
    assert_eq!(sample, 22675);
    assert!(step.rerender);
    assert_eq!(step.dropped, 1);
    assert_eq!(step.lag, 1);
    assert_eq!(c.state.audio_ts, START);
}

#[test]
fn ignore_restarts_when_buffer_is_short() {
    // 1 ms (48 samples) behind with 10 buffered: all 10 dropped, still
    // behind, so the source's audio restarts with a warning.
    let mut c = Input::new(2, 10, START - 1_000_000);
    let step = c.ignore(48000, START);
    let after = START - 1_000_000 + audio_frames_to_ns(48000, 10);
    assert!(!step.rerender);
    assert_eq!(step.dropped, 10);
    assert_eq!(step.lag, START - after);
    assert!(step.warn);
    assert_eq!(step.warn_lag, START - after);
    assert_eq!(c.frames_left(0), 0);
    assert_eq!(c.frames_left(1), 0);
    assert!(c.state.pending);
    assert_eq!(c.state.audio_ts, 0);
    assert!(!c.timing_set);
}

#[test]
fn ignore_without_samples_warns_unless_pending() {
    // Not pending: restart with a warning.
    let mut c = Input::new(2, 0, START - 1000);
    let step = c.ignore(48000, START);
    assert!(!step.rerender);
    assert_eq!(step.buffered, 0);
    assert_eq!(step.dropped, 0);
    assert!(step.warn);
    assert_eq!(step.warn_lag, 1000);
    assert!(c.state.pending);
    assert_eq!(c.state.audio_ts, 0);
    assert!(!c.timing_set);

    // Already pending: the same restart, no warning.
    let mut c = Input::new(2, 0, START - 1000);
    c.state.pending = true;
    let step = c.ignore(48000, START);
    assert!(!step.warn);
    assert!(c.state.pending);
    assert_eq!(c.state.audio_ts, 0);
    assert!(!c.timing_set);
}

#[test]
fn ignore_pops_channel_0_size_everywhere() {
    // The count is channel 0's, and every channel pops it. 1 ms (48
    // samples) behind with 10 in channel 0 and 20 in channel 1: 10 dropped
    // from each.
    let mut c = Input::new(2, 10, START - 1_000_000);
    c.bufs[1].extend((0..10).map(|i| (10000 + 10 + i) as f32));
    c.ignore(48000, START);
    assert_eq!(c.frames_left(0), 0);
    assert_eq!(c.frames_left(1), 10);
    assert_eq!(c.front_index(1), 10);
}

#[test]
fn stop_check_needs_two_unchanged_ticks() {
    // Empty: nothing.
    let mut c = Input::new(2, 0, START);
    c.state.last_size = 40;
    assert_eq!(c.if_stopped(), StopCheck::None);
    assert_eq!(c.state.last_size, 40);

    // The size changed: remembered.
    let mut c = Input::new(2, 10, START);
    assert_eq!(c.if_stopped(), StopCheck::None);
    assert_eq!(c.state.last_size, 10 * F32);
    assert!(!c.state.pending_stop);

    // Unchanged once: pending_stop, data kept.
    assert_eq!(c.if_stopped(), StopCheck::Pending);
    assert!(c.state.pending_stop);
    assert_eq!(c.frames_left(0), 10);

    // Unchanged twice: cleared, each channel by its own size.
    c.bufs[1].extend((0..5).map(|i| (10000 + 10 + i) as f32));
    assert_eq!(c.if_stopped(), StopCheck::Cleared);
    assert!(!c.state.pending_stop);
    assert_eq!(c.frames_left(0), 0);
    assert_eq!(c.frames_left(1), 0);
    assert_eq!(c.state.audio_ts, 0);
    assert_eq!(c.state.last_size, 0);
}

#[test]
fn discard_early_returns() {
    let ts = tick(48000);

    // A render source: audio_ts reset, nothing popped.
    let mut c = Input::new(2, 2000, START);
    let step = c.discard(true, 48000, ts);
    assert_eq!(step.path, DiscardPath::RenderSource);
    assert_eq!(c.state.audio_ts, 0);
    assert_eq!(c.frames_left(0), 2000);

    // At or after the tick end: nothing.
    for ahead in [ts.end, ts.end + 1] {
        let mut c = Input::new(2, 2000, ahead);
        let step = c.discard(false, 48000, ts);
        assert_eq!(step.path, DiscardPath::Ahead);
        assert_eq!(c.state.audio_ts, ahead);
        assert_eq!(c.frames_left(0), 2000);
    }

    // Behind, not pending: no stop check.
    let mut c = Input::new(2, 10, START - 2);
    let step = c.discard(false, 48000, ts);
    assert_eq!(step.path, DiscardPath::Behind);
    assert_eq!(step.stop, StopCheck::None);
    assert_eq!(c.state.last_size, 0);
    assert_eq!(c.state.audio_ts, START - 2);

    // Behind, pending, with a full tick buffered: no stop check either.
    let mut c = Input::new(2, AUDIO_OUTPUT_FRAMES, START - 2);
    c.state.pending = true;
    let step = c.discard(false, 48000, ts);
    assert_eq!(step.path, DiscardPath::Behind);
    assert_eq!(c.state.last_size, 0);

    // Behind, pending, less than a tick: the stop check runs.
    let mut c = Input::new(2, 10, START - 2);
    c.state.pending = true;
    assert_eq!(c.discard(false, 48000, ts).stop, StopCheck::None);
    assert_eq!(c.discard(false, 48000, ts).stop, StopCheck::Pending);
    let step = c.discard(false, 48000, ts);
    assert_eq!(step.path, DiscardPath::Behind);
    assert_eq!(step.stop, StopCheck::Cleared);
    assert_eq!(c.frames_left(0), 0);
    assert_eq!(c.state.audio_ts, 0);

    // A start point on AUDIO_OUTPUT_FRAMES, in a two-tick window at 8 kHz
    // (one tick is exactly 128 ms there): nothing.
    let tick8 = audio_frames_to_ns(8000, FRAMES);
    let wide = TsInfo {
        start: START,
        end: START + 2 * tick8,
    };
    let mut c = Input::new(2, 2000, START + tick8);
    let step = c.discard(false, 8000, wide);
    assert_eq!(tick8, 128_000_000);
    assert_eq!(step.path, DiscardPath::StartAtEnd);
    assert_eq!(c.state.audio_ts, START + tick8);
    assert_eq!(c.frames_left(0), 2000);
}

#[test]
fn discard_pops_the_tick() {
    let ts = tick(48000);

    // On ts.start and on ts.start - 1: the whole tick.
    for channels in [1, 2, 8] {
        for off in 0..2 {
            let mut c = Input::new(channels, 1500, START - off);
            c.state.last_size = 40;
            c.state.pending_stop = true;
            let step = c.discard(false, 48000, ts);
            assert_eq!(step.path, DiscardPath::Done);
            assert_eq!(step.stop, StopCheck::None);
            for ch in 0..channels {
                assert_eq!(c.frames_left(ch), 1500 - AUDIO_OUTPUT_FRAMES);
                assert_eq!(c.front_index(ch), AUDIO_OUTPUT_FRAMES);
            }
            assert_eq!(c.state.audio_ts, ts.end);
            assert_eq!(c.state.last_size, 0);
            assert!(!c.state.pending_stop);
        }
    }

    // From frame 512's time, which maps to frame 511: 513 frames.
    let mut c = Input::new(2, 1500, START + audio_frames_to_ns(48000, 512));
    let step = c.discard(false, 48000, ts);
    assert_eq!(step.path, DiscardPath::Done);
    assert_eq!(c.front_index(0), AUDIO_OUTPUT_FRAMES - 511);
    assert_eq!(c.front_index(1), AUDIO_OUTPUT_FRAMES - 511);
    assert_eq!(c.state.audio_ts, ts.end);
}

#[test]
fn discard_waits_for_pending_data() {
    let ts = tick(48000);

    // Less than the tick: nothing popped, audio_ts moves to the end, the
    // size is remembered.
    let mut c = Input::new(2, 100, START);
    let step = c.discard(false, 48000, ts);
    assert_eq!(step.path, DiscardPath::Pending);
    assert_eq!(step.stop, StopCheck::None);
    assert_eq!(c.frames_left(0), 100);
    assert_eq!(c.state.audio_ts, ts.end);
    assert_eq!(c.state.last_size, 100 * F32);

    // The same size again: pending stop, audio_ts still moves to the end.
    let mut c = Input::new(2, 100, START);
    c.state.last_size = 100 * F32;
    let step = c.discard(false, 48000, ts);
    assert_eq!(step.stop, StopCheck::Pending);
    assert!(c.state.pending_stop);
    assert_eq!(c.state.audio_ts, ts.end);

    // And again: cleared, audio_ts stays 0.
    let mut c = Input::new(2, 100, START);
    c.state.last_size = 100 * F32;
    c.state.pending_stop = true;
    let step = c.discard(false, 48000, ts);
    assert_eq!(step.path, DiscardPath::Pending);
    assert_eq!(step.stop, StopCheck::Cleared);
    assert_eq!(c.frames_left(0), 0);
    assert_eq!(c.state.audio_ts, 0);
}

#[test]
fn edge_start_point_past_the_tick_wraps_to_pending() {
    // A window longer than one tick and audio after its first tick: the
    // start point is past AUDIO_OUTPUT_FRAMES, the frames left wrap in
    // size_t as in C, so no buffer is large enough and the source waits.
    let tick8 = audio_frames_to_ns(8000, FRAMES);
    let wide = TsInfo {
        start: START,
        end: START + 3 * tick8,
    };
    let mut c = Input::new(2, 5000, START + tick8 + audio_frames_to_ns(8000, 10));
    let step = c.discard(false, 8000, wide);
    assert_eq!(step.path, DiscardPath::Pending);
    assert_eq!(c.frames_left(0), 5000);
    assert_eq!(c.state.audio_ts, wide.end);
}

#[test]
fn edge_tick_at_time_zero() {
    // ts.start - 1 wraps: every timestamp is behind, as in C.
    let ts = TsInfo {
        start: 0,
        end: audio_frames_to_ns(48000, FRAMES),
    };
    let mut c = Input::new(2, 2000, 5);
    let step = c.discard(false, 48000, ts);
    assert_eq!(step.path, DiscardPath::Behind);
    assert_eq!(c.frames_left(0), 2000);
}

#[test]
fn edge_rate_zero_drops_without_moving_audio_ts() {
    // A rate of 0 (the C divides by 0 for the time of the dropped samples):
    // one sample is dropped and audio_ts does not move, so the source
    // restarts.
    let mut c = Input::new(2, 100, START - 1000);
    let step = c.ignore(0, START);
    assert_eq!(step.dropped, 1);
    assert!(step.restarted);
    assert_eq!(c.state.audio_ts, 0);
}

#[test]
fn edge_stop_check_ignores_a_new_size_once() {
    let mut c = Input::new(1, 10, START);
    c.state.pending_stop = true;
    // The size changed: remembered, pending_stop kept.
    assert_eq!(c.if_stopped(), StopCheck::None);
    assert!(c.state.pending_stop);
    // Unchanged now, and pending_stop was already set: cleared.
    assert_eq!(c.if_stopped(), StopCheck::Cleared);
}

fn rates() -> impl Strategy<Value = usize> {
    prop_oneof![
        Just(44100usize),
        Just(48000usize),
        Just(8000usize),
        1usize..400_000
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    /// The ignore step never pops more than channel 0 holds, and audio_ts
    /// only moves forward or restarts at 0.
    #[test]
    fn prop_ignore_invariants(
        channels in 1usize..=MAX_AUDIO_CHANNELS,
        frames in 0usize..3000,
        behind in 1u64..200_000_000,
        rate in rates(),
        pending in any::<bool>(),
        no_ts in any::<bool>(),
    ) {
        let audio_ts = if no_ts { 0 } else { START - behind };
        let mut state = InputState { audio_ts, pending, ..InputState::default() };
        let size = frames * F32;
        let (step, pop) = ignore(&mut state, size, rate, START);
        prop_assert!(step.dropped <= step.buffered);
        prop_assert_eq!(step.buffered, frames);
        match pop {
            Pop::None => prop_assert_eq!(frames * step.dropped, 0),
            Pop::Bytes(n) | Pop::Channel0(n) => prop_assert!(n <= size),
            Pop::EachAll => prop_assert!(false, "ignore never clears by channel"),
        }
        prop_assert!(state.audio_ts == 0 || state.audio_ts >= audio_ts);
        prop_assert_eq!(step.rerender, state.audio_ts >= START);
        prop_assert_eq!(step.restarted, !step.rerender && !step.no_timestamp);
        let _ = channels;
    }

    /// The discard step pops nothing or less than channel 0 holds, and
    /// audio_ts never moves backwards except to 0.
    #[test]
    fn prop_discard_invariants(
        frames in 0usize..3000,
        offset in -50_000_000i64..50_000_000,
        rate in rates(),
        pending in any::<bool>(),
        pending_stop in any::<bool>(),
        last_frames in 0usize..3000,
        audio_render in any::<bool>(),
    ) {
        let ts = TsInfo { start: START, end: START + audio_frames_to_ns(rate, FRAMES) };
        let audio_ts = START.wrapping_add_signed(offset);
        let mut state = InputState {
            audio_ts,
            last_size: last_frames * F32,
            pending,
            pending_stop,
        };
        let size = frames * F32;
        let (step, pop) = discard_tick(&mut state, audio_render, size, rate, ts);
        match pop {
            Pop::None => {}
            Pop::Bytes(n) => {
                prop_assert_eq!(step.path, DiscardPath::Done);
                prop_assert!(n <= size);
                prop_assert!(n <= AUDIO_OUTPUT_FRAMES * F32);
            }
            Pop::Channel0(_) => prop_assert!(false, "discard never pops channel 0 alone"),
            Pop::EachAll => prop_assert_eq!(step.stop, StopCheck::Cleared),
        }
        prop_assert!(state.audio_ts == 0 || state.audio_ts >= audio_ts);
    }

    /// Any input, including wrapping timestamps: no panic.
    #[test]
    fn prop_no_panic(
        audio_ts in any::<u64>(),
        start in any::<u64>(),
        end in any::<u64>(),
        size in any::<usize>(),
        rate in any::<usize>(),
        last_size in any::<usize>(),
        flags in any::<[bool; 3]>(),
    ) {
        let mut state = InputState {
            audio_ts,
            last_size,
            pending: flags[0],
            pending_stop: flags[1],
        };
        let _ = ignore(&mut state.clone(), size, rate, start);
        let _ = if_stopped(&mut state.clone(), size);
        let _ = discard_tick(&mut state, flags[2], size, rate, TsInfo { start, end });
    }
}
