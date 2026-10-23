//! Tier 3: the shims against the original `libobs/obs-audio-mix.c.h`.
//!
//! The oracle (`rust/obs-c-oracle/oracle/audio_mix.c`) is the C file,
//! unmodified, with its five functions renamed. Both sides get the same
//! inputs through their C signatures, and are compared:
//!
//! - `obs_audio_mix_time_to_frames`: the frame count;
//! - `obs_audio_mix_add`: every sample of the 6 mixes and 8 channels, bit
//!   for bit, from the same random start values and source audio;
//! - `obs_audio_buffering_maxed`, `_fixed` and `_add`: the step (added,
//!   max_reached, ticks, ms, total_ms), the three fields, `ts`, and the
//!   queue: the `struct deque` fields (size, start, end, capacity) and every
//!   window in it, from the same queue the call starts with.
//!
//! Runs: the layout; every start frame at 44.1 and 48 kHz, on the frame's
//! time, one ns before and one ns after; 4096 random mixes (the window one
//! or two ticks long, audio before, on, inside, at the end of and after it,
//! 0 to 8 channels, rates from 8 kHz to 384 kHz and odd ones); 20 000
//! random fixed and added buffering steps (totals up to the maximum, waiting
//! ticks, delays from 0 ns to beyond the maximum and huge, timestamps near
//! 0 and near `u64::MAX`, 0 to 4 queued windows); 20 000 random frame
//! counts.
//!
//! Excluded, the intentional differences, tested on the shims alone in
//! `audio_mix.rs`: audio after frame 1024 of a two-tick window (the C makes
//! an out-of-range pointer: the shim must leave the mix unchanged); a rate
//! of 0 in the buffering steps (the C divides by 0); a total above the
//! maximum, or a delay whose tick count is negative as an `int` or
//! overflows the total (the C loops or is undefined: the shim must push at
//! most max - total windows); null pointers. `test_audio_buffering_subsample`
//! compiles the C into itself, so this is what compares the swap; the
//! real-mixer tests run it through libobs.

use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use core::ptr;

use obs_c_oracle::audio_mix::{
    LAYOUT_NAMES, OracleAudioBufferingStep, oracle_audio_mix_layout,
    oracle_obs_audio_buffering_add, oracle_obs_audio_buffering_fixed,
    oracle_obs_audio_buffering_maxed, oracle_obs_audio_mix_add,
    oracle_obs_audio_mix_time_to_frames,
};
use obs_core::audio_mix::{AUDIO_OUTPUT_FRAMES, MAX_AUDIO_CHANNELS, MAX_AUDIO_MIXES};
use obs_core::ffi::audio_mix::{
    audio_output_data, obs_audio_buffering, obs_audio_buffering_add, obs_audio_buffering_fixed,
    obs_audio_buffering_maxed, obs_audio_buffering_step, obs_audio_mix_add,
    obs_audio_mix_time_to_frames, ts_info,
};
use obs_media_io::audio_io::audio_frames_to_ns;
use obs_util::ffi::deque::{deque, deque_free, deque_pop_front, deque_push_back};
use obs_util::util_uint64::mul_div64;
use proptest::prelude::*;

const FRAMES: u64 = AUDIO_OUTPUT_FRAMES as u64;
const START: u64 = 1_000_000_000;

/// On MSVC x64 `util_mul_div64` is exact 128-bit math, which faults when
/// the quotient does not fit in 64 bits (the shim saturates). The inputs
/// stay where the quotient fits there.
const MSVC_X64: bool = cfg!(all(target_env = "msvc", target_arch = "x86_64"));

