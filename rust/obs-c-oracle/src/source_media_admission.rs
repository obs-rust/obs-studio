//! `oracle_*` build of `libobs/obs-source-media-admission.c.h`
//! (oracle/source_media_admission.c): the media action admission
//! decisions, renamed. The `bool` results are declared as `u8`, the byte
//! the C returns, so a parity test compares it without assuming it is 0 or
//! 1; the enums as [`c_int`], the width the C passes and returns them in.

use std::os::raw::{c_char, c_int};

/// Index of each entry in `oracle_source_media_admission_value`, in the
/// C's order.
pub const VALUE_NAMES: [&str; 40] = [
    "OBS_SOURCE_MEDIA_CONTROLLABLE",
    "OBS_SOURCE_CONTROLLABLE_MEDIA",
    "OBS_SOURCE_MEDIA_STATE_NONE",
    "OBS_MEDIA_STATE_NONE",
    "OBS_SOURCE_MEDIA_ACTION_NONE",
    "OBS_SOURCE_MEDIA_ACTION_PLAY_PAUSE",
    "OBS_SOURCE_MEDIA_ACTION_RESTART",
    "OBS_SOURCE_MEDIA_ACTION_STOP",
    "OBS_SOURCE_MEDIA_ACTION_NEXT",
    "OBS_SOURCE_MEDIA_ACTION_PREVIOUS",
    "OBS_SOURCE_MEDIA_ACTION_SET_TIME",
    "OBS_SOURCE_MEDIA_CALL_PLAY_PAUSE",
    "OBS_SOURCE_MEDIA_CALL_RESTART",
    "OBS_SOURCE_MEDIA_CALL_STOP",
    "OBS_SOURCE_MEDIA_CALL_NEXT",
    "OBS_SOURCE_MEDIA_CALL_PREVIOUS",
    "OBS_SOURCE_MEDIA_CALL_GET_DURATION",
    "OBS_SOURCE_MEDIA_CALL_GET_TIME",
    "OBS_SOURCE_MEDIA_CALL_SET_TIME",
    "OBS_SOURCE_MEDIA_CALL_GET_STATE",
    "OBS_SOURCE_MEDIA_CALL_STARTED",
    "OBS_SOURCE_MEDIA_CALL_ENDED",
    "OBS_SOURCE_MEDIA_ADMIT_REJECT",
    "OBS_SOURCE_MEDIA_ADMIT_ACCEPT",
    "OBS_SOURCE_MEDIA_ADMIT_READ_FLAGS",
    "OBS_SOURCE_MEDIA_ADMIT_READ_CALLBACK",
    "OBS_SOURCE_MEDIA_DISPATCH_RETURN",
    "OBS_SOURCE_MEDIA_DISPATCH_PLAY_PAUSE",
    "OBS_SOURCE_MEDIA_DISPATCH_RESTART",
    "OBS_SOURCE_MEDIA_DISPATCH_STOP",
    "OBS_SOURCE_MEDIA_DISPATCH_NEXT",
    "OBS_SOURCE_MEDIA_DISPATCH_PREVIOUS",
    "OBS_SOURCE_MEDIA_DISPATCH_SET_TIME",
    "OBS_SOURCE_MEDIA_DISPATCH_SKIP",
    "sizeof(enum obs_source_media_call)",
    "sizeof(enum obs_source_media_admit)",
    "sizeof(enum obs_source_media_dispatch)",
    "sizeof(enum obs_media_state)",
    "sizeof(int)",
    "sizeof(bool)",
];

unsafe extern "C" {
    /// `enum obs_source_media_admit obs_source_media_admit(bool valid)`.
    pub fn oracle_obs_source_media_admit(valid: bool) -> c_int;
    /// `enum obs_source_media_admit obs_source_media_admit_flags(enum
    /// obs_source_media_call call, uint32_t output_flags)`.
    pub fn oracle_obs_source_media_admit_flags(call: c_int, output_flags: u32) -> c_int;
    /// `enum obs_source_media_admit obs_source_media_admit_callback(bool
    /// has_callback)`.
    pub fn oracle_obs_source_media_admit_callback(has_callback: bool) -> c_int;
    /// `int64_t obs_source_media_time_fallback(void)`.
    pub fn oracle_obs_source_media_time_fallback() -> i64;
    /// `int obs_source_media_state_fallback(void)`.
    pub fn oracle_obs_source_media_state_fallback() -> c_int;
    /// `enum obs_source_media_dispatch obs_source_media_dispatch(int
    /// type)`.
    pub fn oracle_obs_source_media_dispatch(action_type: c_int) -> c_int;
    /// `bool obs_source_media_signal_reads_pause(enum
    /// obs_source_media_dispatch dispatch)`.
    pub fn oracle_obs_source_media_signal_reads_pause(dispatch: c_int) -> u8;
    /// `const char *obs_source_media_signal(enum obs_source_media_dispatch
    /// dispatch, bool pause)`: a string literal, or NULL.
    pub fn oracle_obs_source_media_signal(dispatch: c_int, pause: bool) -> *const c_char;
    /// The entry named `VALUE_NAMES[which]`; `i64::MIN` out of range.
    pub fn oracle_source_media_admission_value(which: usize) -> i64;
}
