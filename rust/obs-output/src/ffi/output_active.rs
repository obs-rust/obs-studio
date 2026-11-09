//! Borrow atomic flag addresses from the real private C output layout.
use core::ffi::c_void;
use core::sync::atomic::{AtomicBool, Ordering};

#[repr(C)]
pub struct Fields {
    pub active: *const bool,
    pub reconnecting: *const bool,
}

unsafe extern "C" {
    fn obs_output_active_fields(output: *const c_void) -> Fields;
}

/// # Safety
/// A non-null output must remain live with atomically accessed, aligned flags.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_active(output: *const c_void) -> bool {
    // SAFETY: the adapter borrows live flag addresses, or returns null fields.
    let fields = unsafe { obs_output_active_fields(output) };
    let active = if fields.active.is_null() {
        None
    } else {
        // SAFETY: the live C bool is aligned and accessed atomically by C writers.
        Some(unsafe { AtomicBool::from_ptr(fields.active.cast_mut()) }.load(Ordering::SeqCst))
    };
    crate::output_active::get(active, || {
        // SAFETY: non-null output supplies both flags; the core preserves C's lazy load.
        unsafe { AtomicBool::from_ptr(fields.reconnecting.cast_mut()) }.load(Ordering::SeqCst)
    })
}
