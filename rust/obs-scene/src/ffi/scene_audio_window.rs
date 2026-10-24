//! C ABI for `libobs/obs-scene-audio-window.c.h`, the audio window
//! decisions of `scene_audio_render()`, declared in
//! `libobs/obs-scene-audio-window.h` without `EXPORT`.
//!
//! `scene_audio_render()` stays C (`obs-scene.c.h`): `audio_lock`,
//! `apply_scene_item_volume()` and `process_all_audio_actions()`, every
//! `obs_source_*` call, the two mix loops and every item and source field
//! read, each only where the old expressions read it. It passes plain
//! values here, so no scene, item or source pointer crosses, and these shims
//! take no lock and call nothing in libobs. All seven stay hidden
//! (`rust-hidden.txt`).
//!
//! Values cross as the C has them: `bool`, `uint64_t` as `u64`, `size_t` as
//! `usize`, the child as the C's `enum obs_scene_audio_window_child` value
//! ([`OBS_SCENE_AUDIO_WINDOW_SOURCE`], [`OBS_SCENE_AUDIO_WINDOW_SHOW`],
//! [`OBS_SCENE_AUDIO_WINDOW_HIDE`]), and the window as
//! [`obs_scene_audio_window`], returned by value.
//!
//! Intentional difference from C, where C faults and libobs never gets
//! there: on MSVC x64 the frame of a rate above 1 GHz may not fit in 64
//! bits; the C faults in `_udiv128`, the core saturates and skips the item.

use std::os::raw::c_int;

use crate::audio_window::{self as core, Child};

/// `OBS_SCENE_AUDIO_WINDOW_SOURCE`: `item->source`.
pub const OBS_SCENE_AUDIO_WINDOW_SOURCE: c_int = 0;
/// `OBS_SCENE_AUDIO_WINDOW_SHOW`: `item->show_transition`.
pub const OBS_SCENE_AUDIO_WINDOW_SHOW: c_int = 1;
/// `OBS_SCENE_AUDIO_WINDOW_HIDE`: `item->hide_transition`.
pub const OBS_SCENE_AUDIO_WINDOW_HIDE: c_int = 2;

/// `struct obs_scene_audio_window` (`obs-scene-audio-window.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub struct obs_scene_audio_window {
    pub mix: bool,
    pub pos: usize,
    pub count: usize,
}

/// The C value of a [`Child`].
fn child_value(child: Child) -> c_int {
    match child {
        Child::Source => OBS_SCENE_AUDIO_WINDOW_SOURCE,
        Child::Show => OBS_SCENE_AUDIO_WINDOW_SHOW,
        Child::Hide => OBS_SCENE_AUDIO_WINDOW_HIDE,
    }
}

/// `size_t` as the core's `u64`. `size_t` is 64 bits on every target
/// libobs builds for; a wider one (none) would saturate.
fn rate(sample_rate: usize) -> u64 {
    u64::try_from(sample_rate).unwrap_or(u64::MAX)
}

/// `enum obs_scene_audio_window_child obs_scene_audio_window_pick(bool
/// visible, bool show_active, bool hide_active)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_scene_audio_window_pick(
    visible: bool,
    show_active: bool,
    hide_active: bool,
) -> c_int {
    child_value(core::pick(visible, show_active, hide_active))
}

/// `bool obs_scene_audio_window_counts(bool pending, bool visible, bool
/// hide_active)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_scene_audio_window_counts(
    pending: bool,
    visible: bool,
    hide_active: bool,
) -> bool {
    core::counts(pending, visible, hide_active)
}

/// `uint64_t obs_scene_audio_window_earliest(uint64_t timestamp, uint64_t
/// source_ts)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_scene_audio_window_earliest(timestamp: u64, source_ts: u64) -> u64 {
    core::earliest(timestamp, source_ts)
}

/// `bool obs_scene_audio_window_silent(uint64_t timestamp)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_scene_audio_window_silent(timestamp: u64) -> bool {
    core::silent(timestamp)
}

/// `struct obs_scene_audio_window obs_scene_audio_window_frames(bool
/// pending, uint64_t source_ts, uint64_t timestamp, size_t sample_rate)`:
/// `mix` false with `pos` and `count` 0 when the item is skipped.
#[unsafe(no_mangle)]
pub extern "C" fn obs_scene_audio_window_frames(
    pending: bool,
    source_ts: u64,
    timestamp: u64,
    sample_rate: usize,
) -> obs_scene_audio_window {
    match core::frames(pending, source_ts, timestamp, rate(sample_rate)) {
        Some(f) => obs_scene_audio_window {
            mix: true,
            pos: f.pos,
            count: f.count,
        },
        None => obs_scene_audio_window {
            mix: false,
            pos: 0,
            count: 0,
        },
    }
}

/// `bool obs_scene_audio_window_mixes(bool apply_buf, bool visible, bool
/// hide_active)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_scene_audio_window_mixes(
    apply_buf: bool,
    visible: bool,
    hide_active: bool,
) -> bool {
    core::mixes(apply_buf, visible, hide_active)
}

/// `bool obs_scene_audio_window_adds(bool audio_is_duplicated)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_scene_audio_window_adds(audio_is_duplicated: bool) -> bool {
    core::adds(audio_is_duplicated)
}
