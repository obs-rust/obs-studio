//! RED stub for `obs_encoder_group_num_started()`: not exported yet (the
//! original C still links), and it reads nothing, so the differential tests
//! compare the original against an empty candidate.
use core::ffi::c_void;

/// RED stub: reads nothing and returns 0.
///
/// # Safety
/// None needed: the stub reads nothing.
pub unsafe extern "C" fn obs_encoder_group_num_started(_group: *mut c_void) -> u32 {
    0
}
