//! #1903: `libobs/obs-audio-mix.c.h` shims against the C oracle. The
//! fixtures are the shapes the audio thread runs every tick for each root
//! source: a stereo mix of a whole tick and of a tick from frame 300 into
//! all 6 mixes, the time-to-frames step, and one added buffering tick on a
//! queue that is popped back after each call, so it does not grow.

use std::ffi::c_void;
use std::hint::black_box;
use std::ptr;

use criterion::Criterion;
use obs_c_oracle::audio_mix as c_mix;
use obs_core::audio_mix::{AUDIO_OUTPUT_FRAMES, MAX_AUDIO_CHANNELS, MAX_AUDIO_MIXES};
use obs_core::ffi::audio_mix::{
    audio_output_data, obs_audio_buffering, obs_audio_buffering_add, obs_audio_mix_add,
    obs_audio_mix_time_to_frames, ts_info,
};
use obs_util::ffi::deque::{deque, deque_free, deque_pop_front};

use crate::{measure_pair, pair};

const RATE: usize = 48_000;
/// One tick at 48 kHz in ns, `audio_frames_to_ns(48000, 1024)`.
const TICK_NS: u64 = 21_333_333;
const START: u64 = 1_000_000_000;

type Planes = Box<[[[f32; AUDIO_OUTPUT_FRAMES]; MAX_AUDIO_CHANNELS]; MAX_AUDIO_MIXES]>;

fn planes(seed: f32) -> Planes {
    let mut planes =
        Box::new([[[0.0_f32; AUDIO_OUTPUT_FRAMES]; MAX_AUDIO_CHANNELS]; MAX_AUDIO_MIXES]);
    for (m, mix) in planes.iter_mut().enumerate() {
        for (c, channel) in mix.iter_mut().enumerate() {
            for (f, sample) in channel.iter_mut().enumerate() {
                *sample = seed * ((m * 31 + c * 7 + f) % 97) as f32 / 97.0;
            }
        }
    }
    planes
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

/// Mixes the same source into fresh output on both sides once and checks
/// the samples match bit for bit, then times repeated mixes into the same
/// output (the values grow, the work does not).
fn mix(criterion: &mut Criterion, name: &str, audio_ts: u64) {
    let ts = ts_info {
        start: START,
        end: START + TICK_NS,
    };
    let mut source = planes(0.25);
    let mut source_ptrs = pointers(&mut source);
    let mut rust_out = planes(1.0);
    let mut c_out = planes(1.0);
    let mut rust_mixes = pointers(&mut rust_out).map(|data| audio_output_data { data });
    let mut c_mixes = pointers(&mut c_out).map(|data| audio_output_data { data });
    let source_rows = source_ptrs.as_mut_ptr();

    let mut rust = || {
        // SAFETY: 6 mixes and 6 rows of 8 channels of 1024 floats, no overlap.
        unsafe { obs_audio_mix_add(rust_mixes.as_mut_ptr(), source_rows, audio_ts, &ts, 2, RATE) }
    };
    let mut original = || {
        // SAFETY: as above, through the C signature.
        unsafe {
            c_mix::oracle_obs_audio_mix_add(
                c_mixes.as_mut_ptr().cast::<c_void>(),
                source_rows.cast::<c_void>(),
                audio_ts,
                ptr::from_ref(&ts).cast::<c_void>(),
                2,
                RATE,
            );
        }
    };
    rust();
    original();
    assert!(
        rust_out
            .iter()
            .flatten()
            .flatten()
            .zip(c_out.iter().flatten().flatten())
            .all(|(r, c)| r.to_bits() == c.to_bits()),
        "benchmark fixture differs: {name}"
    );
    measure_pair(criterion, name, rust, original);
}

/// A queue for one side, popped back to empty after each step.
struct Queue {
    buffered_ts: u64,
    queue: deque,
    wait_ticks: u64,
    total_ticks: i32,
}

impl Queue {
    fn new() -> Box<Self> {
        Box::new(Self {
            buffered_ts: 0,
            queue: deque {
                data: ptr::null_mut(),
                size: 0,
                start_pos: 0,
                end_pos: 0,
                capacity: 0,
            },
            wait_ticks: 0,
            total_ticks: 0,
        })
    }

    fn fields(&mut self) -> obs_audio_buffering {
        obs_audio_buffering {
            buffered_ts: &mut self.buffered_ts,
            buffered_timestamps: &mut self.queue,
            buffering_wait_ticks: &mut self.wait_ticks,
            total_buffering_ticks: &mut self.total_ticks,
            max_buffering_ticks: 45,
        }
    }

    /// Back to the start state, keeping the deque's buffer.
    fn reset(&mut self) {
        while self.queue.size > 0 {
            // SAFETY: a deque of whole ts_info entries.
            unsafe { deque_pop_front(&mut self.queue, ptr::null_mut(), size_of::<ts_info>()) };
        }
        self.buffered_ts = 0;
        self.wait_ticks = 0;
        self.total_ticks = 0;
    }
}

impl Drop for Queue {
    fn drop(&mut self) {
        // SAFETY: bmem memory of this deque.
        unsafe { deque_free(&mut self.queue) };
    }
}

fn buffering(criterion: &mut Criterion) {
    let mut rust_q = Queue::new();
    let mut c_q = Queue::new();
    let late = START - TICK_NS / 2;

    let rust = || {
        let fields = rust_q.fields();
        let mut ts = ts_info {
            start: START,
            end: START + TICK_NS,
        };
        // SAFETY: valid fields of one queue, a valid ts.
        let step = unsafe { obs_audio_buffering_add(&fields, RATE, &mut ts, late) };
        rust_q.reset();
        (step.ticks, step.ms, ts)
    };
    let original = || {
        let fields = c_q.fields();
        let mut ts = ts_info {
            start: START,
            end: START + TICK_NS,
        };
        // SAFETY: as above, through the C signature.
        let step = unsafe {
            c_mix::oracle_obs_audio_buffering_add(
                ptr::from_ref(&fields).cast::<c_void>(),
                RATE,
                ptr::from_mut(&mut ts).cast::<c_void>(),
                late,
            )
        };
        c_q.reset();
        (step.ticks, step.ms, ts)
    };
    pair(
        criterion,
        "audio_mix/buffering_add_one_tick",
        rust,
        original,
    );
}

pub fn benchmarks(criterion: &mut Criterion) {
    mix(criterion, "audio_mix/mix_whole_tick", START);
    mix(criterion, "audio_mix/mix_from_frame_300", START + 6_250_000);
    pair(
        criterion,
        "audio_mix/time_to_frames",
        || obs_audio_mix_time_to_frames(RATE, black_box(START + 12_345_678)),
        // SAFETY: plain values.
        || unsafe {
            c_mix::oracle_obs_audio_mix_time_to_frames(RATE, black_box(START + 12_345_678))
        },
    );
    buffering(criterion);
}
