//! Load the borrowed native C long with the original atomic ordering.
use core::ffi::{c_long, c_void};
use core::sync::atomic::{AtomicI32, AtomicI64, Ordering};

unsafe extern "C" {
    fn obs_weak_object_expired_field(weak: *const c_void) -> *const c_long;
}

/// # Safety
/// Non-null weak handles must remain live, with the strong count aligned for
/// atomic native-long accesses. Concurrent C accesses must also be atomic.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_weak_object_expired(weak: *mut c_void) -> bool {
    let owners = if weak.is_null() {
        None
    } else {
        // SAFETY: NULL was rejected before the adapter takes a member address.
        let field = unsafe { obs_weak_object_expired_field(weak) };
        Some(if size_of::<c_long>() == size_of::<i64>() {
            // SAFETY: live, aligned 64-bit C long, accessed atomically by C too.
            unsafe { AtomicI64::from_ptr(field.cast_mut().cast()) }.load(Ordering::SeqCst)
        } else {
            // SAFETY: live, aligned 32-bit C long, accessed atomically by C too.
            i64::from(
                unsafe { AtomicI32::from_ptr(field.cast_mut().cast()) }.load(Ordering::SeqCst),
            )
        })
    };
    crate::weak_object_expired::expired(owners)
}
