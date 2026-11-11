//! Convert the opaque source handle through its private C field accessor.
use core::ffi::{c_long, c_void};

unsafe extern "C" {
    fn obs_source_async_rotation_fields(source: *mut c_void) -> *mut c_long;
}

/// # Safety
/// A non-null source must be live and writable under the original C contract.
#[allow(
    clippy::unnecessary_cast,
    reason = "C long is i32 on Windows and i64 on 64-bit Unix"
)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_set_async_rotation(source: *mut c_void, rotation: c_long) {
    if source.is_null() {
        return;
    }
    // SAFETY: null is rejected before C takes a member address; the caller
    // supplies a live source whose rotation may be updated during this call.
    unsafe {
        *obs_source_async_rotation_fields(source) =
            crate::async_rotation::normalize(rotation as i64) as c_long;
    }
}
