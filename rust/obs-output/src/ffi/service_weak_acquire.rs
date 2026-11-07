//! Hidden weak-control acquisition ABI. The layout is unchanged from
//! `obs-internal.h`; Tier 3 checks both structs against that real header.

use crate::service_weak_acquire;
use core::ffi::c_void;
use obs_util::platform::CLongAtomic;

/// `struct obs_weak_ref`, including the platform's C `long` width.
#[repr(C)]
pub struct WeakRef {
    pub refs: CLongAtomic,
    pub weak_refs: CLongAtomic,
}

/// `struct obs_weak_service`; C owns its allocation and lifetime.
#[repr(C)]
pub struct WeakService {
    pub counts: WeakRef,
    pub service: *mut c_void,
}

/// # Safety
/// `weak` is a live, aligned C weak control retained throughout this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_weak_service_weak_get_ref(weak: *mut WeakService) -> bool {
    // SAFETY: C retains the weak allocation. Only the atomic count is read.
    service_weak_acquire::try_acquire(unsafe { &(*weak).counts.refs })
}

/// # Safety
/// `weak` is a live, aligned C weak control retained throughout this call.
/// The returned service is borrowed and may already have been destroyed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_weak_service_target(weak: *const WeakService) -> *mut c_void {
    // SAFETY: the immutable target field outlives the call under a weak ref.
    service_weak_acquire::target(unsafe { (*weak).service })
}
