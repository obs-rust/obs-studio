//! C ABI for the deprecated module-loading no-ops. Neither export touches the
//! core, so both are safe to call before startup, as in C.
use crate::module_load_all;
use core::ffi::c_void;

/// Deprecated; does nothing.
#[unsafe(no_mangle)]
pub extern "C" fn obs_load_all_modules() {
    module_load_all::load_all_modules();
}

/// Deprecated; does nothing. `mfi` (`struct obs_module_failure_info *`, may be
/// NULL) is never dereferenced, so the caller's count, pointer array and
/// strings stay exactly as they were.
#[unsafe(no_mangle)]
pub extern "C" fn obs_load_all_modules2(mfi: *mut c_void) {
    let _ = mfi;
    module_load_all::load_all_modules2();
}
