//! `libobs/obs-audio-min-ts.c.h` shims against the C oracle. The fixtures
//! are what the audio thread runs every tick for each audio source: the
//! earlier-source test, and the sufficiency check of a source with enough
//! input and a start point inside the tick.

use std::hint::black_box;

use criterion::Criterion;
use obs_c_oracle::audio_min_ts as c_min_ts;
use obs_core::ffi::audio_min_ts::{obs_audio_min_ts_earlier, obs_audio_min_ts_insufficient};

use crate::pair;

const RATE: usize = 48_000;
const START: u64 = 1_000_000_000;
/// Frame 300 of the tick at 48 kHz.
const AT: u64 = START + 6_250_000;
/// Two ticks of one channel, in bytes.
const BUFFERED: usize = 2 * 1024 * 4;

pub fn benchmarks(criterion: &mut Criterion) {
    pair(
        criterion,
        "audio_min_ts/earlier",
        || {
            let mut min_ts = black_box(START);
            // SAFETY: a valid pointer.
            let taken =
                unsafe { obs_audio_min_ts_earlier(false, black_box(START - 10), &mut min_ts) };
            (taken, min_ts)
        },
        || {
            let mut min_ts = black_box(START);
            // SAFETY: as above, through the C signature.
            let taken = unsafe {
                c_min_ts::oracle_obs_audio_min_ts_earlier(false, black_box(START - 10), &mut min_ts)
            };
            (taken, min_ts)
        },
    );
    pair(
        criterion,
        "audio_min_ts/insufficient_inside_tick",
        || {
            let mut pending = false;
            // SAFETY: a valid pointer.
            let short = unsafe {
                obs_audio_min_ts_insufficient(
                    false,
                    &mut pending,
                    black_box(AT),
                    BUFFERED,
                    RATE,
                    START,
                )
            };
            (short, pending)
        },
        || {
            let mut pending = false;
            // SAFETY: as above, through the C signature.
            let short = unsafe {
                c_min_ts::oracle_obs_audio_min_ts_insufficient(
                    false,
                    &mut pending,
                    black_box(AT),
                    BUFFERED,
                    RATE,
                    START,
                )
            };
            (short, pending)
        },
    );
}
