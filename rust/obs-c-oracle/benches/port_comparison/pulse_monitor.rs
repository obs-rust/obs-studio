//! obs-audio-monitoring: the PulseAudio monitor's write steps
//! (`libobs/audio-monitoring/pulse/pulseaudio-monitor-steps.c.h`). They run
//! for each audio packet of every monitored source (about 47 a second at
//! 48 kHz), inside the PulseAudio main-loop lock and the playback mutex, so
//! each must cost what the C does. Each side runs 1024 packets through what
//! `on_audio_playback()` and `do_stream_write()` ask: the activate refs, the
//! bytes, the volume, the grow and cork rules and the write loop against a
//! server that asks for fixed amounts. Nothing is allocated, and the results
//! fold into a checksum both must match.

use super::pair;

use criterion::Criterion;
use obs_audio_monitoring::ffi::pulse_format::uint_fast32_t;
use obs_audio_monitoring::ffi::pulse_monitor::{
    pulseaudio_monitor_grow, pulseaudio_monitor_grown_tlength, pulseaudio_monitor_needs_volume,
    pulseaudio_monitor_playback_bytes, pulseaudio_monitor_playback_idle, pulseaudio_monitor_uncork,
    pulseaudio_monitor_write_more, pulseaudio_monitor_write_short,
};
use obs_c_oracle::pulse_monitor_steps::{
    oracle_pulse_monitor_playback_bytes, oracle_pulseaudio_monitor_grow,
    oracle_pulseaudio_monitor_grown_tlength, oracle_pulseaudio_monitor_needs_volume,
    oracle_pulseaudio_monitor_playback_idle, oracle_pulseaudio_monitor_uncork,
    oracle_pulseaudio_monitor_write_more, oracle_pulseaudio_monitor_write_short,
};
use std::ffi::c_long;
use std::hint::black_box;

const N: u32 = 1024;
/// 25 ms of 48 kHz stereo float.
const TLENGTH: u32 = 9600;
/// Stereo float.
const BPF: u64 = 8;

/// Packet `i`: its activate refs, frames, volume and what the server asks
/// for each write.
fn packet(i: u32) -> (c_long, u32, f32, usize) {
    let refs = c_long::from(!i.is_multiple_of(17));
    let frames = 480 + (i % 5) * 32;
    let vol = if i.is_multiple_of(3) {
        1.0
    } else {
        0.5 + (i % 7) as f32 / 10.0
    };
    let ask = 1024 + (i as usize % 9) * 512;
    (refs, frames, vol, ask)
}

macro_rules! side {
    ($name:ident, $idle:expr, $bytes:expr, $vol:expr, $grow:expr, $grown:expr, $uncork:expr, $more:expr, $short:expr) => {
        fn $name() -> u64 {
            let mut queued = 0usize;
            let mut tlength = TLENGTH;
            let mut corked = true;
            let mut acc = 0u64;
            for i in 0..N {
                let (refs, frames, vol, ask) = black_box(packet(i));
                if !$idle(refs) {
                    queued += $bytes(black_box(BPF), frames);
                    if $vol(vol) {
                        acc = acc.wrapping_add(1);
                    }
                }
                if $grow(queued, tlength) {
                    tlength = $grown(queued);
                    acc = acc.wrapping_mul(31).wrapping_add(u64::from(tlength));
                }
                if corked {
                    if !$uncork(queued, tlength) {
                        continue;
                    }
                    corked = false;
                }
                let mut writes = 0;
                while $more(queued) && writes < 4 {
                    if $short(ask, queued) {
                        acc = acc.wrapping_add(7);
                        break;
                    }
                    queued -= ask;
                    writes += 1;
                    acc = acc.wrapping_mul(31).wrapping_add(queued as u64);
                }
            }
            acc.wrapping_add(queued as u64)
        }
    };
}

fn shim_bytes(b: u64, f: u32) -> usize {
    pulseaudio_monitor_playback_bytes(b as uint_fast32_t, f)
}

side!(
    rust,
    pulseaudio_monitor_playback_idle,
    shim_bytes,
    pulseaudio_monitor_needs_volume,
    pulseaudio_monitor_grow,
    pulseaudio_monitor_grown_tlength,
    pulseaudio_monitor_uncork,
    pulseaudio_monitor_write_more,
    pulseaudio_monitor_write_short
);

// SAFETY (each closure): plain values; nothing in the oracle divides.
side!(
    c,
    |r| unsafe { oracle_pulseaudio_monitor_playback_idle(r) },
    |b, f| unsafe { oracle_pulse_monitor_playback_bytes(b, f) },
    |v| unsafe { oracle_pulseaudio_monitor_needs_volume(v) },
    |q, t| unsafe { oracle_pulseaudio_monitor_grow(q, t) },
    |q| unsafe { oracle_pulseaudio_monitor_grown_tlength(q) },
    |q, t| unsafe { oracle_pulseaudio_monitor_uncork(q, t) },
    |q| unsafe { oracle_pulseaudio_monitor_write_more(q) },
    |a, q| unsafe { oracle_pulseaudio_monitor_write_short(a, q) }
);

pub fn benchmarks(criterion: &mut Criterion) {
    pair(criterion, "pulse_monitor/stream_write/1024", rust, c);
}
