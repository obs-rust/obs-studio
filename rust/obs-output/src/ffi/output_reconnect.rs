//! C ABI for `libobs/obs-output-reconnect.c.h`, the reconnect policy of
//! `obs-output.c`, declared in `libobs/obs-output-reconnect.h` without
//! `EXPORT`.
//!
//! These only decide. `obs-output.c.h` (still C) keeps
//! `obs_output_set_reconnect_settings()`, the reconnect thread, the stop
//! event, the reconnect callback, every atomic on `reconnecting` and the
//! delay flags (each loaded where it always was, in its `&&` order), the
//! retry count and the wait's stores, the capture's start and end, the
//! signals and the logs, and passes plain values here. So `struct
//! obs_output` is never mirrored. These shims take no lock, call nothing in
//! libobs and dereference no pointer. All six stay hidden
//! (`rust-hidden.txt`).
//!
//! Intentional differences: the core's (every wait clamped to 0 to 15
//! minutes, a retry count of 0 or less off).

use std::ffi::c_int;

use crate::output_reconnect as core;

/// `uint32_t obs_output_reconnect_first_delay(int retry_sec)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_reconnect_first_delay(retry_sec: c_int) -> u32 {
    core::first_delay(retry_sec)
}

/// `bool obs_output_reconnect_give_up(int retries, int retry_max)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_reconnect_give_up(retries: c_int, retry_max: c_int) -> bool {
    core::give_up(retries, retry_max)
}

/// `bool obs_output_reconnect_grows(int retries)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_reconnect_grows(retries: c_int) -> bool {
    core::grows(retries)
}

/// `uint32_t obs_output_reconnect_next_delay(uint32_t cur_msec, float exp)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_reconnect_next_delay(cur_msec: u32, exp: f32) -> u32 {
    core::next_delay(cur_msec, exp)
}

/// `bool obs_output_reconnect_active(int retry_max)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_reconnect_active(retry_max: c_int) -> bool {
    core::active(retry_max)
}

/// `bool obs_output_reconnect_can(bool reconnecting, bool active, int
/// code)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_reconnect_can(reconnecting: bool, active: bool, code: c_int) -> bool {
    core::can(reconnecting, active, code)
}
