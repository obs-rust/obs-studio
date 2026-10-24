//! obs-source: the audio render mix plan
//! (`libobs/obs-source-audio-render.c.h`), which
//! `obs_source_audio_render()` runs for every audio source on every tick of
//! the mix. The measurement folds its results into a checksum that both
//! sides must match:
//! - `render`: 1024 sources through `obs_source_audio_render()`,
//!   `custom_audio_render()` and `process_audio_source_tick()` of
//!   `obs-source.c` on a stand-in source (the route, the clears around a
//!   custom render's callback and its result, the tick's pending check and
//!   the copy or clear of each mix), with every combination of the six
//!   source flags, mixer masks from none to all and input sizes below, at
//!   and above the tick; the buffers, the callbacks and the volume are not
//!   run.
//!
//! Measured when it was added (x86_64 Linux, GCC, not pinned to a core;
//! median per 1024 sources): render 12.2 us Rust, 30.2 us C (0.40x). Under
//! 30 ns a source a tick either way, of a tick of 21.3 ms.

use std::hint::black_box;

use criterion::{Criterion, Throughput};
use obs_c_oracle::source_audio_render as c;
use obs_source::ffi::source_audio_render as rs;

const COUNT: usize = 1024;
const MIXES: usize = 6;
const TICK_BYTES: usize = 4096;

struct Side {
    route: fn(bool, bool, bool) -> u32,
    custom_clear_before: fn(u32, u32, usize) -> bool,
    custom_ts: fn(bool, u64) -> u64,
    custom_pending: fn(bool) -> bool,
    custom_unmixed: fn(u64, u32) -> bool,
    custom_in_tick: fn(u32, usize) -> bool,
    custom_clear_after: fn(u32, usize) -> bool,
    tick_unset: fn(u64) -> bool,
    tick_short: fn(usize, usize) -> bool,
    tick_stop: fn(bool, usize) -> bool,
    tick_copy: fn(bool, u32, u32, usize) -> bool,
    tick_volume: fn(bool) -> bool,
    tick_clear_main: fn(u32, u32) -> bool,
}

const RUST: Side = Side {
    route: |a, b, c| rs::obs_source_audio_render_route(a, b, c),
    custom_clear_before: |a, b, c| rs::obs_source_audio_render_custom_clear_before(a, b, c),
    custom_ts: |a, b| rs::obs_source_audio_render_custom_ts(a, b),
    custom_pending: |a| rs::obs_source_audio_render_custom_pending(a),
    custom_unmixed: |a, b| rs::obs_source_audio_render_custom_unmixed(a, b),
    custom_in_tick: |a, b| rs::obs_source_audio_render_custom_in_tick(a, b),
    custom_clear_after: |a, b| rs::obs_source_audio_render_custom_clear_after(a, b),
    tick_unset: |a| rs::obs_source_audio_render_tick_unset(a),
    tick_short: |a, b| rs::obs_source_audio_render_tick_short(a, b),
    tick_stop: |a, b| rs::obs_source_audio_render_tick_stop(a, b),
    tick_copy: |a, b, c, d| rs::obs_source_audio_render_tick_copy(a, b, c, d),
    tick_volume: |a| rs::obs_source_audio_render_tick_volume(a),
    tick_clear_main: |a, b| rs::obs_source_audio_render_tick_clear_main(a, b),
};

const C: Side = Side {
    // SAFETY: plain values; the C touches no memory.
    route: |b, cu, d| unsafe { c::oracle_obs_source_audio_render_route(b, cu, d) },
    // SAFETY: as above; mix < 6.
    custom_clear_before: |a, m, x| unsafe {
        c::oracle_obs_source_audio_render_custom_clear_before(a, m, x) != 0
    },
    // SAFETY: as above.
    custom_ts: |s, t| unsafe { c::oracle_obs_source_audio_render_custom_ts(s, t) },
    // SAFETY: as above.
    custom_pending: |s| unsafe { c::oracle_obs_source_audio_render_custom_pending(s) != 0 },
    // SAFETY: as above.
    custom_unmixed: |t, m| unsafe { c::oracle_obs_source_audio_render_custom_unmixed(t, m) != 0 },
    // SAFETY: as above; mix < 6.
    custom_in_tick: |m, x| unsafe { c::oracle_obs_source_audio_render_custom_in_tick(m, x) != 0 },
    // SAFETY: as above; mix < 6.
    custom_clear_after: |a, x| unsafe {
        c::oracle_obs_source_audio_render_custom_clear_after(a, x) != 0
    },
    // SAFETY: as above.
    tick_unset: |t| unsafe { c::oracle_obs_source_audio_render_tick_unset(t) != 0 },
    // SAFETY: as above.
    tick_short: |i, s| unsafe { c::oracle_obs_source_audio_render_tick_short(i, s) != 0 },
    // SAFETY: as above.
    tick_stop: |s, x| unsafe { c::oracle_obs_source_audio_render_tick_stop(s, x) != 0 },
    // SAFETY: as above; mix < 6.
    tick_copy: |s, a, m, x| unsafe { c::oracle_obs_source_audio_render_tick_copy(s, a, m, x) != 0 },
    // SAFETY: as above.
    tick_volume: |s| unsafe { c::oracle_obs_source_audio_render_tick_volume(s) != 0 },
    // SAFETY: as above.
    tick_clear_main: |a, m| unsafe { c::oracle_obs_source_audio_render_tick_clear_main(a, m) != 0 },
};

