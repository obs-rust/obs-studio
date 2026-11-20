//! `obs_canvas_clear_mix()` over C-owned state: the canvas's mix slot, the
//! core's mix list and its mutex, and `obs_free_video_mix()`, behind the
//! private adapters of `libobs/obs-canvas-clear-mix-fields.c.h`.
//!
//! Declared in `obs-internal.h` without `EXPORT`, so the shim stays hidden
//! (`rust-hidden.d/obs-core-canvas_clear_mix.txt`). The order of the locks,
//! reads, erase, free and detach is the safe core's
//! ([`crate::canvas_clear_mix::clear_mix`]); no reference is taken or
//! released and nothing else in the canvas or the core is touched.
use core::ffi::c_void;
use core::ptr::{NonNull, null_mut};

/// `struct obs_core_video_mix`, opaque here.
pub type Mix = c_void;

unsafe extern "C" {
    fn obs_canvas_clear_mix_slot(canvas: *mut c_void) -> *mut *mut Mix;
    fn obs_canvas_clear_mix_lock();
    fn obs_canvas_clear_mix_unlock();
    fn obs_canvas_clear_mix_count() -> usize;
    fn obs_canvas_clear_mix_at(index: usize) -> *mut Mix;
    fn obs_canvas_clear_mix_erase(index: usize);
    fn obs_canvas_clear_mix_free(mix: *mut Mix);
}

struct Core {
    canvas: *mut c_void,
}

impl crate::canvas_clear_mix::Mixes for Core {
    type Mix = NonNull<Mix>;
    fn canvas_mix(&mut self) -> Option<NonNull<Mix>> {
        // SAFETY: the caller's live canvas (a NULL one faults here, as the
        // original's first read does); the slot is C-owned.
        NonNull::new(unsafe { *obs_canvas_clear_mix_slot(self.canvas) })
    }
    fn detach(&mut self) {
        // SAFETY: as in `canvas_mix`.
        unsafe { *obs_canvas_clear_mix_slot(self.canvas) = null_mut() };
    }
    fn lock(&mut self) {
        // SAFETY: the live core's mixes_mutex, not held by this thread (the
        // core locks once and unlocks once).
        unsafe { obs_canvas_clear_mix_lock() }
    }
    fn unlock(&mut self) {
        // SAFETY: called once, after `lock`.
        unsafe { obs_canvas_clear_mix_unlock() }
    }
    fn count(&mut self) -> usize {
        // SAFETY: read under mixes_mutex, which `lock` holds.
        unsafe { obs_canvas_clear_mix_count() }
    }
    fn at(&mut self, index: usize) -> Option<NonNull<Mix>> {
        // SAFETY: `index < count()` under the same hold of mixes_mutex.
        NonNull::new(unsafe { obs_canvas_clear_mix_at(index) })
    }
    fn erase(&mut self, index: usize) {
        // SAFETY: as in `at`.
        unsafe { obs_canvas_clear_mix_erase(index) }
    }
    fn free(&mut self, mix: Option<NonNull<Mix>>) {
        // SAFETY: the entry just erased from the list, freed once, under
        // the lock, as in C.
        unsafe { obs_canvas_clear_mix_free(mix.map_or(null_mut(), NonNull::as_ptr)) }
    }
}

/// Detach the canvas's mix, removing it from the core's mix list and
/// freeing it when the list held it.
///
/// # Safety
/// As for the C function: `canvas` is a live canvas (NULL is outside the
/// valid domain and faults, as in C); when it has a mix, the core is live,
/// its `mixes_mutex` is not held by this thread, and every listed mix was
/// made by `obs_create_video_mix()`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_canvas_clear_mix(canvas: *mut c_void) {
    crate::canvas_clear_mix::clear_mix(&mut Core { canvas });
}
