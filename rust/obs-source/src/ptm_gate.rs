//! Safe core of `libobs/obs-source-ptm-gate.c.h`: the push-to-mute and
//! push-to-talk gate of `source_muted()`.
//!
//! Every audio packet a source outputs goes to its capture callbacks with a
//! muted flag. `source_muted()` in `obs-source.c.h` works it out from the
//! source's `enabled`, `user_muted` and the two gates, and these are the
//! decisions it asks for:
//! - [`stop_time`]: while a gate's key is held, each packet stores the
//!   moment the key stops holding once it is let go, `os_time` plus the
//!   delay in milliseconds, saturated at `u64::MAX` where the product or the
//!   sum would wrap (#2051). `get_source_volume()` stores the same field
//!   through it;
//! - [`held`]: a key let go still holds strictly before its stop time;
//! - [`mutes`] and [`talk_mutes`]: push-to-mute mutes while enabled and
//!   active (pressed or held), push-to-talk while enabled and not active.
//!
//! `obs-source.c.h` keeps every read and write of the source's fields, in
//! the same order, and passes the values it read: the pressed flags are
//! written by the hotkey thread without atomics and the stop times by
//! `get_source_volume()` on the audio mix thread too, so nothing here sees
//! `struct obs_source`. Nothing here keeps state, takes a lock or calls
//! libobs.
//!
//! Characterized, not endorsed (`test_source_ptm_gate_steps`), and kept: a
//! saturated stop time does not hold at `os_time` `u64::MAX` itself; while
//! held, each packet stores its own time plus the delay, so an earlier
//! packet moves the stop time back.

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

/// `OBS_SOURCE_PTM_GATE_NS_PER_MS`: the delays are milliseconds, the clock
/// and the stop times nanoseconds.
pub const NS_PER_MS: u64 = 1_000_000;

/// `obs_source_ptm_gate_stop_time()`: `os_time + delay_ms * NS_PER_MS`, or
/// `u64::MAX` where the product or the sum would wrap.
///
/// The C checks `delay_ms > UINT64_MAX / 1000000` and then
/// `delay_ns > UINT64_MAX - os_time`; the two saturating operations are the
/// same: a saturated product is `u64::MAX`, and any clock plus that
/// saturates too.
pub const fn stop_time(os_time: u64, delay_ms: u64) -> u64 {
    os_time.saturating_add(delay_ms.saturating_mul(NS_PER_MS))
}

/// `obs_source_ptm_gate_held()`: whether a key let go still holds at
/// `os_time`, strictly before its stop time.
pub const fn held(os_time: u64, stop_time: u64) -> bool {
    os_time < stop_time
}

/// `obs_source_ptm_gate_mutes()`: whether push-to-mute mutes, from
/// `push_to_mute_enabled` and whether its key is pressed or held.
pub const fn mutes(enabled: bool, active: bool) -> bool {
    enabled && active
}

/// `obs_source_ptm_gate_talk_mutes()`: whether push-to-talk mutes, from
/// `push_to_talk_enabled` and whether its key is pressed or held.
pub const fn talk_mutes(enabled: bool, active: bool) -> bool {
    enabled && !active
}
