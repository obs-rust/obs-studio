//! C ABI for `libobs/obs-source-output-cea708.c.h`:
//! `obs_source_output_cea708()` (`EXPORT` in `obs.h`, `rust-exports.d`).
//!
//! Runs the safe core ([`crate::output_cea708`]) over the source's private
//! caption list, which stays C-owned: `destroying(source)`, the list's
//! `struct darray` and its mutex are reached through the C adapters in
//! `obs-source-caption-callback-fields.c.h`, the same list
//! `obs_source_add_caption_callback()` fills. Each entry is read from the
//! array as it is when it is called, as the C reads `array[i - 1]`, and is
//! called with the caller's source and caption packet.
//!
//! No intentional differences from C. A NULL source is outside the contract
//! (the C reads `source->destroying` first); a NULL callback in the list
//! ends the process in both (the C jumps to address 0, here it aborts).

use core::ffi::c_void;
use obs_util::ffi::darray::darray;

use super::source_caption_callbacks::Pair;
use crate::output_cea708::{self, Locked, Source};

unsafe extern "C" {
    fn obs_source_caption_callback_fields(source: *mut c_void) -> *mut darray;
    fn obs_source_caption_callback_lock(source: *mut c_void);
    fn obs_source_caption_callback_unlock(source: *mut c_void);
    fn obs_source_caption_callback_destroying(source: *const c_void) -> bool;
}

/// A live, non-null source and the caption packet it dispatches.
struct Live {
    source: *mut c_void,
    captions: *const c_void,
}

/// The source's caption list, with its mutex held.
struct Held<'a> {
    live: &'a Live,
    list: *mut darray,
}

impl Locked for Held<'_> {
    fn count(&self) -> usize {
        // SAFETY: the source owns this list and its mutex is held.
        unsafe { (*self.list).num }
    }
    fn call(&mut self, index: usize) {
        // SAFETY: as the C, `index` is below the count read under this lock;
        // the entry is read from the array as it is now, and `Pair` mirrors
        // `struct caption_cb_info` (checked against the real headers).
        let pair = unsafe { *(*self.list).array.cast::<Pair>().add(index) };
        let Some(callback) = pair.callback else {
            std::process::abort();
        };
        // SAFETY: a registered callback called as `obs_source_caption_t`,
        // with its own param, the caller's source and caption packet.
        unsafe { callback(pair.param, self.live.source, self.live.captions) };
    }
}

impl Drop for Held<'_> {
    fn drop(&mut self) {
        // SAFETY: `Live::lock` took this live source's list mutex.
        unsafe { obs_source_caption_callback_unlock(self.live.source) };
    }
}

impl Source for Live {
    type Locked<'a> = Held<'a>;
    fn destroying(&self) -> bool {
        // SAFETY: the caller's source is live and non-null.
        unsafe { obs_source_caption_callback_destroying(self.source) }
    }
    fn lock(&self) -> Held<'_> {
        // SAFETY: the caller's source is live; its list is read only after
        // its mutex is taken.
        unsafe {
            obs_source_caption_callback_lock(self.source);
            Held {
                live: self,
                list: obs_source_caption_callback_fields(self.source),
            }
        }
    }
}

/// `obs_source_output_cea708(source, captions)`.
///
/// # Safety
/// `source` must be a live, non-null source (the C dereferences it before
/// any check). `captions` is NULL or what the caller hands its callbacks;
/// every registered callback must be a valid `obs_source_caption_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_output_cea708(source: *mut c_void, captions: *const c_void) {
    let live = Live { source, captions };
    output_cea708::output(&live, !captions.is_null());
}
