//! obs-output: the encoder packet timestamp decisions
//! (`libobs/obs-encoder-packet-ts.c.h`). Every encoder asks them once per
//! packet: the timebase in `do_encode()`, the DTS in microseconds, the time
//! on the system clock and after the pause offset, and the timing entry of
//! a video packet in `send_off_encoder_packet()`. Each side runs 1024 video
//! packets at 29.97 fps with B-frames (two frames of delay, so the timing
//! list holds three entries) and folds the answers into a checksum both
//! must match. Nothing is allocated in the loop.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::encoder_packet_ts::{
    OraclePacketTime, oracle_obs_encoder_packet_ts_dts_usec, oracle_obs_encoder_packet_ts_find,
    oracle_obs_encoder_packet_ts_paused, oracle_obs_encoder_packet_ts_system,
    oracle_obs_encoder_packet_ts_timebase,
};
use obs_output::ffi::encoder_packet_ts::{
    obs_encoder_packet_ts_dts_usec, obs_encoder_packet_ts_find, obs_encoder_packet_ts_paused,
    obs_encoder_packet_ts_system, obs_encoder_packet_ts_timebase,
};
use obs_output::ffi::output_delay::encoder_packet_time;
use std::hint::black_box;

const N: i64 = 1024;
const NUM: u32 = 1001; // 29.97 fps
const DEN: u32 = 30000;
const START_NS: u64 = 5_000_000_123;
const PAUSE_NS: u64 = 2_500_000_000;
/// The timing entries pending when a packet arrives: the frame two back and
/// the two after it.
const PENDING: usize = 3;

fn fold(acc: u64, values: &[u64]) -> u64 {
    values
        .iter()
        .fold(acc, |acc, v| acc.wrapping_mul(31).wrapping_add(*v))
}

/// The timing entries pending for packet `i` (frames i .. i + 3, PTS in
/// timebase units), newest last, as `obs-encoder.c` keeps them.
fn pending(i: i64, step: i64) -> [i64; PENDING] {
    [i * step, (i + 1) * step, (i + 2) * step]
}

fn rust_packets() -> u64 {
    let mut times = [encoder_packet_time::default(); PENDING];
    let tb = obs_encoder_packet_ts_timebase(NUM, DEN, 1);
    let step = i64::from(tb.num);
    let offset = obs_encoder_packet_ts_dts_usec(-2 * step, tb.den);
    (0..N).fold(0, |acc, i| {
        let i = black_box(i);
        let tb = obs_encoder_packet_ts_timebase(NUM, DEN, 1);
        for (t, pts) in times.iter_mut().zip(pending(i, step)) {
            t.pts = pts;
        }
        let dts = (i - 2) * step;
        let usec = obs_encoder_packet_ts_dts_usec(dts, tb.den);
        let at = obs_encoder_packet_ts_system(START_NS, usec, offset);
        let sys = obs_encoder_packet_ts_paused(at, PAUSE_NS);
        // SAFETY: PENDING initialized entries, read only.
        let found = unsafe { obs_encoder_packet_ts_find(times.as_ptr(), PENDING, i * step) };
        fold(
            acc,
            &[
                u64::from(tb.num.cast_unsigned()),
                at.cast_unsigned(),
                sys.cast_unsigned(),
                u64::try_from(found).unwrap_or(u64::MAX),
            ],
        )
    })
}

fn c_packets() -> u64 {
    let mut times = [OraclePacketTime::default(); PENDING];
    // SAFETY: plain values in, plain values out; the oracle keeps no state.
    let (step, offset) = unsafe {
        let tb = oracle_obs_encoder_packet_ts_timebase(NUM, DEN, 1);
        let step = i64::from(tb.num);
        (step, oracle_obs_encoder_packet_ts_dts_usec(-2 * step, tb.den))
    };
    (0..N).fold(0, |acc, i| {
        let i = black_box(i);
        for (t, pts) in times.iter_mut().zip(pending(i, step)) {
            t.pts = pts;
        }
        // SAFETY: plain values (C is defined for them) and PENDING
        // initialized entries, read only.
        let (tb, at, sys, found) = unsafe {
            let tb = oracle_obs_encoder_packet_ts_timebase(NUM, DEN, 1);
            let usec = oracle_obs_encoder_packet_ts_dts_usec((i - 2) * step, tb.den);
            let at = oracle_obs_encoder_packet_ts_system(START_NS, usec, offset);
            let sys = oracle_obs_encoder_packet_ts_paused(at, PAUSE_NS);
            let found = oracle_obs_encoder_packet_ts_find(times.as_ptr(), PENDING, i * step);
            (tb, at, sys, found)
        };
        fold(
            acc,
            &[
                u64::from(tb.num.cast_unsigned()),
                at.cast_unsigned(),
                sys.cast_unsigned(),
                u64::try_from(found).unwrap_or(u64::MAX),
            ],
        )
    })
}

pub fn benchmarks(criterion: &mut Criterion) {
    pair(
        criterion,
        "encoder_packet_ts/packet/1024",
        rust_packets,
        c_packets,
    );
}
