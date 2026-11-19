//! C boundary for `obs_output_get_total_bytes`; the output stays C-owned.
//!
//! libobs's hidden `obs_output_total_bytes_fields()` lends the addresses of
//! the callback slot, both delay flags and `context.data`, so each read
//! happens where the C reads it: no snapshot, and the callback slot is read
//! again at dispatch as the C reads `output->info.get_total_bytes` twice.
use crate::output_total_bytes::{Host, query};
use core::ffi::{c_char, c_int, c_void};
use core::sync::atomic::{AtomicBool, Ordering};

/// `uint64_t (*get_total_bytes)(void *data)` from `struct obs_output_info`.
pub type TotalBytesFn = unsafe extern "C" fn(*mut c_void) -> u64;

/// `struct obs_output_total_bytes_fields` (`obs-internal.h`).
#[repr(C)]
pub struct TotalBytesFields {
    pub get_total_bytes: *const Option<TotalBytesFn>,
    pub delay_active: *const bool,
    pub delay_capturing: *const bool,
    pub data: *const *mut c_void,
}

unsafe extern "C" {
    fn obs_output_total_bytes_fields(output: *const c_void) -> TotalBytesFields;
    fn blog(level: c_int, format: *const c_char, ...);
}

// Only the unsafe export constructs this from a live, non-NULL output.
struct COutput(TotalBytesFields);

impl COutput {
    fn callback(&self) -> Option<TotalBytesFn> {
        // SAFETY: the slot lives in the live output; a plain read, as in C.
        unsafe { self.0.get_total_bytes.read() }
    }
}

impl Host for COutput {
    fn has_callback(&self) -> bool {
        self.callback().is_some()
    }
    fn delay_active(&self) -> bool {
        // SAFETY: live aligned flag that C accesses with os_atomic_*_bool.
        unsafe { AtomicBool::from_ptr(self.0.delay_active.cast_mut()) }.load(Ordering::SeqCst)
    }
    fn delay_capturing(&self) -> bool {
        // SAFETY: live aligned flag that C accesses with os_atomic_*_bool.
        unsafe { AtomicBool::from_ptr(self.0.delay_capturing.cast_mut()) }.load(Ordering::SeqCst)
    }
    fn dispatch(&self) -> u64 {
        let callback = self.callback().expect("checked callback");
        // SAFETY: the current context.data, read where the C reads it.
        let data = unsafe { self.0.data.read() };
        // SAFETY: the output's own callback receives its own data, as in C.
        unsafe { callback(data) }
    }
}

/// # Safety
/// A non-NULL output must stay live for the call, as in the C API contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_get_total_bytes(output: *const c_void) -> u64 {
    let host = if output.is_null() {
        None
    } else {
        // SAFETY: checked for NULL before the adapter forms field addresses.
        Some(COutput(unsafe { obs_output_total_bytes_fields(output) }))
    };
    query(host.as_ref(), || {
        // SAFETY: literal format with two static NUL-terminated strings.
        unsafe {
            blog(
                400,
                c"%s: Null '%s' parameter".as_ptr(),
                c"obs_output_get_total_bytes".as_ptr(),
                c"output".as_ptr(),
            )
        }
    })
}
