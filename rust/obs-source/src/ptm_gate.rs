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

/// `OBS_SOURCE_PTM_GATE_NS_PER_MS`: the delays are milliseconds, the clock
/// and the stop times nanoseconds.
pub const NS_PER_MS: u64 = 1_000_000;

/// `obs_source_ptm_gate_stop_time()`: `os_time + delay_ms * NS_PER_MS`, or
/// `u64::MAX` where the product or the sum would wrap.
pub fn stop_time(os_time: u64, delay_ms: u64) -> u64 {
    let _ = (os_time, delay_ms);
    todo!()
}

/// `obs_source_ptm_gate_held()`: whether a key let go still holds at
/// `os_time`.
pub fn held(os_time: u64, stop_time: u64) -> bool {
    let _ = (os_time, stop_time);
    todo!()
}

/// `obs_source_ptm_gate_mutes()`: whether push-to-mute mutes, from
/// `push_to_mute_enabled` and whether its key is pressed or held.
pub fn mutes(enabled: bool, active: bool) -> bool {
    let _ = (enabled, active);
    todo!()
}

/// `obs_source_ptm_gate_talk_mutes()`: whether push-to-talk mutes, from
/// `push_to_talk_enabled` and whether its key is pressed or held.
pub fn talk_mutes(enabled: bool, active: bool) -> bool {
    let _ = (enabled, active);
    todo!()
}
