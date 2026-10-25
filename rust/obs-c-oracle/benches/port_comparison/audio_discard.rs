//! #2403: `libobs/obs-audio-discard.c.h` shims against the C oracle. The
//! fixture is what the audio thread runs every tick for each audio source
//! after the mix: `discard_audio` of a whole tick from a stereo input that
//! holds two ticks, and the not-enough-data path (one tick of a source
//! still filling). After each call the popped frames are pushed back as
//! zeros, so the deques do not shrink; both sides do the same.

use std::ffi::c_void;
use std::ptr;

use criterion::Criterion;
use obs_c_oracle::audio_discard as c_discard;
use obs_core::audio_mix::{AUDIO_OUTPUT_FRAMES, MAX_AUDIO_CHANNELS};
use obs_core::ffi::audio_discard::{obs_audio_discard_tick, obs_audio_input};
use obs_core::ffi::audio_mix::ts_info;
use obs_util::ffi::deque::{deque, deque_free, deque_push_back_zero};

use crate::pair;

const RATE: usize = 48_000;
/// One tick at 48 kHz in ns, `audio_frames_to_ns(48000, 1024)`.
const TICK_NS: u64 = 21_333_333;
const START: u64 = 1_000_000_000;
const CHANNELS: usize = 2;
const TICK_BYTES: usize = AUDIO_OUTPUT_FRAMES * 4;

/// One side's source: the fields and 8 deques.
struct Source {
    bufs: [deque; MAX_AUDIO_CHANNELS],
    audio_ts: u64,
    last_size: usize,
    pending: bool,
    pending_stop: bool,
    timing_set: bool,
}

impl Source {
    fn new(frames_bytes: usize) -> Box<Self> {
        let mut source = Box::new(Self {
            bufs: core::array::from_fn(|_| deque {
                data: ptr::null_mut(),
                size: 0,
                start_pos: 0,
                end_pos: 0,
                capacity: 0,
            }),
            audio_ts: START,
            last_size: 0,
            pending: false,
            pending_stop: false,
            timing_set: true,
        });
        for dq in source.bufs.iter_mut().take(CHANNELS) {
            // SAFETY: a valid deque.
            unsafe { deque_push_back_zero(dq, frames_bytes) };
        }
        source
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

    /// Back to the start state: the popped bytes pushed back as zeros.
    fn reset(&mut self, bytes: usize) {
        for dq in self.bufs.iter_mut().take(CHANNELS) {
            let missing = bytes - dq.size;
            if missing > 0 {
                // SAFETY: a valid deque.
                unsafe { deque_push_back_zero(dq, missing) };
            }
        }
        self.audio_ts = START;
        self.last_size = 0;
        self.pending_stop = false;
    }
}

impl Drop for Source {
    fn drop(&mut self) {
        for dq in &mut self.bufs {
            // SAFETY: bmem memory of this deque.
            unsafe { deque_free(dq) };
        }
    }
}

fn discard(criterion: &mut Criterion, name: &str, bytes: usize) {
    let ts = ts_info {
        start: START,
        end: START + TICK_NS,
    };
    let mut rust_src = Source::new(bytes);
    let mut c_src = Source::new(bytes);
    let rust = || {
        let input = rust_src.input();
        // SAFETY: valid fields and deques, a valid ts.
        let step = unsafe { obs_audio_discard_tick(&input, false, CHANNELS, RATE, &ts) };
        let out = (step.path, rust_src.audio_ts, rust_src.bufs[1].size);
        rust_src.reset(bytes);
        out
    };
    let original = || {
        let input = c_src.input();
        // SAFETY: as above, through the C signature.
        let step = unsafe {
            c_discard::oracle_obs_audio_discard_tick(
                ptr::from_ref(&input).cast::<c_void>(),
                false,
                CHANNELS,
                RATE,
                ptr::from_ref(&ts).cast::<c_void>(),
            )
        };
        let out = (step.path, c_src.audio_ts, c_src.bufs[1].size);
        c_src.reset(bytes);
        out
    };
    pair(criterion, name, rust, original);
}

pub fn benchmarks(criterion: &mut Criterion) {
    discard(
        criterion,
        "audio_discard/discard_tick_stereo",
        2 * TICK_BYTES,
    );
    discard(
        criterion,
        "audio_discard/pending_tick_stereo",
        TICK_BYTES / 2,
    );
}