#[test]
fn layout() {
    let rust = [
        size_of::<ts_info>(),
        align_of::<ts_info>(),
        offset_of!(ts_info, start),
        offset_of!(ts_info, end),
        size_of::<audio_output_data>(),
        align_of::<audio_output_data>(),
        offset_of!(audio_output_data, data),
        size_of::<obs_audio_buffering>(),
        align_of::<obs_audio_buffering>(),
        offset_of!(obs_audio_buffering, buffered_ts),
        offset_of!(obs_audio_buffering, buffered_timestamps),
        offset_of!(obs_audio_buffering, buffering_wait_ticks),
        offset_of!(obs_audio_buffering, total_buffering_ticks),
        offset_of!(obs_audio_buffering, max_buffering_ticks),
        size_of::<obs_audio_buffering_step>(),
        align_of::<obs_audio_buffering_step>(),
        offset_of!(obs_audio_buffering_step, added),
        offset_of!(obs_audio_buffering_step, max_reached),
        offset_of!(obs_audio_buffering_step, ticks),
        offset_of!(obs_audio_buffering_step, ms),
        offset_of!(obs_audio_buffering_step, total_ms),
        size_of::<deque>(),
        align_of::<deque>(),
        AUDIO_OUTPUT_FRAMES,
        MAX_AUDIO_MIXES,
        MAX_AUDIO_CHANNELS,
        size_of::<i32>(),
    ];
    for (which, (name, value)) in LAYOUT_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: a pure table lookup.
        assert_eq!(unsafe { oracle_audio_mix_layout(which) }, value, "{name}");
    }
    assert_eq!(
        size_of::<OracleAudioBufferingStep>(),
        size_of::<obs_audio_buffering_step>()
    );
}

/// A small deterministic generator for sample values.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// Finite floats of mixed sign and size, including exact small values.
    fn sample(&mut self) -> f32 {
        let v = self.next();
        match v % 4 {
            0 => (v >> 8) as i16 as f32,
            1 => f32::from_bits(((v >> 8) as u32 & 0x807f_ffff) | 0x3f00_0000),
            2 => f32::from_bits(((v >> 8) as u32 & 0x807f_ffff) | 0x4500_0000),
            _ => ((v >> 8) as i32 as f32) * 1e-9,
        }
    }
}

type Planes = Vec<Vec<Vec<f32>>>;

fn planes(rng: &mut Rng) -> Planes {
    (0..MAX_AUDIO_MIXES)
        .map(|_| {
            (0..MAX_AUDIO_CHANNELS)
                .map(|_| (0..AUDIO_OUTPUT_FRAMES).map(|_| rng.sample()).collect())
                .collect()
        })
        .collect()
}

fn pointers(planes: &mut Planes) -> [[*mut f32; MAX_AUDIO_CHANNELS]; MAX_AUDIO_MIXES] {
    let mut out = [[ptr::null_mut(); MAX_AUDIO_CHANNELS]; MAX_AUDIO_MIXES];
    for (row, mix) in out.iter_mut().zip(planes.iter_mut()) {
        for (p, channel) in row.iter_mut().zip(mix.iter_mut()) {
            *p = channel.as_mut_ptr();
        }
    }
    out
}

/// Runs `obs_audio_mix_add` on both sides from the same start values and
/// source audio and compares every sample bit for bit. When the C would
/// make an out-of-range pointer (a start frame above 1024), only the shim
/// runs, and the mix must stay as it was.
fn compare_mix(seed: u64, audio_ts: u64, ts: ts_info, channels: usize, rate: usize) {
    let mut rng = Rng(seed | 1);
    let start_values = planes(&mut rng);
    let mut source = planes(&mut rng);
    let mut rust_out = start_values.clone();
    let mut c_out = start_values.clone();
    let mut source_ptrs = pointers(&mut source);

    let mut rust_mixes = pointers(&mut rust_out).map(|data| audio_output_data { data });
    // SAFETY: 6 mixes and 6 rows of 8 channels of 1024 floats, no overlap.
    unsafe {
        obs_audio_mix_add(
            rust_mixes.as_mut_ptr(),
            source_ptrs.as_mut_ptr(),
            audio_ts,
            &ts,
            channels,
            rate,
        );
    }

    let c_ub = audio_ts > ts.start
        && audio_ts < ts.end
        && mul_div64(audio_ts - ts.start, rate as u64, 1_000_000_000) > FRAMES;
    if c_ub {
        assert_eq!(
            rust_out, start_values,
            "start past frame 1024 must add nothing"
        );
        return;
    }

    let mut c_mixes = pointers(&mut c_out).map(|data| audio_output_data { data });
    // SAFETY: as above, through the C signature.
    unsafe {
        oracle_obs_audio_mix_add(
            c_mixes.as_mut_ptr().cast::<c_void>(),
            source_ptrs.as_mut_ptr().cast::<c_void>(),
            audio_ts,
            ptr::from_ref(&ts).cast::<c_void>(),
            channels,
            rate,
        );
    }
    for (m, (rm, cm)) in rust_out.iter().zip(&c_out).enumerate() {
        for (c, (rc, cc)) in rm.iter().zip(cm).enumerate() {
            for (f, (r, c_sample)) in rc.iter().zip(cc).enumerate() {
                assert_eq!(
                    r.to_bits(),
                    c_sample.to_bits(),
                    "mix {m} ch {c} frame {f}: audio_ts {audio_ts} ts {ts:?} channels {channels} rate {rate}"
                );
            }
        }
    }
}

