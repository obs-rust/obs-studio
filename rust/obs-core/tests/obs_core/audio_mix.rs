//! Tier 1: the safe core of `libobs/obs-audio-mix.c.h`.
//!
//! The first 18 tests mirror `test/cmocka/test_audio_buffering_subsample.c`
//! one to one, with the same names, in the same order, with the same inputs
//! and the same expected values. `struct obs_core_audio` is a [`Buffering`]
//! plus a [`Vec`] queue whose front is index 0; the source's mix is
//! [`mix_all`], the loop of `obs_audio_mix_add()` over the mixes and
//! channels on top of [`mix_start`] and [`mix_into`].
//!
//! The cases after those (`edge_*`) pin the intentional differences and
//! the edges of the safe API, and a proptest checks that no input panics.

use obs_core::audio_mix::{
    AUDIO_OUTPUT_FRAMES, Buffering, MAX_AUDIO_CHANNELS, MAX_AUDIO_MIXES, Step, TsInfo,
    buffering_add, buffering_fixed, buffering_maxed, mix_into, mix_start, time_to_frames,
};
use obs_media_io::audio_io::{audio_frames_to_ns, ns_to_audio_frames};
use proptest::prelude::*;

const START: u64 = 1_000_000_000;
const FRAMES: u64 = AUDIO_OUTPUT_FRAMES as u64;
const MIX_CHANNELS: usize = 2;

/// floor(ns * rate / 1e9), independent of `util_mul_div64`.
fn frames_at(rate: usize, ns: u64) -> usize {
    usize::try_from(u128::from(ns) * rate as u128 / 1_000_000_000).unwrap()
}

fn tick(rate: usize) -> TsInfo {
    TsInfo {
        start: START,
        end: START + audio_frames_to_ns(rate, FRAMES),
    }
}

/// `struct obs_core_audio`, the fields the core uses.
#[derive(Debug, Default)]
struct Audio {
    buffering: Buffering,
    queue: Vec<TsInfo>,
}

impl Audio {
    fn new(total: i32, max: i32) -> Self {
        Self {
            buffering: Buffering {
                total_ticks: total,
                max_ticks: max,
                ..Buffering::default()
            },
            queue: Vec::new(),
        }
    }

    fn fixed(&mut self, rate: usize, ts: &mut TsInfo) -> Step {
        let queue = &mut self.queue;
        buffering_fixed(&mut self.buffering, rate, ts, |w| queue.insert(0, w))
    }

    fn add(&mut self, rate: usize, ts: &mut TsInfo, min_ts: u64) -> Step {
        let queue = &mut self.queue;
        buffering_add(&mut self.buffering, rate, ts, min_ts, |w| {
            queue.insert(0, w)
        })
    }
}

type Mixes = Vec<Vec<Vec<f32>>>;

/// Every output sample starts at its own value, every source sample is
/// 1000 + its position, so a sum shows which source frame went where.
fn mix_setup() -> (Mixes, Mixes) {
    let out = (0..MAX_AUDIO_MIXES)
        .map(|m| {
            (0..MAX_AUDIO_CHANNELS)
                .map(|c| {
                    (0..AUDIO_OUTPUT_FRAMES)
                        .map(|f| ((m * 16 + c) * 2048 + f) as f32)
                        .collect()
                })
                .collect()
        })
        .collect();
    let source = (0..MAX_AUDIO_MIXES)
        .map(|_| {
            (0..MAX_AUDIO_CHANNELS)
                .map(|_| {
                    (0..AUDIO_OUTPUT_FRAMES)
                        .map(|f| (1000 + f) as f32)
                        .collect()
                })
                .collect()
        })
        .collect();
    (out, source)
}

/// The loop of `obs_audio_mix_add()` over the mixes and channels.
fn mix_all(
    out: &mut Mixes,
    source: &Mixes,
    audio_ts: u64,
    ts: TsInfo,
    channels: usize,
    rate: usize,
) {
    let Some(start) = mix_start(audio_ts, ts, rate) else {
        return;
    };
    for (out_mix, source_mix) in out.iter_mut().zip(source) {
        for (dst, src) in out_mix.iter_mut().zip(source_mix).take(channels) {
            mix_into(dst, src, start);
        }
    }
}

