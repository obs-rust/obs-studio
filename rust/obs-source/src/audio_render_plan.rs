//! Safe core of `libobs/obs-source-audio-render.c.h`: the audio render mix
//! plan of `obs_source_audio_render()`, `custom_audio_render()` and
//! `process_audio_source_tick()`.
//!
//! Each audio tick, the mix asks every audio source for its six mixes
//! through `obs_source_audio_render()` in `obs-source.c.h`, and these are
//! the decisions it takes on the way:
//! - [`route`]: pending without an output buffer, or with an `audio_render`
//!   callback but no data yet (the audio thread can reach a source still
//!   being created, 432017b2c); the callback with data; else the tick;
//! - around the callback (`custom_audio_render()`):
//!   [`custom_clear_before`] (a mix in both masks is cleared before it),
//!   [`custom_ts`] and [`custom_pending`] (its timestamp when it succeeds,
//!   else 0 and pending), [`custom_unmixed`] (no timestamp or no tick
//!   mixers: nothing after it), [`custom_in_tick`] and
//!   [`custom_clear_after`] (a mix of the tick the source is not on is
//!   cleared after it);
//! - on the buffered input (`process_audio_source_tick()`): [`tick_unset`]
//!   and [`tick_short`] (pending with no timestamp or less input than the
//!   tick), [`tick_stop`] and [`tick_copy`] (mixes 1 to 5 copy mix 0 when
//!   in both masks, else are cleared; a submix source does mix 1 only, from
//!   bit 0 of its mixers), [`tick_volume`] and [`tick_clear_main`] (a
//!   submix source stops there; any other clears mix 0 unless it is in both
//!   masks, then scales by its volume).
//!
//! The plan is one function per decision, not one call per tick, because
//! `obs-source.c.h` must read each field where it did: `audio_mixers` is set
//! by other threads without a lock and read once per mix, before and after
//! the plugin's callback; `context.data` only behind the callback check; the
//! input size only with a timestamp; `audio_ts` only after a successful
//! callback. `obs-source.c.h` keeps those reads, `audio_buf_mutex`,
//! `deque_peek_front()`, every memset and memcpy, the plugin callbacks,
//! `audio_submix()` and `apply_audio_volume()`. Nothing here keeps state,
//! takes a lock or calls libobs.
//!
//! Characterized, not endorsed (`test_source_audio_render_steps`), and
//! kept: tick mixers with no bit below [`MAX_AUDIO_MIXES`] (0x40 and up)
//! are not "no mixers", so a custom render's clears look at every mix and
//! its volume runs; a submix source keeps the peeked input in mix 0 even
//! when it is on no mix, and fills mix 1 whatever the tick's mixers.
//!
//! Intentional difference: a `mix` of 32 or more names no mix (the C's
//! `1 << mix` on an `int` is undefined from 31 on; libobs passes a mix
//! below [`MAX_AUDIO_MIXES`]).

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::as_conversions,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
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

/// The mix's bit, the C's `1 << mix`; 0 (no mix) from 32 on.
fn bit(mix: usize) -> u32 {
    u32::try_from(mix)
        .ok()
        .and_then(|shift| 1_u32.checked_shl(shift))
        .unwrap_or(0)
}

/// `obs_source_audio_render_route()`: [`Route::Pending`] without
/// `has_buffer`; with `custom`, [`Route::Custom`] when `has_data`, else
/// [`Route::Pending`]; else [`Route::Tick`].
pub const fn route(has_buffer: bool, custom: bool, has_data: bool) -> Route {
    if !has_buffer {
        return Route::Pending;
    }
    if custom {
        if !has_data {
            return Route::Pending;
        }
        return Route::Custom;
    }
    Route::Tick
}

/// `obs_source_audio_render_custom_clear_before()`: the mix is in both
/// `audio_mixers` and `mixers`.
pub fn custom_clear_before(audio_mixers: u32, mixers: u32, mix: usize) -> bool {
    audio_mixers & mixers & bit(mix) != 0
}

/// `obs_source_audio_render_custom_ts()`: `ts` when the callback
/// succeeded, else 0 (`ts` is not looked at).
pub const fn custom_ts(success: bool, ts: u64) -> u64 {
    if success { ts } else { 0 }
}

/// `obs_source_audio_render_custom_pending()`: pending when the callback
/// failed.
pub const fn custom_pending(success: bool) -> bool {
    !success
}

/// `obs_source_audio_render_custom_unmixed()`: no timestamp or no tick
/// mixers, so nothing is cleared or scaled after the callback.
pub const fn custom_unmixed(audio_ts: u64, mixers: u32) -> bool {
    audio_ts == 0 || mixers == 0
}

/// `obs_source_audio_render_custom_in_tick()`: the mix is in the tick's
/// `mixers`.
pub fn custom_in_tick(mixers: u32, mix: usize) -> bool {
    mixers & bit(mix) != 0
}

/// `obs_source_audio_render_custom_clear_after()`: the source is not on the
/// mix.
pub fn custom_clear_after(audio_mixers: u32, mix: usize) -> bool {
    audio_mixers & bit(mix) == 0
}

/// `obs_source_audio_render_tick_unset()`: no input timestamp yet.
pub const fn tick_unset(audio_ts: u64) -> bool {
    audio_ts == 0
}

/// `obs_source_audio_render_tick_short()`: less buffered input than the
/// tick reads.
pub const fn tick_short(input_size: usize, size: usize) -> bool {
    input_size < size
}

/// `obs_source_audio_render_tick_stop()`: a submix source stops after
/// mix 1.
pub const fn tick_stop(submix: bool, mix: usize) -> bool {
    submix && mix > 1
}

/// `obs_source_audio_render_tick_copy()`: the mix copies mix 0 when it is
/// in both masks (else it is cleared); for a submix source, when
/// `audio_mixers` has bit 0, whatever `mixers` and `mix`.
pub fn tick_copy(submix: bool, audio_mixers: u32, mixers: u32, mix: usize) -> bool {
    let (mixers, mix_bit) = if submix { (1, 1) } else { (mixers, bit(mix)) };
    audio_mixers & mix_bit != 0 && mixers & mix_bit != 0
}

/// `obs_source_audio_render_tick_volume()`: anything but a submix source
/// goes on to mix 0 and the volume.
pub const fn tick_volume(submix: bool) -> bool {
    !submix
}

/// `obs_source_audio_render_tick_clear_main()`: mix 0 is cleared unless it
/// is in both masks.
pub const fn tick_clear_main(audio_mixers: u32, mixers: u32) -> bool {
    audio_mixers & 1 == 0 || mixers & 1 == 0
}
