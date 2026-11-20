//! RED stubs for `obs_encoder_add_output()` and `obs_encoder_remove_output()`:
//! not exported yet (the original C still links), and they do nothing, so
//! the differential tests compare the original against an empty candidate.
use core::ffi::c_void;

/// RED stub: does nothing.
///
/// # Safety
/// None needed: the stub reads nothing.
pub unsafe extern "C" fn obs_encoder_add_output(_encoder: *mut c_void, _output: *mut c_void) {}

/// RED stub: does nothing.
///
/// # Safety
/// None needed: the stub reads nothing.
pub unsafe extern "C" fn obs_encoder_remove_output(_encoder: *mut c_void, _output: *mut c_void) {}