/// With the source's audio from frame `first` on: frames before it
/// unchanged, frame f gets source frame f - first, channels past
/// [`MIX_CHANNELS`] unchanged.
fn assert_mixed_from(out: &Mixes, first: usize) {
    for (m, mix) in out.iter().enumerate() {
        for (c, channel) in mix.iter().enumerate() {
            for (f, &sample) in channel.iter().enumerate() {
                let mut expected = ((m * 16 + c) * 2048 + f) as f32;
                if c < MIX_CHANNELS && f >= first {
                    expected += (1000 + f - first) as f32;
                }
                assert_eq!(
                    sample.to_bits(),
                    expected.to_bits(),
                    "mix {m} ch {c} frame {f}"
                );
            }
        }
    }
}

/// The queue holds `count` contiguous one-tick windows before
/// `buffered_ts`, earliest at the front: the front one starts
/// `first + count` ticks before it, the back one ends `first` ticks before.
fn assert_queued_ticks(queue: &[TsInfo], rate: usize, buffered_ts: u64, first: u64, count: u64) {
    assert_eq!(queue.len() as u64, count);
    for (i, queued) in queue.iter().enumerate() {
        let ticks = first + count - i as u64;
        assert_eq!(
            queued.start,
            buffered_ts - audio_frames_to_ns(rate, ticks * FRAMES)
        );
        assert_eq!(
            queued.end,
            buffered_ts - audio_frames_to_ns(rate, (ticks - 1) * FRAMES)
        );
    }
}

#[test]
fn subsample_delay_preserves_interval() {
    for rate in [44_100, 48_000, 96_000] {
        for offset in [1, 10_000, (1_000_000_000 - 1) / rate as u64] {
            let mut audio = Audio::new(0, 45);
            let original = tick(rate);
            let mut ts = original;
            assert_eq!(ns_to_audio_frames(rate, offset), 0);
            audio.add(rate, &mut ts, START - offset);
            assert_eq!(ts, original);
            assert_eq!(audio.buffering.total_ticks, 0);
            assert_eq!(audio.buffering.wait_ticks, 0);
            assert!(audio.queue.is_empty());
        }
    }
}

#[test]
fn subsample_delay_preserves_pending_interval() {
    let tick_ns = audio_frames_to_ns(48_000, FRAMES);
    let mut audio = Audio::new(1, 45);
    audio.buffering.wait_ticks = 1;
    audio.buffering.buffered_ts = START + tick_ns;
    let original = TsInfo {
        start: START,
        end: START + tick_ns,
    };
    let mut ts = original;
    audio.queue.insert(0, original);
    audio.add(48_000, &mut ts, START - 1);
    assert_eq!(ts, original);
    assert_eq!(audio.queue[0], original);
    assert_eq!(audio.buffering.total_ticks, 1);
    assert_eq!(audio.buffering.wait_ticks, 1);
}

#[test]
fn first_sample_adds_one_tick() {
    let mut audio = Audio::new(0, 45);
    let mut ts = tick(48_000);
    assert_eq!(ns_to_audio_frames(48_000, 20_834), 1);
    audio.add(48_000, &mut ts, START - 20_834);
    assert_eq!(audio.buffering.total_ticks, 1);
    assert_eq!(audio.buffering.wait_ticks, 1);
    assert_eq!(ts.start, START - audio_frames_to_ns(48_000, FRAMES));
    assert_eq!(ts.end, START);
    assert_eq!(audio.queue[0], ts);
}

