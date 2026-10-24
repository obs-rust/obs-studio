//! C ABI for `libobs/obs-source-ptm-gate.c.h`, the push-to-mute and
//! push-to-talk gate of `source_muted()`, declared in
//! `libobs/obs-source-ptm-gate.h` without `EXPORT`.
//!
//! `source_muted()` in `obs-source.c.h` (still C) reads the source's fields
//! itself, in the C's order, passes the values to these shims and stores the
//! stop times it gets back; `get_source_volume()` stores its stop times
//! through [`obs_source_ptm_gate_stop_time`] too. So `struct obs_source` is
//! never mirrored, no pointer crosses, and the shims take no lock, call
//! nothing in libobs and touch no memory. They stay hidden
//! (`rust-hidden.txt`). There is no intentional difference from the C.

use crate::ptm_gate as core;

/// `uint64_t obs_source_ptm_gate_stop_time(uint64_t os_time, uint64_t
/// delay_ms)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_ptm_gate_stop_time(os_time: u64, delay_ms: u64) -> u64 {
    core::stop_time(os_time, delay_ms)
}

/// `bool obs_source_ptm_gate_held(uint64_t os_time, uint64_t stop_time)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_ptm_gate_held(os_time: u64, stop_time: u64) -> bool {
    core::held(os_time, stop_time)
}

/// `bool obs_source_ptm_gate_mutes(bool enabled, bool active)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_ptm_gate_mutes(enabled: bool, active: bool) -> bool {
    core::mutes(enabled, active)
}

/// `bool obs_source_ptm_gate_talk_mutes(bool enabled, bool active)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_ptm_gate_talk_mutes(enabled: bool, active: bool) -> bool {
    core::talk_mutes(enabled, active)
}