#[test]
fn mix_on_every_start_frame() {
    for rate in [44_100, 48_000] {
        let ts = ts_info {
            start: START,
            end: START + audio_frames_to_ns(rate, FRAMES),
        };
        for frame in 0..FRAMES {
            let at = START + audio_frames_to_ns(rate, frame);
            for audio_ts in [at - 1, at, at + 1] {
                compare_mix(frame ^ (rate as u64) << 20, audio_ts, ts, 2, rate);
            }
        }
    }
}

fn window_strategy() -> impl Strategy<Value = (u64, ts_info, usize, usize, u64)> {
    let rate = prop_oneof![
        Just(8_000_usize),
        Just(22_050),
        Just(44_100),
        Just(48_000),
        Just(96_000),
        Just(192_000),
        Just(384_000),
        1_usize..=400_000,
    ];
    (
        rate,
        1_u64..=2,
        prop_oneof![
            Just(0_u64),
            1_u64..=1_000,
            any::<u64>(),
            (u64::MAX - 1_000_000_000)..=u64::MAX
        ],
        0_usize..=MAX_AUDIO_CHANNELS,
        any::<u64>(),
        0_u8..6,
        any::<u64>(),
    )
        .prop_map(|(rate, ticks, start, channels, seed, place, pick)| {
            let len = audio_frames_to_ns(rate, ticks * FRAMES);
            let ts = ts_info {
                start,
                end: start.wrapping_add(len),
            };
            let audio_ts = match place {
                0 => start.wrapping_sub(1 + pick % 1_000_000),
                1 => start,
                2 => start.wrapping_add(pick % len.max(1)),
                3 => ts.end,
                4 => ts.end.wrapping_add(pick % 1_000_000),
                _ => pick,
            };
            (audio_ts, ts, channels, rate, seed)
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    #[test]
    fn mix_matches((audio_ts, ts, channels, rate, seed) in window_strategy()) {
        compare_mix(seed, audio_ts, ts, channels, rate);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20_000))]

    #[test]
    fn time_to_frames_matches(
        rate in prop_oneof![Just(0_usize), 1_usize..=400_000, any::<u32>().prop_map(|r| r as usize)],
        t in any::<u64>(),
    ) {
        let rate = if MSVC_X64 { rate % 1_000_000_000 } else { rate };
        // SAFETY: plain values.
        prop_assert_eq!(obs_audio_mix_time_to_frames(rate, t), unsafe {
            oracle_obs_audio_mix_time_to_frames(rate, t)
        });
    }
}

/// One side of a buffering call: the fields of `struct obs_core_audio` and
/// its queue.
struct Side {
    buffered_ts: u64,
    queue: deque,
    wait_ticks: u64,
    total_ticks: i32,
}

