//! C ABI for `libobs/obs-audio-render.c.h`.
//!
//! `obs-audio.c` calls the four functions inside libobs, for each source of
//! the audio tree and each rendered source. They are declared in
//! `obs-audio-render.h` without `EXPORT`, so the shims stay hidden
//! (`libobs/cmake/rust-hidden.txt`). The values come in plainly; the fields
//! of the deduplication source come as a `struct obs_audio_monitor_dedup` by
//! pointer. The enums go back as `int`.
//!
//! Intentional difference, where C dereferences a null pointer and libobs
//! never gets there: a null `dedup` silences nothing. The safe core lists
//! the rest ([`crate::audio_render`]).

use crate::audio_render::{
    BackwardsChoice, MonitorDedup, TreeChoice, backwards, clear_mixes, silence, tree,
};

/// `enum obs_audio_tree_choice` (`obs-audio-render.h`), as `int`.
pub const OBS_AUDIO_TREE_NONE: i32 = 0;
pub const OBS_AUDIO_TREE_SKIP: i32 = 1;
pub const OBS_AUDIO_TREE_ADD: i32 = 2;
pub const OBS_AUDIO_TREE_DUPLICATE: i32 = 3;

/// `enum obs_audio_backwards_choice` (`obs-audio-render.h`), as `int`.
pub const OBS_AUDIO_BACKWARDS_NONE: i32 = 0;
pub const OBS_AUDIO_BACKWARDS_PENDING: i32 = 1;
pub const OBS_AUDIO_BACKWARDS_IGNORE: i32 = 2;

/// `struct obs_audio_monitor_dedup` (`obs-audio-render.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct ObsAudioMonitorDedup {
    pub set: bool,
    pub active: bool,
    pub muted: bool,
    pub volume: f32,
    pub mixers: u32,
}

impl From<&ObsAudioMonitorDedup> for MonitorDedup {
    fn from(d: &ObsAudioMonitorDedup) -> Self {
        Self {
            set: d.set,
            active: d.active,
            muted: d.muted,
            volume: d.volume,
            mixers: d.mixers,
        }
    }
}

/// `obs_audio_render_tree()`; returns an `enum obs_audio_tree_choice`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_audio_render_tree(
    removed: bool,
    present: bool,
    kind: u32,
    output_flags: u32,
    duplicated: bool,
) -> i32 {
    match tree(removed, present, kind, output_flags, duplicated) {
        TreeChoice::None => OBS_AUDIO_TREE_NONE,
        TreeChoice::Skip => OBS_AUDIO_TREE_SKIP,
        TreeChoice::Add => OBS_AUDIO_TREE_ADD,
        TreeChoice::Duplicate => OBS_AUDIO_TREE_DUPLICATE,
    }
}

/// `obs_audio_render_silence()`.
///
/// # Safety
///
/// `dedup` is null or a valid `struct obs_audio_monitor_dedup`. A null
/// `dedup` returns false.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_audio_render_silence(
    dedup: *const ObsAudioMonitorDedup,
    monitoring_enabled: bool,
    is_dedup_source: bool,
) -> bool {
    // SAFETY: null or valid (the contract).
    let Some(dedup) = (unsafe { dedup.as_ref() }) else {
        return false;
    };
    silence(&dedup.into(), monitoring_enabled, is_dedup_source)
}

/// `obs_audio_render_clear_mixes()`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_audio_render_clear_mixes(dedup_mixers: u32, source_mixers: u32) -> u32 {
    clear_mixes(dedup_mixers, source_mixers)
}

/// `obs_audio_render_backwards()`; returns an `enum
/// obs_audio_backwards_choice`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_audio_render_backwards(
    maxed: bool,
    audio_ts: u64,
    tick_start: u64,
    audio_render: bool,
) -> i32 {
    match backwards(maxed, audio_ts, tick_start, audio_render) {
        BackwardsChoice::None => OBS_AUDIO_BACKWARDS_NONE,
        BackwardsChoice::Pending => OBS_AUDIO_BACKWARDS_PENDING,
        BackwardsChoice::Ignore => OBS_AUDIO_BACKWARDS_IGNORE,
    }
}