#[test]
fn larger_delay_adds_two_ticks() {
    let mut audio = Audio::new(0, 45);
    let mut ts = tick(48_000);
    audio.add(48_000, &mut ts, START - 22_000_000);
    assert_eq!(audio.queue.len(), 2);
    assert_eq!(audio.buffering.total_ticks, 2);
    assert_eq!(audio.buffering.wait_ticks, 2);
    assert_eq!(ts.start, START - audio_frames_to_ns(48_000, 2 * FRAMES));
    assert_eq!(ts.end, START - audio_frames_to_ns(48_000, FRAMES));
    assert_eq!(audio.queue[0], ts);
}

#[test]
fn full_buffer_preserves_interval() {
    let mut audio = Audio::new(45, 45);
    let original = tick(48_000);
    let mut ts = original;
    audio.add(48_000, &mut ts, START - 1);
    assert_eq!(ts, original);
    assert_eq!(audio.buffering.total_ticks, 45);
    assert_eq!(audio.buffering.wait_ticks, 0);
    assert!(audio.queue.is_empty());
}

#[test]
fn time_to_frames_rounds_down() {
    for rate in [8_000, 22_050, 44_100, 48_000, 96_000, 192_000] {
        for frames in 0..=FRAMES {
            let ns = audio_frames_to_ns(rate, frames);
            assert_eq!(time_to_frames(rate, ns), frames_at(rate, ns));
            assert_eq!(time_to_frames(rate, ns + 1), frames_at(rate, ns + 1));
        }
    }
    // Characterized, not endorsed: a frame's own time can map to the frame
    // before it.
    assert_eq!(time_to_frames(44_100, audio_frames_to_ns(44_100, 1)), 0);
    assert_eq!(time_to_frames(48_000, audio_frames_to_ns(48_000, 3)), 3);
}

#[test]
fn mix_whole_tick() {
    let (mut out, source) = mix_setup();
    let ts = tick(48_000);
    mix_all(&mut out, &source, ts.start, ts, MIX_CHANNELS, 48_000);
    assert_mixed_from(&out, 0);
}

#[test]
fn mix_from_start_point() {
    for rate in [44_100, 48_000] {
        for frames in [1, 2, 511, 512, 1023] {
            let (mut out, source) = mix_setup();
            let ts = tick(rate);
            let audio_ts = START + audio_frames_to_ns(rate, frames);
            mix_all(&mut out, &source, audio_ts, ts, MIX_CHANNELS, rate);
            assert_mixed_from(&out, frames_at(rate, audio_ts - START));
        }
    }
}

#[test]
fn mix_outside_tick_adds_nothing() {
    let window = tick(48_000);
    for audio_ts in [START - 1, window.end, window.end + 1] {
        let (mut out, source) = mix_setup();
        mix_all(&mut out, &source, audio_ts, window, MIX_CHANNELS, 48_000);
        assert_mixed_from(&out, AUDIO_OUTPUT_FRAMES);
    }
}

#[test]
fn mix_on_frame_1024_adds_nothing() {
    let (mut out, source) = mix_setup();
    let ts = TsInfo {
        start: START,
        end: START + audio_frames_to_ns(48_000, FRAMES + 1),
    };
    let audio_ts = START + audio_frames_to_ns(48_000, FRAMES) + 1;
    assert_eq!(frames_at(48_000, audio_ts - START), AUDIO_OUTPUT_FRAMES);
    mix_all(&mut out, &source, audio_ts, ts, MIX_CHANNELS, 48_000);
    assert_mixed_from(&out, AUDIO_OUTPUT_FRAMES);
}

#[test]
fn buffering_maxed_only_at_max() {
    let mut buffering = Buffering {
        max_ticks: 45,
        ..Buffering::default()
    };
    assert!(!buffering_maxed(&buffering));
    buffering.total_ticks = 44;
    assert!(!buffering_maxed(&buffering));
    buffering.total_ticks = 45;
    assert!(buffering_maxed(&buffering));
    // Characterized, not endorsed: only equality counts.
    buffering.total_ticks = 46;
    assert!(!buffering_maxed(&buffering));
}

