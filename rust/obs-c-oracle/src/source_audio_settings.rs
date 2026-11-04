//! `oracle_*` builds of `libobs/obs-source-audio-settings.c.h`
//! (oracle/source_audio_settings.c): the source audio settings, renamed, with
//! the seam (`libobs/obs-source-audio-settings-seam.c.h`) compiled under its
//! own names, which the Rust shims call too. A test source is a zeroed
//! `struct obs_source` with its mutexes, a real signal handler and a log of
//! each signal and monitor call. Sources cross as opaque pointers, the enum as
//! `int`.

use core::ffi::{c_char, c_int, c_void};

unsafe extern "C" {
    /// A test source. With `replace`, the handlers put `replace_float` or
    /// `replace_int` in the calldata. Free it with
    /// [`oracle_audio_settings_free`].
    pub fn oracle_audio_settings_new(
        private_source: bool,
        replace: bool,
        replace_float: f64,
        replace_int: i64,
        volume: f32,
        balance: f32,
    ) -> *mut c_void;
    /// Frees a test source.
    pub fn oracle_audio_settings_free(source: *mut c_void);
    /// The log, the queued action values, every field and whether both
    /// mutexes are free, as text. Returns its length, or 0 when it does not
    /// fit in `cap`.
    pub fn oracle_audio_settings_state(
        source: *const c_void,
        out: *mut c_char,
        cap: usize,
    ) -> usize;

    /// `obs_source_set_volume`.
    pub fn oracle_obs_source_set_volume(source: *mut c_void, volume: f32);
    /// `obs_source_get_volume`.
    pub fn oracle_obs_source_get_volume(source: *const c_void) -> f32;
    /// `obs_source_set_sync_offset`.
    pub fn oracle_obs_source_set_sync_offset(source: *mut c_void, offset: i64);
    /// `obs_source_get_sync_offset`.
    pub fn oracle_obs_source_get_sync_offset(source: *const c_void) -> i64;
    /// `obs_source_muted`.
    pub fn oracle_obs_source_muted(source: *const c_void) -> bool;
    /// `obs_source_set_muted`.
    pub fn oracle_obs_source_set_muted(source: *mut c_void, muted: bool);
    /// `obs_source_set_monitoring_enabled`.
    pub fn oracle_obs_source_set_monitoring_enabled(source: *mut c_void, enabled: bool);
    /// `obs_source_get_monitoring_enabled`.
    pub fn oracle_obs_source_get_monitoring_enabled(source: *const c_void) -> bool;
    /// `obs_source_set_monitoring_type`.
    pub fn oracle_obs_source_set_monitoring_type(source: *mut c_void, monitoring_type: c_int);
    /// `obs_source_get_monitoring_type`.
    pub fn oracle_obs_source_get_monitoring_type(source: *const c_void) -> c_int;
    /// `obs_source_set_balance_value`.
    pub fn oracle_obs_source_set_balance_value(source: *mut c_void, balance: f32);
    /// `obs_source_get_balance_value`.
    pub fn oracle_obs_source_get_balance_value(source: *const c_void) -> f32;
}
