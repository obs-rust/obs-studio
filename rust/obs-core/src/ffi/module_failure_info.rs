//! Public module failure-info cleanup ABI, from `libobs/obs.h`.

use std::ffi::c_char;

/// Layout measured against the unchanged public header in Tier 3.
#[repr(C)]
pub struct ModuleFailureInfo {
    pub failed_modules: *mut *mut c_char,
    pub count: usize,
}

/// Release only the table; preserve the count and borrowed strings.
///
/// # Safety
/// `info` must point to a writable `obs_module_failure_info`. Its table must
/// be null or a live libobs allocation. The C implementation also requires
/// a non-null `info`; it does not traverse the table or validate `count`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_module_failure_info_free(info: *mut ModuleFailureInfo) {
    // SAFETY: the caller supplies the writable field and its allocation.
    unsafe {
        crate::module_failure_info::free_table(&mut (*info).failed_modules, |table| {
            obs_util::ffi::darray::bfree(table.cast());
        });
    }
}
