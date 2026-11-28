//! Safe core of `libobs/obs-source-muted-volume.c.h`: the two mute steps of
//! a source around the push-to-mute and push-to-talk gate.
//!
//! - [`source_muted`] (`source_muted()`, the audio output path's mute check
//!   for every packet, at `os_gettime_ns()`): the user's flags,
//!   `user_push_to_mute_pressed`, `user_push_to_talk_pressed` and
//!   `user_muted` (an atomic load).
//! - [`get_source_volume`] (`get_source_volume()`, the volume
//!   `apply_audio_volume()` and `apply_audio_actions()` apply on the audio
//!   mix thread, at the source's audio timestamp): the mix thread's flags,
//!   `push_to_mute_pressed`, `push_to_talk_pressed` and `muted`, then
//!   [`crate::volume_apply::snap`] of the result and `volume`, read once.
//!
//! Both first write a key's stop time ([`crate::ptm_gate::stop_time`] of
//! the moment and the key's delay) when the key is enabled and its flag
//! pressed, push-to-mute first, and only then read the flags and stop times
//! back: a key is active while pressed or held ([`crate::ptm_gate::held`]).
//! The source is muted when it is not enabled, else when the mute flag is
//! set, else when push-to-mute mutes ([`crate::ptm_gate::mutes`]), else when
//! push-to-talk does ([`crate::ptm_gate::talk_mutes`]); each read happens
//! only where the C's `||` reaches it.
//!
//! Every field stays C, reached through the [`Host`] trait one read or
//! write at a time.
//!
//! No intentional differences from C.

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

use crate::{ptm_gate, volume_apply};

/// One of the two keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// Push-to-mute: `push_to_mute_*`.
    Mute,
    /// Push-to-talk: `push_to_talk_*`.
    Talk,
}

/// Whose flags a step reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flags {
    /// `source_muted()`: `user_push_to_<key>_pressed` and `user_muted`.
    User,
    /// `get_source_volume()`: `push_to_<key>_pressed` and `muted`.
    Mix,
}

/// The source's fields the steps read and write, in the C's terms.
pub trait Host {
    /// A live, non-null source.
    type Source: Copy;
    /// `source->push_to_<key>_enabled`.
    fn key_enabled(&self, source: Self::Source, key: Key) -> bool;
    /// `source->user_push_to_<key>_pressed` or `source->push_to_<key>_pressed`.
    fn pressed(&self, source: Self::Source, flags: Flags, key: Key) -> bool;
    /// `source->push_to_<key>_delay`.
    fn delay(&self, source: Self::Source, key: Key) -> u64;
    /// `source->push_to_<key>_stop_time`.
    fn stop_time(&self, source: Self::Source, key: Key) -> u64;
    /// `source->push_to_<key>_stop_time = stop_time`.
    fn set_stop_time(&self, source: Self::Source, key: Key, stop_time: u64);
    /// `source->enabled`.
    fn enabled(&self, source: Self::Source) -> bool;
    /// `os_atomic_load_bool(&source->user_muted)` or `source->muted`.
    fn muted(&self, source: Self::Source, flags: Flags) -> bool;
    /// `source->volume`.
    fn volume(&self, source: Self::Source) -> f32;
}

/// The gate both steps share, on `flags`: the stop-time writes, then the
/// reads, then whether the source is muted.
fn gate<H: Host>(host: &H, source: H::Source, flags: Flags, os_time: u64) -> bool {
    for key in [Key::Mute, Key::Talk] {
        if host.key_enabled(source, key) && host.pressed(source, flags, key) {
            let delay = host.delay(source, key);
            host.set_stop_time(source, key, ptm_gate::stop_time(os_time, delay));
        }
    }

    let mute_active = host.pressed(source, flags, Key::Mute)
        || ptm_gate::held(os_time, host.stop_time(source, Key::Mute));
    let talk_active = host.pressed(source, flags, Key::Talk)
        || ptm_gate::held(os_time, host.stop_time(source, Key::Talk));

    !host.enabled(source)
        || host.muted(source, flags)
        || ptm_gate::mutes(host.key_enabled(source, Key::Mute), mute_active)
        || ptm_gate::talk_mutes(host.key_enabled(source, Key::Talk), talk_active)
}

/// `source_muted(source, os_time)`.
pub fn source_muted<H: Host>(host: &H, source: H::Source, os_time: u64) -> bool {
    gate(host, source, Flags::User, os_time)
}

/// `get_source_volume(source, os_time)`.
pub fn get_source_volume<H: Host>(host: &H, source: H::Source, os_time: u64) -> f32 {
    let muted = gate(host, source, Flags::Mix, os_time);
    // Read once, muted or not: only the mix thread writes it.
    volume_apply::snap(muted, host.volume(source))
}
