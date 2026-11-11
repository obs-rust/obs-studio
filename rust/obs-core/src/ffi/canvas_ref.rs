//! Canvas handles and the unchanged private C weak-control layout.
use crate::canvas_ref;
use core::ffi::c_void;
use obs_util::ffi::darray::bfree;
use obs_util::platform::CLongAtomic;

#[repr(C)]
pub struct WeakRef {
    pub refs: CLongAtomic,
    pub weak_refs: CLongAtomic,
}

#[repr(C)]
pub struct WeakCanvas {
    pub counts: WeakRef,
    pub canvas: *mut c_void,
}

unsafe extern "C" {
    fn obs_canvas_ref_control(canvas: *const c_void) -> *mut WeakCanvas;
}

/// # Safety
/// Non-null weak handles must be live, aligned controls held by the caller.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_weak_canvas_addref(weak: *mut WeakCanvas) {
    // SAFETY: a retained control has live atomic fields; NULL maps to None.
    canvas_ref::addref(unsafe { weak.as_ref() }.map(|weak| &weak.counts.weak_refs));
}

/// # Safety
/// Non-null weak handles must own a reference. The final release invalidates it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_weak_canvas_release(weak: *mut WeakCanvas) {
    // SAFETY: the caller owns this weak reference throughout the count operation.
    let free = canvas_ref::release(unsafe { weak.as_ref() }.map(|weak| &weak.counts.weak_refs));
    if free {
        // SAFETY: the final weak owner returns the C allocation to its allocator.
        unsafe { bfree(weak.cast()) };
    }
}

/// # Safety
/// A non-null canvas must be live and retained during the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_canvas_get_ref(canvas: *mut c_void) -> *mut c_void {
    if canvas.is_null() {
        return core::ptr::null_mut();
    }
    // SAFETY: reject NULL before the C field adapter; canvas retains its control.
    unsafe { obs_weak_canvas_get_canvas(obs_canvas_ref_control(canvas)) }
}

/// # Safety
/// A non-null canvas must be live and retained during the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_canvas_get_weak_canvas(canvas: *mut c_void) -> *mut WeakCanvas {
    if canvas.is_null() {
        return core::ptr::null_mut();
    }
    // SAFETY: the caller retains canvas; reject NULL before taking a field address.
    let control = unsafe { obs_canvas_ref_control(canvas) };
    // SAFETY: a live canvas owns its control. NULL controls preserve C's no-op.
    canvas_ref::weak(unsafe { control.as_ref() }.map(|weak| (&weak.counts.weak_refs, control)))
        .unwrap_or(core::ptr::null_mut())
}

/// # Safety
/// A non-null weak handle must be a live control retained throughout the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_weak_canvas_get_canvas(weak: *mut WeakCanvas) -> *mut c_void {
    // SAFETY: the target field is immutable for the control's whole lifetime.
    canvas_ref::acquire(unsafe { weak.as_ref() }.map(|weak| (&weak.counts.refs, weak.canvas)))
        .unwrap_or(core::ptr::null_mut())
}
