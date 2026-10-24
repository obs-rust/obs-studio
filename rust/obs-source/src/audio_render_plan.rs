//! Safe core of `libobs/obs-source-audio-render.c.h`: the audio render mix
//! plan of `obs_source_audio_render()`, `custom_audio_render()` and
//! `process_audio_source_tick()`.
//!
//! Not ported yet: every function is `todo!()`, so the Tier 1 tests fail.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::as_conversions,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable
)]

/// `MAX_AUDIO_MIXES`: the mixes a tick fills.
pub const MAX_AUDIO_MIXES: usize = 6;

/// `enum obs_source_audio_render_route`: which way
/// `obs_source_audio_render()` takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Route {
    /// `OBS_SOURCE_AUDIO_RENDER_PENDING`: no output buffer, or a custom
    /// render without data.
    Pending = 0,
    /// `OBS_SOURCE_AUDIO_RENDER_CUSTOM`: the source's `audio_render`
    /// callback.
    Custom = 1,
    /// `OBS_SOURCE_AUDIO_RENDER_TICK`: the buffered input (`audio_submix()`
    /// first when there is an `audio_mix` callback).
    Tick = 2,
}

impl From<Route> for u32 {
    /// The C enum's value.
    fn from(route: Route) -> Self {
        match route {
            Route::Pending => 0,
            Route::Custom => 1,
            Route::Tick => 2,
        }
    }
}

/// `obs_source_audio_render_route()`.
pub fn route(has_buffer: bool, custom: bool, has_data: bool) -> Route {
    let _ = (has_buffer, custom, has_data);
    todo!("obs_source_audio_render_route")
}

/// `obs_source_audio_render_custom_clear_before()`.
pub fn custom_clear_before(audio_mixers: u32, mixers: u32, mix: usize) -> bool {
    let _ = (audio_mixers, mixers, mix);
    todo!("obs_source_audio_render_custom_clear_before")
}

/// `obs_source_audio_render_custom_ts()`.
pub fn custom_ts(success: bool, ts: u64) -> u64 {
    let _ = (success, ts);
    todo!("obs_source_audio_render_custom_ts")
}

/// `obs_source_audio_render_custom_pending()`.
pub fn custom_pending(success: bool) -> bool {
    let _ = success;
    todo!("obs_source_audio_render_custom_pending")
}

/// `obs_source_audio_render_custom_unmixed()`.
pub fn custom_unmixed(audio_ts: u64, mixers: u32) -> bool {
    let _ = (audio_ts, mixers);
    todo!("obs_source_audio_render_custom_unmixed")
}

/// `obs_source_audio_render_custom_in_tick()`.
pub fn custom_in_tick(mixers: u32, mix: usize) -> bool {
    let _ = (mixers, mix);
    todo!("obs_source_audio_render_custom_in_tick")
}

/// `obs_source_audio_render_custom_clear_after()`.
pub fn custom_clear_after(audio_mixers: u32, mix: usize) -> bool {
    let _ = (audio_mixers, mix);
    todo!("obs_source_audio_render_custom_clear_after")
}

/// `obs_source_audio_render_tick_unset()`.
pub fn tick_unset(audio_ts: u64) -> bool {
    let _ = audio_ts;
    todo!("obs_source_audio_render_tick_unset")
}

/// `obs_source_audio_render_tick_short()`.
pub fn tick_short(input_size: usize, size: usize) -> bool {
    let _ = (input_size, size);
    todo!("obs_source_audio_render_tick_short")
}

/// `obs_source_audio_render_tick_stop()`.
pub fn tick_stop(submix: bool, mix: usize) -> bool {
    let _ = (submix, mix);
    todo!("obs_source_audio_render_tick_stop")
}

/// `obs_source_audio_render_tick_copy()`.
pub fn tick_copy(submix: bool, audio_mixers: u32, mixers: u32, mix: usize) -> bool {
    let _ = (submix, audio_mixers, mixers, mix);
    todo!("obs_source_audio_render_tick_copy")
}

/// `obs_source_audio_render_tick_volume()`.
pub fn tick_volume(submix: bool) -> bool {
    let _ = submix;
    todo!("obs_source_audio_render_tick_volume")
}

/// `obs_source_audio_render_tick_clear_main()`.
pub fn tick_clear_main(audio_mixers: u32, mixers: u32) -> bool {
    let _ = (audio_mixers, mixers);
    todo!("obs_source_audio_render_tick_clear_main")
}
