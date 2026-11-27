//! C ABI of `obs_source_addref()`, `obs_source_release()`,
//! `obs_source_get_ref()` and `obs_weak_source_get_source()` (`EXPORT` in
//! `obs.h`, so listed in `libobs/cmake/rust-exports.d`). The source and its
//! control stay C-owned: the control, its strong count and named source, the
//! core-alive check and `obs_source_destroy()` are the hidden adapters of
//! `obs-source-refs-fields.c.h`. The source's own weak reference goes to the
//! Rust `obs_weak_source_release()`.
use crate::ffi::source_weak_refs::obs_weak_source_release;
use crate::strong_refs::{self, Host};
use core::ffi::{c_char, c_int, c_void};
use core::ptr::{NonNull, null_mut};
use obs_util::platform::CLongAtomic;

unsafe extern "C" {
    fn obs_source_refs_control(source: *mut c_void) -> *mut c_void;
    fn obs_weak_source_refs_field(weak: *mut c_void) -> *const CLongAtomic;
    fn obs_weak_source_source_field(weak: *const c_void) -> *mut c_void;
    fn obs_source_refs_core_alive() -> bool;
    fn obs_source_refs_destroy(source: *mut c_void);
    fn blog(level: c_int, format: *const c_char, ...);
}

/// `LOG_WARNING`.
const LOG_WARNING: c_int = 200;

/// libobs: each method is the one C step the original made.
struct Libobs;

impl Host for Libobs {
    /// Raw: a control names its source as stored, NULL included.
    type Source = *mut c_void;
    type Control = NonNull<c_void>;
    fn core_alive(&mut self) -> bool {
        // SAFETY: reads the core pointer; callers synchronize shutdown, as in C.
        unsafe { obs_source_refs_core_alive() }
    }
    fn warn_shutdown(&mut self) {
        // SAFETY: static NUL-terminated format with no conversions.
        unsafe {
            blog(
                LOG_WARNING,
                c"Tried to release a source when the OBS core is shut down!".as_ptr(),
            );
        }
    }
    fn control(&mut self, source: *mut c_void) -> Option<NonNull<c_void>> {
        // SAFETY: a live source (the entries reject NULL first).
        NonNull::new(unsafe { obs_source_refs_control(source) })
    }
    fn refs(&self, control: NonNull<c_void>) -> &CLongAtomic {
        // SAFETY: the caller's reference (strong, or weak for a promotion)
        // keeps the control and its aligned native long alive across the
        // count operation; C accesses it atomically too.
        unsafe { &*obs_weak_source_refs_field(control.as_ptr()) }
    }
    fn source(&mut self, control: NonNull<c_void>) -> *mut c_void {
        // SAFETY: a live control, just promoted; it names its source.
        unsafe { obs_weak_source_source_field(control.as_ptr()) }
    }
    fn destroy(&mut self, source: *mut c_void) {
        // SAFETY: the last strong owner destroys the source exactly once; it
        // is not read again here.
        unsafe { obs_source_refs_destroy(source) }
    }
    fn release_weak(&mut self, control: NonNull<c_void>) {
        // SAFETY: the source's own weak reference keeps the control alive
        // across the destroy; this drops it.
        unsafe { obs_weak_source_release(control.as_ptr()) }
    }
}

fn source_in(source: *mut c_void) -> Option<*mut c_void> {
    (!source.is_null()).then_some(source)
}

fn out(source: Option<*mut c_void>) -> *mut c_void {
    source.unwrap_or(null_mut())
}

/// # Safety
/// A non-NULL source must be live and own its control.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_addref(source: *mut c_void) {
    if source.is_null() {
        return;
    }
    // SAFETY: a live source; its control and the count outlive this call.
    let refs = unsafe {
        let control = obs_source_refs_control(source);
        &*obs_weak_source_refs_field(control)
    };
    strong_refs::addref(Some(refs));
}

/// # Safety
/// A non-NULL source must hold a strong reference the caller gives up; the
/// final release destroys it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_release(source: *mut c_void) {
    strong_refs::release(&mut Libobs, source_in(source));
}

/// # Safety
/// A non-NULL source must be live (a reference, or a pointer the caller
/// knows is still allocated).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_get_ref(source: *mut c_void) -> *mut c_void {
    out(strong_refs::get_ref(&mut Libobs, source_in(source)))
}

/// # Safety
/// A non-NULL handle must be a control the caller holds a weak reference on.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_weak_source_get_source(weak: *mut c_void) -> *mut c_void {
    out(strong_refs::weak_get_source(
        &mut Libobs,
        NonNull::new(weak),
    ))
}