impl Side {
    fn new(buffered_ts: u64, wait_ticks: u64, total_ticks: i32, queued: &[ts_info]) -> Self {
        let mut side = Self {
            buffered_ts,
            queue: deque {
                data: ptr::null_mut(),
                size: 0,
                start_pos: 0,
                end_pos: 0,
                capacity: 0,
            },
            wait_ticks,
            total_ticks,
        };
        for window in queued {
            // SAFETY: a zeroed deque of ts_info, one whole entry.
            unsafe {
                deque_push_back(
                    &mut side.queue,
                    ptr::from_ref(window).cast::<c_void>(),
                    size_of::<ts_info>(),
                );
            }
        }
        side
    }

    fn fields(&mut self, max: i32) -> obs_audio_buffering {
        obs_audio_buffering {
            buffered_ts: &mut self.buffered_ts,
            buffered_timestamps: &mut self.queue,
            buffering_wait_ticks: &mut self.wait_ticks,
            total_buffering_ticks: &mut self.total_ticks,
            max_buffering_ticks: max,
        }
    }

    /// The deque state, then every window in it, front first. Frees it.
    fn drain(mut self) -> ((usize, usize, usize, usize), Vec<ts_info>) {
        let shape = (
            self.queue.size,
            self.queue.start_pos,
            self.queue.end_pos,
            self.queue.capacity,
        );
        let mut windows = Vec::new();
        while self.queue.size >= size_of::<ts_info>() {
            let mut window = ts_info::default();
            // SAFETY: a deque of whole ts_info entries.
            unsafe {
                deque_pop_front(
                    &mut self.queue,
                    ptr::from_mut(&mut window).cast::<c_void>(),
                    size_of::<ts_info>(),
                );
            }
            windows.push(window);
        }
        // SAFETY: bmem memory of this deque.
        unsafe { deque_free(&mut self.queue) };
        (shape, windows)
    }
}

#[derive(Debug, Clone)]
struct BufferingCase {
    add: bool,
    rate: usize,
    max: i32,
    total: i32,
    wait_ticks: u64,
    buffered_ts: u64,
    ts: ts_info,
    min_ts: u64,
    queued: Vec<ts_info>,
}

fn buffering_strategy() -> impl Strategy<Value = BufferingCase> {
    let rate = prop_oneof![
        Just(8_000_usize),
        Just(22_050),
        Just(44_100),
        Just(48_000),
        Just(96_000),
        Just(192_000),
        1_usize..=400_000,
    ];
    let time = prop_oneof![
        Just(0_u64),
        0_u64..=100_000_000,
        any::<u64>(),
        (u64::MAX - 100_000_000)..=u64::MAX,
        Just(START),
    ];
    (
        any::<bool>(),
        rate,
        0_i32..=64,
        any::<u32>(),
        prop_oneof![Just(0_u64), 1_u64..=64, any::<u64>()],
        time.clone(),
        time,
        any::<u64>(),
        0_u8..5,
        any::<u64>(),
        prop::collection::vec(any::<(u64, u64)>(), 0..=4),
    )
        .prop_map(
            |(
                add,
                rate,
                max,
                total_pick,
                wait_ticks,
                buffered_ts,
                start,
                end_len,
                delay_kind,
                delay_pick,
                queued,
            )| {
                let total = (total_pick % (max as u32 + 1)) as i32;
                let wait_ticks = if MSVC_X64 {
                    wait_ticks % 1_000_000
                } else {
                    wait_ticks
                };
                let tick = audio_frames_to_ns(rate, FRAMES);
                let delay = match delay_kind {
                    0 => delay_pick % 2,
                    1 => delay_pick % tick.max(1),
                    2 => delay_pick % (tick.saturating_mul(70)).max(1),
                    3 => audio_frames_to_ns(rate, (delay_pick % 80) * FRAMES)
                        .wrapping_add(delay_pick % 3),
                    _ => delay_pick,
                };
                BufferingCase {
                    add,
                    rate,
                    max,
                    total,
                    wait_ticks,
                    buffered_ts,
                    ts: ts_info {
                        start,
                        end: start.wrapping_add(end_len % (2 * tick).max(1)),
                    },
                    min_ts: start.wrapping_sub(delay),
                    queued: queued
                        .into_iter()
                        .map(|(start, end)| ts_info { start, end })
                        .collect(),
                }
            },
        )
}