#[test]
fn fixed_buffering_fills_to_max() {
    let mut audio = Audio::new(0, 45);
    let mut ts = tick(48_000);
    audio.fixed(48_000, &mut ts);
    assert_eq!(audio.buffering.total_ticks, 45);
    assert_eq!(audio.buffering.wait_ticks, 45);
    assert_eq!(audio.buffering.buffered_ts, START);
    assert_eq!(ts.start, START - audio_frames_to_ns(48_000, 45 * FRAMES));
    assert_eq!(ts.end, START - audio_frames_to_ns(48_000, 44 * FRAMES));
    assert_queued_ticks(&audio.queue, 48_000, START, 0, 45);
}

#[test]
fn fixed_buffering_keeps_buffered_ts_while_waiting() {
    let buffered_ts = START + 5 * audio_frames_to_ns(48_000, FRAMES);
    let mut audio = Audio::new(3, 45);
    audio.buffering.wait_ticks = 2;
    audio.buffering.buffered_ts = buffered_ts;
    let mut ts = tick(48_000);
    audio.fixed(48_000, &mut ts);
    assert_eq!(audio.buffering.total_ticks, 45);
    assert_eq!(audio.buffering.wait_ticks, 44);
    assert_eq!(audio.buffering.buffered_ts, buffered_ts);
    assert_eq!(
        ts.start,
        buffered_ts - audio_frames_to_ns(48_000, 44 * FRAMES)
    );
    assert_queued_ticks(&audio.queue, 48_000, buffered_ts, 2, 42);
}

#[test]
fn fixed_buffering_at_max_changes_nothing() {
    let mut audio = Audio::new(45, 45);
    let original = tick(48_000);
    let mut ts = original;
    audio.fixed(48_000, &mut ts);
    assert_eq!(ts, original);
    assert_eq!(audio.buffering.total_ticks, 45);
    assert_eq!(audio.buffering.wait_ticks, 0);
    assert!(audio.queue.is_empty());
}

#[test]
fn buffering_stops_at_max() {
    let tick_ns = audio_frames_to_ns(48_000, FRAMES);
    let mut audio = Audio::new(43, 45);
    let mut ts = tick(48_000);
    audio.add(48_000, &mut ts, START - 5 * tick_ns);
    assert_eq!(audio.buffering.total_ticks, 45);
    assert_eq!(audio.buffering.wait_ticks, 2);
    assert_eq!(ts.start, START - audio_frames_to_ns(48_000, 2 * FRAMES));
    assert_queued_ticks(&audio.queue, 48_000, START, 0, 2);
}

#[test]
fn buffering_rounds_ticks_up() {
    for rate in [44_100, 48_000] {
        for frames in [1, 1023, 1024, 1025, 2048, 2049] {
            let mut audio = Audio::new(0, 45);
            let mut ts = tick(rate);
            let late = audio_frames_to_ns(rate, frames) + 1;
            let ticks = ns_to_audio_frames(rate, late).div_ceil(FRAMES);
            audio.add(rate, &mut ts, START - late);
            assert_eq!(audio.buffering.total_ticks as u64, ticks);
            assert_eq!(audio.buffering.wait_ticks, ticks);
            assert_eq!(ts.start, START - audio_frames_to_ns(rate, ticks * FRAMES));
            assert_queued_ticks(&audio.queue, rate, START, 0, ticks);
        }
    }
}

#[test]
fn core_mix_on_plain_buffers() {
    let (mut out, source) = mix_setup();
    let ts = tick(48_000);
    // Frame 512's time maps to frame 511 at 48 kHz (both round down).
    let audio_ts = START + audio_frames_to_ns(48_000, 512);
    mix_all(&mut out, &source, audio_ts, ts, MIX_CHANNELS, 48_000);
    assert_mixed_from(&out, frames_at(48_000, audio_frames_to_ns(48_000, 512)));
    assert_eq!(frames_at(48_000, audio_frames_to_ns(48_000, 512)), 511);

    let (mut out, source) = mix_setup();
    mix_all(&mut out, &source, ts.end, ts, MIX_CHANNELS, 48_000);
    assert_mixed_from(&out, AUDIO_OUTPUT_FRAMES);
    assert_eq!(time_to_frames(44_100, audio_frames_to_ns(44_100, 1)), 0);
}

