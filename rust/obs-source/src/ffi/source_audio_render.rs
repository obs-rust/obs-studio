//! C ABI for `libobs/obs-source-audio-render.c.h`, the audio render mix plan
//! of `obs_source_audio_render()`, `custom_audio_render()` and
//! `process_audio_source_tick()`, declared in
//! `libobs/obs-source-audio-render.h` without `EXPORT`.
//!
//! The three callers in `obs-source.c.h` (still C) read the source's fields
//! themselves, in the C's order and under its conditions, pass the values
//! to these shims and act on the answers: `audio_buf_mutex`,
//! `deque_peek_front()`, every memset and memcpy, the plugin callbacks,
//! `audio_submix()` and `apply_audio_volume()` stay C. So `struct
//! obs_source` is never mirrored, no pointer crosses, and the shims take no
//! lock, call nothing in libobs and touch no memory. They stay hidden
//! (`rust-hidden.txt`).
//!
//! `obs_source_audio_render_custom_ts()` receives the callback's `ts` by
//! value also when the callback failed and may not have written it (an
//! indeterminate `uint64_t` whose address was taken, so not undefined in
//! C); any `u64` is valid here, and it is not looked at.
//!
//! Intentional difference: a `mix` of 32 or more names no mix, where the C's
//! `1 << mix` on an `int` is undefined from 31 on (libobs passes
//! `mix < MAX_AUDIO_MIXES`).

use crate::audio_render_plan::{self as core, Route};

/// `enum obs_source_audio_render_route`, as the C's enum is returned (an
/// `int` or `unsigned int`; every value fits both).
#[allow(non_camel_case_types)]
pub type obs_source_audio_render_route_t = u32;

/// `OBS_SOURCE_AUDIO_RENDER_PENDING`.
pub const OBS_SOURCE_AUDIO_RENDER_PENDING: obs_source_audio_render_route_t = 0;
/// `OBS_SOURCE_AUDIO_RENDER_CUSTOM`.
pub const OBS_SOURCE_AUDIO_RENDER_CUSTOM: obs_source_audio_render_route_t = 1;
/// `OBS_SOURCE_AUDIO_RENDER_TICK`.
pub const OBS_SOURCE_AUDIO_RENDER_TICK: obs_source_audio_render_route_t = 2;

/// `enum obs_source_audio_render_route obs_source_audio_render_route(bool
/// has_buffer, bool custom, bool has_data)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_audio_render_route(
    has_buffer: bool,
    custom: bool,
    has_data: bool,
) -> obs_source_audio_render_route_t {
    match core::route(has_buffer, custom, has_data) {
        Route::Pending => OBS_SOURCE_AUDIO_RENDER_PENDING,
        Route::Custom => OBS_SOURCE_AUDIO_RENDER_CUSTOM,
        Route::Tick => OBS_SOURCE_AUDIO_RENDER_TICK,
    }
}

/// `bool obs_source_audio_render_custom_clear_before(uint32_t audio_mixers,
/// uint32_t mixers, size_t mix)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_audio_render_custom_clear_before(
    audio_mixers: u32,
    mixers: u32,
    mix: usize,
) -> bool {
    core::custom_clear_before(audio_mixers, mixers, mix)
}

/// `uint64_t obs_source_audio_render_custom_ts(bool success, uint64_t ts)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_audio_render_custom_ts(success: bool, ts: u64) -> u64 {
    core::custom_ts(success, ts)
}

/// `bool obs_source_audio_render_custom_pending(bool success)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_audio_render_custom_pending(success: bool) -> bool {
    core::custom_pending(success)
}

/// `bool obs_source_audio_render_custom_unmixed(uint64_t audio_ts, uint32_t
/// mixers)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_audio_render_custom_unmixed(audio_ts: u64, mixers: u32) -> bool {
    core::custom_unmixed(audio_ts, mixers)
}

/// `bool obs_source_audio_render_custom_in_tick(uint32_t mixers, size_t
/// mix)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_audio_render_custom_in_tick(mixers: u32, mix: usize) -> bool {
    core::custom_in_tick(mixers, mix)
}

/// `bool obs_source_audio_render_custom_clear_after(uint32_t audio_mixers,
/// size_t mix)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_audio_render_custom_clear_after(
    audio_mixers: u32,
    mix: usize,
) -> bool {
    core::custom_clear_after(audio_mixers, mix)
}

/// `bool obs_source_audio_render_tick_unset(uint64_t audio_ts)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_audio_render_tick_unset(audio_ts: u64) -> bool {
    core::tick_unset(audio_ts)
}

/// `bool obs_source_audio_render_tick_short(size_t input_size, size_t
/// size)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_audio_render_tick_short(input_size: usize, size: usize) -> bool {
    core::tick_short(input_size, size)
}

/// `bool obs_source_audio_render_tick_stop(bool submix, size_t mix)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_audio_render_tick_stop(submix: bool, mix: usize) -> bool {
    core::tick_stop(submix, mix)
}

/// `bool obs_source_audio_render_tick_copy(bool submix, uint32_t
/// audio_mixers, uint32_t mixers, size_t mix)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_audio_render_tick_copy(
    submix: bool,
    audio_mixers: u32,
    mixers: u32,
    mix: usize,
) -> bool {
    core::tick_copy(submix, audio_mixers, mixers, mix)
}

/// `bool obs_source_audio_render_tick_volume(bool submix)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_audio_render_tick_volume(submix: bool) -> bool {
    core::tick_volume(submix)
}

/// `bool obs_source_audio_render_tick_clear_main(uint32_t audio_mixers,
/// uint32_t mixers)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_audio_render_tick_clear_main(audio_mixers: u32, mixers: u32) -> bool {
    core::tick_clear_main(audio_mixers, mixers)
}
