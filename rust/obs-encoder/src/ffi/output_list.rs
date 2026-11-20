//! `obs_encoder_add_output()` and `obs_encoder_remove_output()` over
//! C-owned state: the encoder's output list, its `darray` buffer and its
//! `outputs_mutex`, behind the private adapters of
//! `libobs/obs-encoder-outputs-fields.c.h`.
//!
//! Declared in `obs-internal.h` without `EXPORT`, so both shims stay hidden
//! (`rust-hidden.d/obs-encoder-output_list.txt`). The NULL checks, the lock
//! pair and the first-match search are the safe core's
//! ([`crate::output_list`]); the buffer is grown, shifted and freed only by
//! the C darray helpers, never by Rust.
use core::ffi::c_void;
use core::ptr::NonNull;

/// `struct obs_output`, opaque here.
pub type Output = c_void;

unsafe extern "C" {
    fn obs_encoder_outputs_lock(encoder: *mut c_void);
    fn obs_encoder_outputs_unlock(encoder: *mut c_void);
    fn obs_encoder_outputs_push(encoder: *mut c_void, output: *mut Output);
    fn obs_encoder_outputs_count(encoder: *const c_void) -> usize;
    fn obs_encoder_outputs_at(encoder: *const c_void, index: usize) -> *mut Output;
    fn obs_encoder_outputs_erase(encoder: *mut c_void, index: usize);
}

/// A live, non-NULL encoder.
struct Encoder(NonNull<c_void>);

impl crate::output_list::Outputs for Encoder {
    type Output = NonNull<Output>;
    fn lock(&mut self) {
        // SAFETY: the caller's live encoder, its outputs_mutex initialized
        // and not held by this thread; the core locks once and unlocks once.
        unsafe { obs_encoder_outputs_lock(self.0.as_ptr()) }
    }
    fn unlock(&mut self) {
        // SAFETY: called once, after `lock`.
        unsafe { obs_encoder_outputs_unlock(self.0.as_ptr()) }
    }
    fn push(&mut self, output: NonNull<Output>) {
        // SAFETY: under outputs_mutex, which `lock` holds; the C darray
        // grows the libobs-owned buffer.
        unsafe { obs_encoder_outputs_push(self.0.as_ptr(), output.as_ptr()) }
    }
    fn count(&mut self) -> usize {
        // SAFETY: read under outputs_mutex.
        unsafe { obs_encoder_outputs_count(self.0.as_ptr()) }
    }
    fn at(&mut self, index: usize) -> Option<NonNull<Output>> {
        // SAFETY: `index < count()` under the same hold of outputs_mutex.
        NonNull::new(unsafe { obs_encoder_outputs_at(self.0.as_ptr(), index) })
    }
    fn erase(&mut self, index: usize) {
        // SAFETY: as in `at`; the C darray shifts the rest in place.
        unsafe { obs_encoder_outputs_erase(self.0.as_ptr(), index) }
    }
}

/// Append `output` to the encoder's output list; a NULL encoder or output
/// does nothing.
///
/// # Safety
/// As for the C function: a non-NULL `encoder` is live, with its
/// `outputs_mutex` initialized and not held by this thread when `output` is
/// non-NULL. `output` is only stored, never read.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_add_output(encoder: *mut c_void, output: *mut Output) {
    let mut encoder = NonNull::new(encoder).map(Encoder);
    crate::output_list::add_output(encoder.as_mut(), NonNull::new(output));
}

/// Erase the first entry equal to `output` from the encoder's output list;
/// a NULL encoder or output, or an output not listed, changes nothing.
///
/// # Safety
/// As for [`obs_encoder_add_output`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_remove_output(encoder: *mut c_void, output: *mut Output) {
    let mut encoder = NonNull::new(encoder).map(Encoder);
    crate::output_list::remove_output(encoder.as_mut(), NonNull::new(output));
}
