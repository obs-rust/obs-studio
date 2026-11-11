//! Private common-context projection; counts reuse the canvas reference core.
use crate::ffi::{canvas_ref::WeakRef, core_initialized::obs_initialized};
use crate::{
    canvas_ref,
    object_ref::{self, Host},
};
use core::ffi::{c_char, c_int, c_void};
use obs_util::{ffi::darray::bfree, platform::CLongAtomic};

#[repr(C)]
pub struct WeakObject {
    pub counts: WeakRef,
    pub object: *mut c_void,
}

unsafe extern "C" {
    fn obs_object_ref_control(object: *const c_void) -> *mut WeakObject;
    fn obs_object_ref_destroy(object: *mut c_void);
    fn blog(level: c_int, format: *const c_char, ...);
}

struct Object(*mut c_void);

impl Host for Object {
    type Control = *mut WeakObject;
    fn initialized(&self) -> bool {
        // SAFETY: callers synchronize core startup and shutdown, as in C.
        unsafe { obs_initialized() }
    }
    fn warn_shutdown(&mut self) {
        // SAFETY: static NUL-terminated format with no arguments, LOG_WARNING.
        unsafe {
            blog(
                200,
                c"Tried to release an object when the OBS core is shut down!".as_ptr(),
            )
        };
    }
    fn control(&self) -> Option<Self::Control> {
        if self.0.is_null() {
            return None;
        }
        // SAFETY: reject NULL before the accessor; the caller owns a strong ref.
        Some(unsafe { obs_object_ref_control(self.0) })
    }
    fn strong(&self, control: Self::Control) -> &CLongAtomic {
        // SAFETY: a valid strong owner retains its non-NULL control throughout
        // the decrement. This borrow ends before destroy can free the context.
        unsafe { &(*control).counts.refs }
    }
    fn destroy(&mut self) {
        // SAFETY: the last strong owner invokes the stored callback exactly once.
        // It may free self.0, which is never read again afterwards.
        unsafe { obs_object_ref_destroy(self.0) };
    }
    fn release_weak(&mut self, control: Self::Control) {
        // SAFETY: the object's weak ownership retains control across destroy.
        let free = canvas_ref::release(Some(unsafe { &(*control).counts.weak_refs }));
        if free {
            // SAFETY: the last weak owner returns the allocation to libobs.
            unsafe { bfree(control.cast()) };
        }
    }
}

/// # Safety
/// A non-NULL context must remain live throughout acquisition.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_object_get_ref(object: *mut c_void) -> *mut c_void {
    if object.is_null() {
        return core::ptr::null_mut();
    }
    // SAFETY: NULL was rejected; a live context owns its control if present.
    let control = unsafe { obs_object_ref_control(object).as_ref() };
    object_ref::get_ref(control.map(|control| (&control.counts.refs, control.object)))
        .unwrap_or(core::ptr::null_mut())
}

/// # Safety
/// A non-NULL context must own a strong reference when the core is initialized.
/// Final release can destroy the context; callers synchronize core shutdown.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_object_release(object: *mut c_void) {
    object_ref::release(&mut Object(object));
}
