//! obs-source: the source audio placement and action timeline
//! (`libobs/obs-source-audio-place.c.h`). Every audio packet a source
//! outputs asks the placement decisions, and every tick with queued audio
//! actions asks the timeline. Each side runs 1024 packets around an audio
//! timestamp at 48 kHz (no audio_ts yet, older, at it, a sample, a block and
//! seconds later, near `MAX_BUF_SIZE`; every monitoring type; push back on
//! and off), each asking the decisions of the last step of
//! `source_output_audio_data()` and of `source_output_audio_place()` with
//! two channels in order, then one action of `apply_audio_actions()` and
//! its six mixes; nothing is allocated, and the results fold into a
//! checksum both must match.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::source_audio_place as oracle;
use obs_source::ffi::source_audio_place as shim;
use std::hint::black_box;
use std::os::raw::c_int;

const N: u64 = 1024;
const BASE: u64 = 1_000_000_000_000;
const RATE: u32 = 48000;

/// Packet `i`: the source's audio_ts, the packet's timestamp and frames,
/// the monitoring type, push back, a channel size and an action timestamp.
fn packet(i: u64) -> (u64, u64, u32, c_int, bool, usize, u64) {
    let audio_ts = if i.is_multiple_of(7) { 0 } else { BASE };
    let step = [0, 1, 20834, 21_333_334, 1_000_000_000, 21_312_000_000];
    let ts = BASE
        .wrapping_add(step[usize::try_from(i % 6).unwrap_or(0)])
        .wrapping_sub(i % 5 / 4);
    let frames = [480, 1024, 1_024_000][usize::try_from(i % 3).unwrap_or(0)];
    let monitoring = c_int::try_from(i % 3).unwrap_or(0);
    let action = BASE.wrapping_add((i % 1100) * 20833).wrapping_sub(i % 2);
    (
        audio_ts,
        ts,
        frames,
        monitoring,
        i & 8 != 0,
        usize::try_from(i * 4096).unwrap_or(0),
        action,
    )
}

fn fold(acc: u64, values: [usize; 16]) -> u64 {
    values.iter().fold(acc, |a, &v| {
        a.wrapping_mul(31)
            .wrapping_add(u64::try_from(v).unwrap_or(0))
    })
}

fn rust() -> u64 {
    (0..N).fold(0, |acc, i| {
        let (audio_ts, ts, frames, monitoring, push_back, buf, action) = black_box(packet(i));
        let size = shim::obs_source_audio_place_size(frames);
        let placement = shim::obs_source_audio_place_offset(ts, audio_ts, RATE);
        let frame = shim::obs_source_audio_place_action_frame(action, audio_ts, RATE as usize);
        fold(
            acc,
            [
                usize::from(shim::obs_source_audio_place_outputs(monitoring)),
                usize::from(push_back && shim::obs_source_audio_place_started(audio_ts)),
                usize::from(shim::obs_source_audio_place_older(ts, audio_ts)),
                size,
                placement,
                usize::from(shim::obs_source_audio_place_fits(placement, size)),
                shim::obs_source_audio_place_pop_back(buf, placement, size),
                shim::obs_source_audio_place_pop_back(buf + 4, placement, size),
                usize::from(shim::obs_source_audio_place_fits(buf, size)),
                frame,
                usize::from(shim::obs_source_audio_place_action_stops(frame)),
                shim::obs_source_audio_place_fill_end(
                    usize::try_from(i % 1024).unwrap_or(0),
                    frame,
                ),
                usize::from(shim::obs_source_audio_place_mix_on(
                    u32::try_from(i & 0x3f).unwrap_or(0),
                    0,
                )) + usize::from(shim::obs_source_audio_place_mix_on(
                    u32::try_from(i & 0x3f).unwrap_or(0),
                    1,
                )),
                usize::from(shim::obs_source_audio_place_mix_on(
                    u32::try_from(i & 0x3f).unwrap_or(0),
                    2,
                )) + usize::from(shim::obs_source_audio_place_mix_on(
                    u32::try_from(i & 0x3f).unwrap_or(0),
                    3,
                )),
                usize::from(shim::obs_source_audio_place_mix_on(
                    u32::try_from(i & 0x3f).unwrap_or(0),
                    4,
                )),
                usize::from(shim::obs_source_audio_place_mix_on(
                    u32::try_from(i & 0x3f).unwrap_or(0),
                    5,
                )),
            ],
        )
    })
}

fn c() -> u64 {
    (0..N).fold(0, |acc, i| {
        let (audio_ts, ts, frames, monitoring, push_back, buf, action) = black_box(packet(i));
        // SAFETY: plain values in and out; the C touches no memory.
        let values = unsafe {
            let size = oracle::oracle_obs_source_audio_place_size(frames);
            let placement = oracle::oracle_obs_source_audio_place_offset(ts, audio_ts, RATE);
            let frame =
                oracle::oracle_obs_source_audio_place_action_frame(action, audio_ts, RATE as usize);
            let mask = u32::try_from(i & 0x3f).unwrap_or(0);
            [
                usize::from(oracle::oracle_obs_source_audio_place_outputs(monitoring)),
                usize::from(
                    push_back && oracle::oracle_obs_source_audio_place_started(audio_ts) != 0,
                ),
                usize::from(oracle::oracle_obs_source_audio_place_older(ts, audio_ts)),
                size,
                placement,
                usize::from(oracle::oracle_obs_source_audio_place_fits(placement, size)),
                oracle::oracle_obs_source_audio_place_pop_back(buf, placement, size),
                oracle::oracle_obs_source_audio_place_pop_back(buf + 4, placement, size),
                usize::from(oracle::oracle_obs_source_audio_place_fits(buf, size)),
                frame,
                usize::from(oracle::oracle_obs_source_audio_place_action_stops(frame)),
                oracle::oracle_obs_source_audio_place_fill_end(
                    usize::try_from(i % 1024).unwrap_or(0),
                    frame,
                ),
                usize::from(oracle::oracle_obs_source_audio_place_mix_on(mask, 0))
                    + usize::from(oracle::oracle_obs_source_audio_place_mix_on(mask, 1)),
                usize::from(oracle::oracle_obs_source_audio_place_mix_on(mask, 2))
                    + usize::from(oracle::oracle_obs_source_audio_place_mix_on(mask, 3)),
                usize::from(oracle::oracle_obs_source_audio_place_mix_on(mask, 4)),
                usize::from(oracle::oracle_obs_source_audio_place_mix_on(mask, 5)),
            ]
        };
        fold(acc, values)
    })
}

pub fn benchmarks(criterion: &mut Criterion) {
    pair(
        criterion,
        "source_audio_place/source_output_audio_place/1024",
        rust,
        c,
    );
}
