//! `oracle_*` build of `libobs/obs-output-reconnect.c.h`
//! (oracle/output_reconnect.c): the output reconnect policy, renamed.

use std::ffi::c_int;

unsafe extern "C" {
    /// Undefined for a `retry_sec` whose `* 1000` overflows `int` (above
    /// 2147483 or below -2147483): never call it with those.
    pub fn oracle_obs_output_reconnect_first_delay(retry_sec: c_int) -> u32;

    pub fn oracle_obs_output_reconnect_give_up(retries: c_int, retry_max: c_int) -> bool;

    pub fn oracle_obs_output_reconnect_grows(retries: c_int) -> bool;

    /// Undefined for a product `(float)cur_msec * exp` that is negative, not
    /// a number or above `UINT32_MAX`: never call it with those.
    pub fn oracle_obs_output_reconnect_next_delay(cur_msec: u32, exp: f32) -> u32;

    pub fn oracle_obs_output_reconnect_active(retry_max: c_int) -> bool;

    pub fn oracle_obs_output_reconnect_can(reconnecting: bool, active: bool, code: c_int) -> bool;
}
