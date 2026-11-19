//! Borrow the C-owned validation and signal primitives without source projections.
use crate::update_properties::{self, Source};
use core::ffi::c_void;

unsafe extern "C" {
    fn obs_source_update_properties_valid(source: *mut c_void) -> bool;
    fn obs_source_update_properties_signal(source: *mut c_void);
}

struct BorrowedSource(*mut c_void);

impl Source for BorrowedSource {
    fn valid(&self) -> bool {
        // SAFETY: the original validation accepts NULL without dereferencing it.
        unsafe { obs_source_update_properties_valid(self.0) }
    }

    fn signal_update_properties(&self) {
        // SAFETY: the safe orchestrator validates first. The caller keeps the
        // source and handler live through synchronous, possibly reentrant callbacks.
        unsafe { obs_source_update_properties_signal(self.0) }
    }
}

/// # Safety
/// A non-null source and its signal handler must remain live through the call
/// and synchronous callbacks, under the original C API's ownership contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_update_properties(source: *mut c_void) {
    update_properties::update_properties(&BorrowedSource(source));
}
