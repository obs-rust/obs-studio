//! Borrow private C fields without acquiring references or mirroring layouts.
use core::ffi::{c_long, c_void};
use core::sync::atomic::{AtomicI32, AtomicI64, Ordering};

unsafe extern "C" {
    fn obs_source_weak_predicates_refs_field(weak: *const c_void) -> *const c_long;
    fn obs_source_weak_predicates_source_field(weak: *const c_void) -> *const c_void;
}

unsafe fn owners(weak: *const c_void) -> Option<i64> {
    if weak.is_null() {
        return None;
    }
    // SAFETY: NULL is rejected before forming a member address. The caller
    // holds the weak control block; C accesses this aligned long atomically.
    let field = unsafe { obs_source_weak_predicates_refs_field(weak) };
    Some(if size_of::<c_long>() == size_of::<i64>() {
        // SAFETY: the native C long is an aligned, live 64-bit atomic field.
        unsafe { AtomicI64::from_ptr(field.cast_mut().cast()) }.load(Ordering::SeqCst)
    } else {
        // SAFETY: the native C long is an aligned, live 32-bit atomic field.
        i64::from(unsafe { AtomicI32::from_ptr(field.cast_mut().cast()) }.load(Ordering::SeqCst))
    })
}

unsafe fn stored(weak: *const c_void) -> Option<*const c_void> {
    if weak.is_null() {
        return None;
    }
    // SAFETY: NULL is rejected first. The stored pointer is immutable during
    // the control block's lifetime and is compared only, never dereferenced.
    let source = unsafe { obs_source_weak_predicates_source_field(weak) };
    (!source.is_null()).then_some(source)
}

/// # Safety
/// A non-null weak control block must remain live and aligned throughout.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_weak_source_expired(weak: *mut c_void) -> bool {
    // SAFETY: inherited weak-handle lifetime and atomic-access contract.
    crate::weak_predicates::expired(unsafe { owners(weak) })
}

/// # Safety
/// Non-null weak must remain live; source is a borrowed identity, never read.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_weak_source_references_source(
    weak: *mut c_void,
    source: *mut c_void,
) -> bool {
    // SAFETY: inherited weak-handle lifetime; stored rejects NULL first.
    crate::weak_predicates::references(
        unsafe { stored(weak) },
        (!source.is_null()).then_some(source.cast_const()),
    )
}
