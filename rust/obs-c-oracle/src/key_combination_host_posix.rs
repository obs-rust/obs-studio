//! `oracle_*` build of `libobs/obs-key-combination-host-posix.c.h`
//! (oracle/key_combination_host_posix.c).
//!
//! The text is a `struct dstr`, passed as an untyped pointer: callers use
//! `obs_util::ffi::dstr::dstr`, whose layout the dstr oracle checks. The
//! oracle file also defines `obs_key_to_str` under its real name, a
//! stand-in that calls the hook set with
//! [`oracle_key_combination_set_key_to_str`]; with no hook the name stays
//! empty.

use core::ffi::{c_int, c_void};

pub use crate::nix_hotkey_state::OracleKeyCombination;

/// The stand-in `obs_key_to_str`'s hook, with the dstr untyped.
pub type OracleKeyToStrFn = Option<unsafe extern "C" fn(key: c_int, str: *mut c_void)>;

unsafe extern "C" {
    pub fn oracle_obs_key_combination_to_str(combination: OracleKeyCombination, str: *mut c_void);
    /// Sets this thread's hook for the stand-in `obs_key_to_str`.
    pub fn oracle_key_combination_set_key_to_str(hook: OracleKeyToStrFn);
}
