//! Thin ABI bridge over the real C control and its unchanged atomic primitives.
use super::service_weak_acquire::WeakRef;
use crate::output_weak_lifetime::{self as policy, Host};
use core::{ffi::c_void, ptr::NonNull};
use obs_util::ffi::darray::bfree;

#[repr(C)]
pub struct WeakOutput {
    pub counts: WeakRef,
    pub output: *mut c_void,
}

unsafe extern "C" {
    fn obs_output_weak_lifetime_control(output: *mut c_void) -> *mut WeakOutput;
    fn obs_output_weak_lifetime_addref(weak: *mut WeakOutput);
    fn obs_output_weak_lifetime_release(weak: *mut WeakOutput) -> bool;
    fn obs_output_weak_lifetime_promote(weak: *mut WeakOutput) -> bool;
}

struct C;
impl Host for C {
    type Weak = NonNull<WeakOutput>;
    type Output = NonNull<c_void>;

    fn control(&self, output: Self::Output) -> Option<Self::Weak> {
        // SAFETY: NonNull rejects NULL before the private field adapter. The
        // caller holds the output, which owns its control for this call.
        NonNull::new(unsafe { obs_output_weak_lifetime_control(output.as_ptr()) })
    }
    fn addref(&self, weak: Self::Weak) {
        // SAFETY: the caller retains the non-null control across this operation.
        unsafe { obs_output_weak_lifetime_addref(weak.as_ptr()) };
    }
    fn release(&self, weak: Self::Weak) -> bool {
        // SAFETY: the caller transfers one weak owner; C performs the atomic decrement.
        unsafe { obs_output_weak_lifetime_release(weak.as_ptr()) }
    }
    fn free(&self, weak: Self::Weak) {
        // SAFETY: policy calls this only after the last weak owner releases.
        unsafe { bfree(weak.as_ptr().cast()) };
    }
    fn promote(&self, weak: Self::Weak) -> bool {
        // SAFETY: a retained control remains live, including when its output expires.
        unsafe { obs_output_weak_lifetime_promote(weak.as_ptr()) }
    }
    fn target(&self, weak: Self::Weak) -> Option<Self::Output> {
        // SAFETY: the target field is immutable throughout the retained control's
        // lifetime. Read identity only; an expired output is never dereferenced.
        NonNull::new(unsafe { weak.as_ref().output })
    }
}

/// # Safety
/// Non-null handles are retained, aligned C weak-output controls.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_weak_output_addref(weak: *mut WeakOutput) {
    policy::addref(&C, NonNull::new(weak));
}

/// # Safety
/// A non-null handle transfers one weak owner and is invalid after final release.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_weak_output_release(weak: *mut WeakOutput) {
    policy::release(&C, NonNull::new(weak));
}

/// # Safety
/// A non-null output is live and retained throughout the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_get_weak_output(output: *mut c_void) -> *mut WeakOutput {
    policy::get_weak(&C, NonNull::new(output)).map_or(core::ptr::null_mut(), NonNull::as_ptr)
}

/// # Safety
/// A non-null weak control is retained throughout the call. Success owns a strong ref.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_weak_output_get_output(weak: *mut WeakOutput) -> *mut c_void {
    policy::get_output(&C, NonNull::new(weak)).map_or(core::ptr::null_mut(), NonNull::as_ptr)
}

/// # Safety
/// A non-null weak control is retained; output is compared only as an identity.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_weak_output_references_output(
    weak: *mut WeakOutput,
    output: *mut c_void,
) -> bool {
    policy::references(&C, NonNull::new(weak), NonNull::new(output))
}
