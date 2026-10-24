//! C ABI for `libobs/obs-source-volume.c.h`, the volume decisions of
//! `get_source_volume()` and `apply_audio_volume()`, declared in
//! `libobs/obs-source-volume.h` without `EXPORT`.
//!
//! `get_source_volume()` and `apply_audio_volume()` in `obs-source.c.h`
//! (still C) read the source's fields themselves, in the C's order, pass
//! the values to these shims and act on the answer: the memset, the
//! multiply and `apply_audio_actions()` stay C. So `struct obs_source` is
//! never mirrored, no pointer crosses, and the shims take no lock, call
//! nothing in libobs and touch no memory. They stay hidden
//! (`rust-hidden.txt`).
//!
//! Intentional difference: `obs_source_volume_mixes()` with a `mix` of 31
//! or more, where the C's `1 << mix` on an `int` is undefined, answers for
//! bit 31 and then for no mix (libobs passes `mix < MAX_AUDIO_MIXES`).

use crate::volume_apply::{self as core, Step};

/// `enum obs_source_volume_op`, as the C's enum is returned (an `int` or
/// `unsigned int`; every value fits both).
#[allow(non_camel_case_types)]
pub type obs_source_volume_op_t = u32;

/// `OBS_SOURCE_VOLUME_SKIP`.
pub const OBS_SOURCE_VOLUME_SKIP: obs_source_volume_op_t = 0;
/// `OBS_SOURCE_VOLUME_CLEAR`.
pub const OBS_SOURCE_VOLUME_CLEAR: obs_source_volume_op_t = 1;
/// `OBS_SOURCE_VOLUME_MULTIPLY`.
pub const OBS_SOURCE_VOLUME_MULTIPLY: obs_source_volume_op_t = 2;

/// `float obs_source_volume_snap(bool muted, float volume)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_volume_snap(muted: bool, volume: f32) -> f32 {
    core::snap(muted, volume)
}

/// `bool obs_source_volume_action_due(uint64_t action_ts, uint64_t
/// audio_ts, uint64_t duration)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_volume_action_due(
    action_ts: u64,
    audio_ts: u64,
    duration: u64,
) -> bool {
    core::action_due(action_ts, audio_ts, duration)
}

/// `enum obs_source_volume_op obs_source_volume_step(float vol, uint32_t
/// mixers)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_volume_step(vol: f32, mixers: u32) -> obs_source_volume_op_t {
    match core::step(vol, mixers) {
        Step::Skip => OBS_SOURCE_VOLUME_SKIP,
        Step::Clear => OBS_SOURCE_VOLUME_CLEAR,
        Step::Multiply => OBS_SOURCE_VOLUME_MULTIPLY,
    }
}

/// `bool obs_source_volume_mixes(uint32_t audio_mixers, uint32_t mixers,
/// size_t mix)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_volume_mixes(audio_mixers: u32, mixers: u32, mix: usize) -> bool {
    core::mixes(audio_mixers, mixers, mix)
}
