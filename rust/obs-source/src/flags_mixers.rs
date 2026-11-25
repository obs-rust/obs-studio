//! `obs_source_set_flags()` and `obs_source_set_audio_mixers()`: the two
//! public setters the advanced audio properties, source loading and
//! obs-websocket use. Each writes one field of the source and emits one
//! signal on the source's own handler; the same-value, audio and truncation
//! rules are [`crate::monitoring`]'s.

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

use core::ffi::CStr;

/// The public function named in `set_flags`' NULL diagnostic.
pub const SET_FLAGS_FUNCTION: &CStr = c"obs_source_set_flags";
/// The public function named in `set_audio_mixers`' NULL diagnostic.
pub const SET_AUDIO_MIXERS_FUNCTION: &CStr = c"obs_source_set_audio_mixers";

/// What the two setters read, write and emit on a live source, each call one
/// access, in the order they ask.
pub trait Source {
    /// `flags`, read now.
    fn flags(&self) -> u32;
    /// Writes `flags`.
    fn store_flags(&self, flags: u32);
    /// Emits `update_flags` with the source and its `flags` as they are now.
    fn signal_update_flags(&self);
    /// `owns_info_id`, read now.
    fn owns_info_id(&self) -> bool;
    /// `info.output_flags`, read now.
    fn output_flags(&self) -> u32;
    /// `audio_mixers`, read now.
    fn audio_mixers(&self) -> u32;
    /// Emits `audio_mixers` with the source and `mixers`, and answers the
    /// calldata's `mixers` after the handlers ran (`calldata_int()`: 0 when
    /// a handler left it with another size).
    fn signal_audio_mixers(&self, mixers: i64) -> i64;
    /// Writes `audio_mixers`.
    fn store_audio_mixers(&self, mixers: u32);
}

/// `None` is a NULL source, logged as [`SET_FLAGS_FUNCTION`]. Otherwise an
/// unchanged value does nothing more; a change is stored, then signalled.
pub fn set_flags(source: Option<&impl Source>, flags: u32, log_null: impl FnOnce(&'static CStr)) {
    let Some(source) = source else {
        log_null(SET_FLAGS_FUNCTION);
        return;
    };
    if crate::monitoring::changes(source.flags(), flags) {
        source.store_flags(flags);
        source.signal_update_flags();
    }
}

/// `None` is a NULL source, logged as [`SET_AUDIO_MIXERS_FUNCTION`].
/// Otherwise a source that does not own its info id and has no audio is left
/// alone, and so is an unchanged value. A change is signalled first; the
/// handlers may rewrite it, and the low 32 bits of what they leave are
/// stored.
pub fn set_audio_mixers(
    source: Option<&impl Source>,
    mixers: u32,
    log_null: impl FnOnce(&'static CStr),
) {
    let Some(source) = source else {
        log_null(SET_AUDIO_MIXERS_FUNCTION);
        return;
    };
    if !source.owns_info_id() && !crate::monitoring::has_audio(source.output_flags()) {
        return;
    }
    if !crate::monitoring::changes(source.audio_mixers(), mixers) {
        return;
    }
    let left = source.signal_audio_mixers(i64::from(mixers));
    source.store_audio_mixers(crate::monitoring::mixers_store(left));
}