/// Whether the C's tick count for `case` is defined: not negative as an
/// `int`, and its sum with the total fits an `int`.
fn c_defined(case: &BufferingCase) -> bool {
    if !case.add {
        return true;
    }
    let frames = mul_div64(
        case.ts.start.wrapping_sub(case.min_ts),
        case.rate as u64,
        1_000_000_000,
    );
    let ticks = (frames.wrapping_add(FRAMES - 1) / FRAMES) as i32;
    ticks >= 0 && case.total.checked_add(ticks).is_some()
}

fn compare_buffering(case: &BufferingCase) {
    let mut rust = Side::new(case.buffered_ts, case.wait_ticks, case.total, &case.queued);
    let mut rust_ts = case.ts;
    let rust_fields = rust.fields(case.max);
    // SAFETY: valid fields of one side, a valid ts.
    let (rust_maxed, rust_step) = unsafe {
        let maxed = obs_audio_buffering_maxed(&rust_fields);
        let step = if case.add {
            obs_audio_buffering_add(&rust_fields, case.rate, &mut rust_ts, case.min_ts)
        } else {
            obs_audio_buffering_fixed(&rust_fields, case.rate, &mut rust_ts)
        };
        (maxed, step)
    };
    let rust_state = (rust.buffered_ts, rust.wait_ticks, rust.total_ticks);
    let rust_queue = rust.drain();

    if !c_defined(case) {
        let pushed = rust_queue.1.len().saturating_sub(case.queued.len());
        assert!(
            pushed as i64 <= i64::from(case.max - case.total),
            "{case:?}"
        );
        return;
    }

    let mut c = Side::new(case.buffered_ts, case.wait_ticks, case.total, &case.queued);
    let mut c_ts = case.ts;
    let c_fields = c.fields(case.max);
    // SAFETY: as above, through the C signatures.
    let (c_maxed, c_step) = unsafe {
        let fields = ptr::from_ref(&c_fields).cast::<c_void>();
        let ts = ptr::from_mut(&mut c_ts).cast::<c_void>();
        let maxed = oracle_obs_audio_buffering_maxed(fields);
        let step = if case.add {
            oracle_obs_audio_buffering_add(fields, case.rate, ts, case.min_ts)
        } else {
            oracle_obs_audio_buffering_fixed(fields, case.rate, ts)
        };
        (maxed, step)
    };
    let c_state = (c.buffered_ts, c.wait_ticks, c.total_ticks);
    let c_queue = c.drain();

    assert_eq!(rust_maxed, c_maxed, "{case:?}");
    let rust_step = OracleAudioBufferingStep {
        added: rust_step.added,
        max_reached: rust_step.max_reached,
        ticks: rust_step.ticks,
        ms: rust_step.ms,
        total_ms: rust_step.total_ms,
    };
    assert_eq!(rust_step, c_step, "{case:?}");
    assert_eq!(rust_state, c_state, "{case:?}");
    assert_eq!(rust_ts, c_ts, "{case:?}");
    assert_eq!(rust_queue, c_queue, "{case:?}");
}

#[test]
fn buffering_chosen_cases() {
    let tick = audio_frames_to_ns(48_000, FRAMES);
    for add in [false, true] {
        for (total, max) in [(0, 45), (43, 45), (45, 45), (0, 0), (0, 1)] {
            for wait_ticks in [0, 1, 7] {
                for delay in [0, 1, tick - 1, tick, tick + 1, 5 * tick, 100 * tick] {
                    compare_buffering(&BufferingCase {
                        add,
                        rate: 48_000,
                        max,
                        total,
                        wait_ticks,
                        buffered_ts: 100 * START + 3 * tick,
                        ts: ts_info {
                            start: 100 * START,
                            end: 100 * START + tick,
                        },
                        min_ts: 100 * START - delay,
                        queued: vec![ts_info { start: 1, end: 2 }; wait_ticks.min(3) as usize],
                    });
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20_000))]

    #[test]
    fn buffering_matches(case in buffering_strategy()) {
        compare_buffering(&case);
    }
}
