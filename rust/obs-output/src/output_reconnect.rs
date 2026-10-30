//! Safe core of `libobs/obs-output-reconnect.c.h`: the reconnect policy of
//! `obs-output.c`.
//!
//! `obs_output_signal_stop()` runs it each time an output's plugin reports a
//! stop: a stream that loses its server reconnects with a growing wait
//! until it gives up. `obs-output.c` keeps
//! `obs_output_set_reconnect_settings()` and its stores, the exponent's
//! random jitter at create, every atomic on `reconnecting` and the delay
//! flags (each loaded where it always was, in its `&&` order), the reconnect
//! stop event's waits, tests and reset, `reconnect_thread()` with
//! `pthread_create()`, `pthread_detach()` and `obs_output_actual_start()`,
//! the reconnect callback and its NULL test, the end of the capture, the
//! retry count's reset and increment, the stores, the signals and the logs.
//! It passes the values here. These keep no state, take no lock and call
//! nothing.
//!
//! - `output_reconnect()`: [`first_delay`] (the first attempt's wait, from
//!   the delay setting), [`give_up`] (the output stops as disconnected),
//!   then [`grows`] and [`next_delay`] (a later attempt waits longer, up to
//!   [`MAX_MSEC`]);
//! - `can_reconnect()`: [`active`] (whether reconnecting is on, and so
//!   whether the reconnect callback is asked), then [`can`] (whether the
//!   stop reconnects).
//!
//! The settings come from the frontend, or from any plugin or script
//! through `obs_output_set_reconnect_settings()`, and nothing here can
//! panic on any input.
//!
//! Intentional differences:
//! - the wait is computed in 64 bits and clamped to 0 to [`MAX_MSEC`] at
//!   every step. The C multiplies the delay setting by 1000 in `int`, so
//!   above 2147483 seconds it overflows (undefined) and a negative setting
//!   wraps to about 49 days; it clamps only from the second attempt on, so
//!   the first wait is whatever the setting gave; and the second attempt
//!   converts a product above `UINT32_MAX` to `uint32_t` (undefined) before
//!   it clamps;
//! - a retry count of 0 or less is off. The C treats any count but 0 as on,
//!   so a negative count runs the reconnect callback, ends the capture on
//!   the reconnect path and then gives up at once.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

/// `OBS_OUTPUT_SUCCESS`.
pub const OBS_OUTPUT_SUCCESS: i32 = 0;
/// `OBS_OUTPUT_DISCONNECTED`.
pub const OBS_OUTPUT_DISCONNECTED: i32 = -5;

/// The longest wait between two attempts, in milliseconds (15 minutes;
/// `RECONNECT_RETRY_MAX_MSEC`).
pub const MAX_MSEC: u32 = 15 * 60 * 1000;

/// `output_reconnect()`, on the first attempt: the wait before it, in
/// milliseconds, from the delay setting `retry_sec` (`reconnect_retry_sec`,
/// in seconds), clamped to 0 to [`MAX_MSEC`].
#[must_use]
pub fn first_delay(retry_sec: i32) -> u32 {
    let msec = i64::from(retry_sec).saturating_mul(1000);
    u32::try_from(msec.clamp(0, i64::from(MAX_MSEC))).unwrap_or(MAX_MSEC)
}

/// `output_reconnect()`: `retries` attempts were made (`reconnect_retries`,
/// 0 on the first) out of `retry_max` (`reconnect_retry_max`), so the output
/// gives up and stops as disconnected.
#[must_use]
pub fn give_up(retries: i32, retry_max: i32) -> bool {
    retries >= retry_max
}

/// `output_reconnect()`, for an attempt that goes ahead: `retries` attempts
/// were made before it, so this one waits longer than the last
/// ([`next_delay`]); the first keeps its [`first_delay`].
#[must_use]
pub fn grows(retries: i32) -> bool {
    retries != 0
}

/// `output_reconnect()`: the wait before a later attempt, from the last one
/// (`cur_msec`, `reconnect_retry_cur_msec`) times the exponent (`exp`,
/// `reconnect_retry_exp`) in `f32` as the C computes it, truncated, at most
/// [`MAX_MSEC`]. A product that is negative gives 0, and one that is not a
/// number gives [`MAX_MSEC`].
#[must_use]
#[allow(
    clippy::as_conversions,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
pub fn next_delay(cur_msec: u32, exp: f32) -> u32 {
    // `(float)cur_msec * exp`, clamped before the conversion: `min` keeps
    // the limit for a NaN, and `as` truncates toward 0 and saturates a
    // negative to 0.
    ((cur_msec as f32) * exp).min(MAX_MSEC as f32) as u32
}

/// `can_reconnect()`: reconnecting is on for a positive `retry_max`
/// (`reconnect_retry_max`), so the reconnect callback, if any, is asked
/// first, and a disconnect reconnects.
#[must_use]
pub fn active(retry_max: i32) -> bool {
    retry_max > 0
}

/// `can_reconnect()`, once the callback, if asked, agreed: a stop with
/// `code` reconnects. A failed attempt (`reconnecting`) retries on any code
/// but [`OBS_OUTPUT_SUCCESS`]; otherwise only [`OBS_OUTPUT_DISCONNECTED`]
/// reconnects, and only while reconnecting is on (`active`, from
/// [`active`]).
#[must_use]
pub fn can(reconnecting: bool, active: bool, code: i32) -> bool {
    (reconnecting && code != OBS_OUTPUT_SUCCESS) || (active && code == OBS_OUTPUT_DISCONNECTED)
}
