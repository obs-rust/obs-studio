//! `oracle_*` build of `libobs/obs-audio-controls.c.h`
//! (oracle/audio_controls.c).

use std::ffi::{c_int, c_void};

unsafe extern "C" {
    pub fn oracle_obs_fader_create(ty: c_int) -> *mut c_void;
    pub fn oracle_obs_fader_destroy(fader: *mut c_void);
    pub fn oracle_obs_fader_set_db(fader: *mut c_void, db: f32) -> bool;
    pub fn oracle_obs_fader_get_db(fader: *mut c_void) -> f32;
    pub fn oracle_obs_fader_set_deflection(fader: *mut c_void, def: f32) -> bool;
    pub fn oracle_obs_fader_get_deflection(fader: *mut c_void) -> f32;
    pub fn oracle_obs_fader_set_mul(fader: *mut c_void, mul: f32) -> bool;
    pub fn oracle_obs_fader_get_mul(fader: *mut c_void) -> f32;
    pub fn oracle_obs_fader_db_to_def(fader: *mut c_void) -> extern "C" fn(f32) -> f32;
    pub fn oracle_obs_volmeter_create(ty: c_int) -> *mut c_void;
    pub fn oracle_obs_volmeter_destroy(volmeter: *mut c_void);
    pub fn oracle_obs_mul_to_db(mul: f32) -> f32;
    pub fn oracle_obs_db_to_mul(db: f32) -> f32;
    pub fn oracle_obs_audio_info_size() -> usize;
    pub fn oracle_obs_audio_info_align() -> usize;
    pub fn oracle_obs_audio_info_speakers_offset() -> usize;
    /// `get_sample_peak()` (#3003). `previous` holds 4 floats; `samples`
    /// is 16-byte aligned and holds `nr_samples` floats.
    pub fn oracle_get_sample_peak(
        previous: *const f32,
        samples: *const f32,
        nr_samples: usize,
    ) -> f32;
    /// `get_true_peak()` (#3003), with the same arguments.
    pub fn oracle_get_true_peak(
        previous: *const f32,
        samples: *const f32,
        nr_samples: usize,
    ) -> f32;
}
