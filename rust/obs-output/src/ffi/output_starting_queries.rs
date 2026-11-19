//! Private C ABI; C owns the output layout and every atomic operation.
use crate::output_starting_queries::{self as decisions, Flag};
use core::ffi::c_void;

unsafe extern "C" {
    fn obs_output_starting_load_active(output: *const c_void) -> bool;
    fn obs_output_starting_load_starting(output: *const c_void) -> bool;
    fn obs_output_starting_load_reconnecting(output: *const c_void) -> bool;
    fn obs_output_starting_load_delay_active(output: *const c_void) -> bool;
}

fn load(output: *const c_void, flag: Flag) -> bool {
    // SAFETY: shims reject NULL before any adapter; caller keeps output live.
    // Each adapter performs exactly one C-owned atomic load when requested.
    unsafe {
        match flag {
            Flag::Active => obs_output_starting_load_active(output),
            Flag::Starting => obs_output_starting_load_starting(output),
            Flag::Reconnecting => obs_output_starting_load_reconnecting(output),
            Flag::DelayActive => obs_output_starting_load_delay_active(output),
        }
    }
}

/// # Safety
/// A non-null output must be live throughout the call, with C atomic writers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_needs_encoders(output: *const c_void) -> bool {
    if output.is_null() {
        return false;
    }
    decisions::needs_encoders(|flag| load(output, flag))
}

/// # Safety
/// A non-null output must be live throughout the call, with C atomic writers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_starting(output: *const c_void) -> bool {
    if output.is_null() {
        return false;
    }
    decisions::starting(|flag| load(output, flag))
}