#[derive(Clone, Copy)]
struct Source {
    bits: u8,
    audio_ts: u64,
    audio_mixers: u32,
    input_size: usize,
    cb_ts: u64,
    mixers: u32,
}

/// What the three callers would do, folded into a number.
fn render(side: &Side, s: &Source) -> u64 {
    let has_buffer = s.bits & 1 != 0;
    let custom = has_buffer && s.bits & 2 != 0;
    let has_data = custom && s.bits & 4 != 0;
    let submix = s.bits & 16 != 0;
    let mut sum = 0_u64;
    match (side.route)(has_buffer, custom, has_data) {
        0 => return 1,
        1 => {
            for mix in 0..MIXES {
                if (side.custom_clear_before)(s.audio_mixers, s.mixers, mix) {
                    sum = sum.wrapping_mul(7).wrapping_add(mix as u64 + 1);
                }
            }
            let success = s.bits & 32 != 0;
            let ts = (side.custom_ts)(success, s.cb_ts);
            sum = sum.wrapping_add(ts);
            if (side.custom_pending)(success) || (side.custom_unmixed)(ts, s.mixers) {
                return sum.wrapping_add(2);
            }
            for mix in 0..MIXES {
                if (side.custom_in_tick)(s.mixers, mix)
                    && (side.custom_clear_after)(s.audio_mixers, mix)
                {
                    sum = sum.wrapping_mul(11).wrapping_add(mix as u64 + 1);
                }
            }
            return sum.wrapping_add(3);
        }
        _ => {}
    }
    if (side.tick_unset)(s.audio_ts) || (side.tick_short)(s.input_size, TICK_BYTES) {
        return 4;
    }
    for mix in 1..MIXES {
        if (side.tick_stop)(submix, mix) {
            break;
        }
        let copy = (side.tick_copy)(submix, s.audio_mixers, s.mixers, mix);
        sum = sum
            .wrapping_mul(13)
            .wrapping_add(u64::from(copy) + mix as u64);
    }
    if !(side.tick_volume)(submix) {
        return sum.wrapping_add(5);
    }
    if (side.tick_clear_main)(s.audio_mixers, s.mixers) {
        sum = sum.wrapping_add(6);
    }
    sum.wrapping_add(7)
}

const MASKS: [u32; 8] = [0x3f, 0x01, 0x03, 0x00, 0x40, 0x3e, 0xff, u32::MAX];

fn sources() -> Vec<Source> {
    (0..COUNT)
        .map(|i| {
            let n = i as u64;
            Source {
                bits: (i % 64) as u8 | 1,
                audio_ts: if i % 11 == 0 {
                    0
                } else {
                    1_000_000_000 + n * 21_333_333
                },
                audio_mixers: MASKS[(i / 3) % MASKS.len()],
                input_size: TICK_BYTES - 1 + i % 3,
                cb_ts: if i % 13 == 0 { 0 } else { 2_000_000_000 + n },
                mixers: MASKS[(i / 7) % MASKS.len()],
            }
        })
        .collect()
}

fn run(side: &Side, sources: &[Source]) -> u64 {
    let mut sum = 0_u64;
    for s in sources {
        let r = render(side, black_box(s));
        sum = sum.wrapping_mul(31).wrapping_add(r);
    }
    sum
}

pub(crate) fn benchmarks(criterion: &mut Criterion) {
    let sources = sources();
    assert_eq!(
        run(&RUST, &sources),
        run(&C, &sources),
        "benchmark fixture differs: render"
    );

    let mut group = criterion.benchmark_group("source_audio_render/render");
    group.throughput(Throughput::Elements(COUNT as u64));
    group.bench_function("rust", |b| b.iter(|| black_box(run(&RUST, &sources))));
    group.bench_function("c", |b| b.iter(|| black_box(run(&C, &sources))));
    group.finish();
}