#[test]
fn core_buffering_steps() {
    let tick_ns = audio_frames_to_ns(48_000, FRAMES);

    // At the maximum: nothing, and added is false.
    let mut audio = Audio::new(45, 45);
    let mut ts = tick(48_000);
    assert!(buffering_maxed(&audio.buffering));
    assert!(!audio.fixed(48_000, &mut ts).added);
    assert!(!audio.add(48_000, &mut ts, START - tick_ns).added);
    assert_eq!(ts.start, START);
    assert!(audio.queue.is_empty());

    // Fixed from 0: 45 ticks, 960 ms at 48 kHz.
    let mut audio = Audio::new(0, 45);
    let mut ts = tick(48_000);
    let step = audio.fixed(48_000, &mut ts);
    assert_eq!(
        step,
        Step {
            added: true,
            max_reached: false,
            ticks: 45,
            ms: 960,
            total_ms: 960,
        }
    );
    assert_eq!(audio.buffering.total_ticks, 45);
    assert_eq!(audio.buffering.wait_ticks, 45);
    assert_eq!(audio.buffering.buffered_ts, START);
    assert_eq!(audio.queue.len(), 45);
    assert_eq!(ts.start, START - audio_frames_to_ns(48_000, 45 * FRAMES));

    // 5 ticks late with room for 2: cut at the maximum.
    let mut audio = Audio::new(43, 45);
    let mut ts = tick(48_000);
    let step = audio.add(48_000, &mut ts, START - 5 * tick_ns);
    assert_eq!(
        step,
        Step {
            added: true,
            max_reached: true,
            ticks: 2,
            ms: 2 * 1024 * 1000 / 48_000,
            total_ms: 960,
        }
    );
    assert_eq!(audio.buffering.wait_ticks, 2);

    // One sample late at 44.1 kHz: one tick, 23 ms.
    let mut audio = Audio::new(0, 45);
    let mut ts = tick(44_100);
    let step = audio.add(44_100, &mut ts, START - audio_frames_to_ns(44_100, 1) - 1);
    assert_eq!(
        step,
        Step {
            added: true,
            max_reached: false,
            ticks: 1,
            ms: 1024 * 1000 / 44_100,
            total_ms: 1024 * 1000 / 44_100,
        }
    );

    // Sub-sample late: added with 0 ticks and 0 ms, ts unchanged.
    let mut audio = Audio::new(0, 45);
    let original = tick(48_000);
    let mut ts = original;
    let step = audio.add(48_000, &mut ts, START - 1);
    assert_eq!(
        step,
        Step {
            added: true,
            ..Step::default()
        }
    );
    assert_eq!(ts, original);
    assert_eq!(audio.buffering.buffered_ts, START);
    assert!(audio.queue.is_empty());
}

#[test]
fn edge_audio_after_frame_1024_adds_nothing() {
    // A window two ticks long, audio on frame 1500: the C makes an
    // out-of-range pointer; the port adds nothing.
    let ts = TsInfo {
        start: START,
        end: START + audio_frames_to_ns(48_000, 2 * FRAMES),
    };
    assert_eq!(
        mix_start(START + audio_frames_to_ns(48_000, 1500) + 1, ts, 48_000),
        None
    );
    assert_eq!(mix_start(START, ts, 48_000), Some(0));
}

#[test]
fn edge_mix_into_short_slices() {
    // The add loop stops at the end of either slice.
    let mut dst = vec![1.0_f32; 8];
    mix_into(&mut dst, &[2.0; 3], 6);
    assert_eq!(dst, [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 3.0, 3.0]);
    let mut dst = vec![1.0_f32; 4];
    mix_into(&mut dst, &[2.0; 2], 0);
    assert_eq!(dst, [3.0, 3.0, 1.0, 1.0]);
    let mut dst = vec![1.0_f32; 4];
    mix_into(&mut dst, &[2.0; 4], 9);
    assert_eq!(dst, [1.0; 4]);
}

