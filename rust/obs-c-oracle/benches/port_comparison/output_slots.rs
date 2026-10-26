//! obs-output: the encoder slot rules and setter gates
//! (`libobs/obs-output-slots.c.h`). The setters run when an output is set
//! up, but `get_first_video_encoder_index()` and
//! `get_first_audio_encoder_index()` also run for every interleaved packet
//! and every pause, so each decision must cost what the C does. Each side
//! runs 1024 rounds over fixed inputs (occupancy masks, flag words with and
//! without the multitrack flags, indexes in and out of range, encoder
//! types, scaled sizes), asking every decision once per round as the
//! setters, getters and `obs_output_set_mixer()` ask them; nothing is
//! allocated, and the results fold into a checksum both must match.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::output_slots::{
    oracle_obs_output_slots_audio_index_ok, oracle_obs_output_slots_audio_slot,
    oracle_obs_output_slots_audio_type_ok, oracle_obs_output_slots_default_index,
    oracle_obs_output_slots_first_audio, oracle_obs_output_slots_first_video,
    oracle_obs_output_slots_mixer_in_range, oracle_obs_output_slots_mixer_mask,
    oracle_obs_output_slots_scaled_axis_set, oracle_obs_output_slots_video_index_ok,
    oracle_obs_output_slots_video_slot, oracle_obs_output_slots_video_type_ok,
};
use obs_output::ffi::output_slots::{
    obs_output_slots_audio_index_ok, obs_output_slots_audio_slot, obs_output_slots_audio_type_ok,
    obs_output_slots_default_index, obs_output_slots_first_audio, obs_output_slots_first_video,
    obs_output_slots_mixer_in_range, obs_output_slots_mixer_mask, obs_output_slots_scaled_axis_set,
    obs_output_slots_video_index_ok, obs_output_slots_video_slot, obs_output_slots_video_type_ok,
};
use std::hint::black_box;

const N: u32 = 1024;

/// Round `i`: an occupancy mask, a flag word, an index, an encoder type and
/// a scaled size.
fn round(i: u32) -> (u32, u32, usize, i32, u32) {
    let mask = (i.wrapping_mul(0x9e37_79b9) >> 7) & 0x3ff;
    let flags = 0x7 | if i.is_multiple_of(3) { 0x50 } else { 0 };
    let idx = (i % 13) as usize;
    let kind = (i % 3) as i32;
    let size = if i.is_multiple_of(5) { 0 } else { 1280 };
    (mask, flags, idx, kind, size)
}

macro_rules! side {
    ($name:ident, $call:ident) => {
        fn $name() -> u64 {
            let mut acc = 0u64;
            for i in 0..N {
                let (mask, flags, idx, kind, size) = black_box(round(i));
                let mut first = usize::MAX;
                if $call!(first_video(mask, &raw mut first)) {
                    acc = acc.wrapping_add(first as u64);
                }
                if $call!(first_audio(mask, &raw mut first)) {
                    acc = acc.wrapping_mul(3).wrapping_add(first as u64);
                }
                let mut bits = 0u64;
                for (n, b) in [
                    $call!(video_type_ok(kind)),
                    $call!(audio_type_ok(kind)),
                    $call!(video_index_ok(flags, idx)),
                    $call!(audio_index_ok(flags, idx)),
                    $call!(default_index(idx)),
                    $call!(scaled_axis_set(size)),
                    $call!(video_slot(idx)),
                    $call!(audio_slot(idx)),
                    $call!(mixer_in_range(idx)),
                ]
                .into_iter()
                .enumerate()
                {
                    bits |= u64::from(b) << n;
                }
                if $call!(mixer_in_range(idx)) {
                    acc = acc.wrapping_add($call!(mixer_mask(idx)) as u64);
                }
                acc = acc.wrapping_mul(31).wrapping_add(bits);
            }
            acc
        }
    };
}

macro_rules! shim {
    ($f:ident($($a:expr),*)) => {
        paste_shim!($f, $($a),*)
    };
}
macro_rules! oracle {
    ($f:ident($($a:expr),*)) => {
        // SAFETY: plain values; the first_* functions write one usize to a
        // local; mixer_mask is asked only below MAX_AUDIO_MIXES.
        unsafe { paste_oracle!($f, $($a),*) }
    };
}
macro_rules! paste_shim {
    // SAFETY: the first_* shims write one usize to a local.
    (first_video, $($a:expr),*) => { unsafe { obs_output_slots_first_video($($a),*) } };
    // SAFETY: as above.
    (first_audio, $($a:expr),*) => { unsafe { obs_output_slots_first_audio($($a),*) } };
    (mixer_in_range, $($a:expr),*) => { obs_output_slots_mixer_in_range($($a),*) };
    (mixer_mask, $($a:expr),*) => { obs_output_slots_mixer_mask($($a),*) };
    (video_type_ok, $($a:expr),*) => { obs_output_slots_video_type_ok($($a),*) };
    (audio_type_ok, $($a:expr),*) => { obs_output_slots_audio_type_ok($($a),*) };
    (video_index_ok, $($a:expr),*) => { obs_output_slots_video_index_ok($($a),*) };
    (audio_index_ok, $($a:expr),*) => { obs_output_slots_audio_index_ok($($a),*) };
    (default_index, $($a:expr),*) => { obs_output_slots_default_index($($a),*) };
    (scaled_axis_set, $($a:expr),*) => { obs_output_slots_scaled_axis_set($($a),*) };
    (video_slot, $($a:expr),*) => { obs_output_slots_video_slot($($a),*) };
    (audio_slot, $($a:expr),*) => { obs_output_slots_audio_slot($($a),*) };
}
macro_rules! paste_oracle {
    (first_video, $($a:expr),*) => { oracle_obs_output_slots_first_video($($a),*) };
    (first_audio, $($a:expr),*) => { oracle_obs_output_slots_first_audio($($a),*) };
    (mixer_in_range, $($a:expr),*) => { oracle_obs_output_slots_mixer_in_range($($a),*) };
    (mixer_mask, $($a:expr),*) => { oracle_obs_output_slots_mixer_mask($($a),*) };
    (video_type_ok, $($a:expr),*) => { oracle_obs_output_slots_video_type_ok($($a),*) };
    (audio_type_ok, $($a:expr),*) => { oracle_obs_output_slots_audio_type_ok($($a),*) };
    (video_index_ok, $($a:expr),*) => { oracle_obs_output_slots_video_index_ok($($a),*) };
    (audio_index_ok, $($a:expr),*) => { oracle_obs_output_slots_audio_index_ok($($a),*) };
    (default_index, $($a:expr),*) => { oracle_obs_output_slots_default_index($($a),*) };
    (scaled_axis_set, $($a:expr),*) => { oracle_obs_output_slots_scaled_axis_set($($a),*) };
    (video_slot, $($a:expr),*) => { oracle_obs_output_slots_video_slot($($a),*) };
    (audio_slot, $($a:expr),*) => { oracle_obs_output_slots_audio_slot($($a),*) };
}

side!(rust, shim);
side!(c, oracle);

pub fn benchmarks(criterion: &mut Criterion) {
    pair(criterion, "output_slots/setter_gates/1024", rust, c);
}
