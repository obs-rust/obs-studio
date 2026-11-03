//! `oracle_*` build of `libobs/obs-hotkey-save-load.c.h`
//! (oracle/hotkey_save_load.c).
//!
//! The registry, the objects and the obs_data objects are untyped pointers:
//! the test binary defines the seam over a fake registry, and callers use the
//! Rust obs-data port, which both this oracle and the shims read and write.

use core::ffi::c_void;

pub use crate::nix_hotkey_state::OracleKeyCombination;

/// Index of each value in `oracle_hotkey_save_load_value`, in the C's order.
pub const VALUE_NAMES: [&str; 2] = ["OBS_KEY_VK_VOLUME_UP", "OBS_KEY_VK_VOLUME_DOWN"];

unsafe extern "C" {
    pub fn oracle_load_bindings(hotkey: *mut c_void, data: *mut c_void, removed: bool, saved: bool);
    pub fn oracle_obs_hotkey_load_bindings(
        id: usize,
        combinations: *const OracleKeyCombination,
        num: usize,
    );
    pub fn oracle_obs_hotkey_load(id: usize, data: *mut c_void);
    pub fn oracle_obs_hotkeys_load_encoder(encoder: *mut c_void, hotkeys: *mut c_void);
    pub fn oracle_obs_hotkeys_load_output(output: *mut c_void, hotkeys: *mut c_void);
    pub fn oracle_obs_hotkeys_load_service(service: *mut c_void, hotkeys: *mut c_void);
    pub fn oracle_obs_hotkeys_load_source(source: *mut c_void, hotkeys: *mut c_void);
    pub fn oracle_obs_hotkey_pair_load(id: usize, data0: *mut c_void, data1: *mut c_void);
    pub fn oracle_obs_hotkey_save(id: usize) -> *mut c_void;
    pub fn oracle_obs_hotkey_pair_save(
        id: usize,
        p_data0: *mut *mut c_void,
        p_data1: *mut *mut c_void,
    );
    pub fn oracle_obs_hotkeys_save_encoder(encoder: *mut c_void) -> *mut c_void;
    pub fn oracle_obs_hotkeys_save_output(output: *mut c_void) -> *mut c_void;
    pub fn oracle_obs_hotkeys_save_service(service: *mut c_void) -> *mut c_void;
    pub fn oracle_obs_hotkeys_save_source(source: *mut c_void) -> *mut c_void;
    pub fn oracle_hotkey_save_load_value(which: usize) -> i64;
}