#[test]
fn edge_rate_zero() {
    // The C divides by 0. Here frames, windows and ms are 0.
    assert_eq!(time_to_frames(0, 123_456), 0);
    assert_eq!(mix_start(START + 1, tick(48_000), 0), Some(0));

    let mut audio = Audio::new(0, 3);
    let mut ts = tick(48_000);
    let step = audio.fixed(0, &mut ts);
    assert_eq!(step.ticks, 3);
    assert_eq!((step.ms, step.total_ms), (0, 0));
    assert_eq!(
        audio.queue,
        vec![
            TsInfo {
                start: START,
                end: START
            };
            3
        ]
    );
    assert_eq!(
        ts,
        TsInfo {
            start: START,
            end: START
        }
    );

    let mut audio = Audio::new(0, 3);
    let mut ts = tick(48_000);
    let step = audio.add(0, &mut ts, START - 1_000_000);
    assert_eq!((step.ticks, step.ms, step.total_ms), (0, 0, 0));
    assert!(audio.queue.is_empty());
}

#[test]
fn edge_total_above_max_pushes_nothing() {
    // The C loops on a negative tick count. Here no window is pushed, and
    // the totals and ts are set as the C sets them before its loop.
    let mut audio = Audio::new(50, 45);
    audio.buffering.wait_ticks = 1;
    audio.buffering.buffered_ts = START;
    let mut ts = tick(48_000);
    let step = audio.fixed(48_000, &mut ts);
    assert!(step.added);
    assert_eq!(step.ticks, -5);
    assert_eq!(audio.buffering.total_ticks, 45);
    assert_eq!(audio.buffering.wait_ticks, 1);
    assert!(audio.queue.is_empty());
    assert_eq!(ts.start, START - audio_frames_to_ns(48_000, FRAMES));
    assert_eq!(step.ms, (-5_i32 * 1024 * 1000) as isize as usize / 48_000);
}

#[test]
fn edge_timestamps_wrap() {
    // ts->start - min_ts and buffered_ts - ns wrap as uint64_t does.
    let mut audio = Audio::new(0, 2);
    let mut ts = TsInfo { start: 5, end: 10 };
    let step = audio.add(48_000, &mut ts, u64::MAX);
    assert_eq!(step.ticks, 0);
    assert_eq!(ts.start, 5);

    let mut audio = Audio::new(0, 2);
    let mut ts = TsInfo { start: 5, end: 10 };
    audio.fixed(48_000, &mut ts);
    assert_eq!(
        ts.start,
        5_u64.wrapping_sub(audio_frames_to_ns(48_000, 2 * FRAMES))
    );
}

proptest! {
    #[test]
    fn edge_no_input_panics(
        audio_ts in any::<u64>(),
        start in any::<u64>(),
        end in any::<u64>(),
        min_ts in any::<u64>(),
        rate in prop_oneof![Just(0_usize), 1_usize..=384_000, any::<u32>().prop_map(|r| r as usize)],
        buffered_ts in any::<u64>(),
        wait_ticks in any::<u64>(),
        total in -64_i32..=64,
        max in 0_i32..=64,
    ) {
        let ts = TsInfo { start, end };
        let _ = time_to_frames(rate, audio_ts);
        if let Some(first) = mix_start(audio_ts, ts, rate) {
            prop_assert!(first < AUDIO_OUTPUT_FRAMES);
        }
        let mut b = Buffering { buffered_ts, wait_ticks, total_ticks: total, max_ticks: max };
        let mut pushed = 0_usize;
        let mut t = ts;
        let _ = buffering_fixed(&mut b, rate, &mut t, |_| pushed += 1);
        prop_assert!(pushed <= 128);
        let mut b = Buffering { buffered_ts, wait_ticks, total_ticks: total, max_ticks: max };
        let mut t = ts;
        let _ = buffering_add(&mut b, rate, &mut t, min_ts, |_| pushed += 1);
        prop_assert!(pushed <= 256);
    }
}
