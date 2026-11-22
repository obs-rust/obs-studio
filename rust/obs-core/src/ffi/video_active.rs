//! `obs_video_active()` over C-owned state: the core's mix list, its mutex
//! and each mix's activity counts, behind the private adapters of
//! `libobs/obs-video-active-fields.c.h`. The order of the lock, the reads and
//! the unlock is the safe core's ([`crate::video_active::video_active`]);
//! nothing is written and no reference is taken.
use core::ffi::{c_long, c_void};

/// `struct obs_core_video_mix`, opaque here.
pub type Mix = c_void;

unsafe extern "C" {
    fn obs_video_active_lock();
    fn obs_video_active_unlock();
    fn obs_video_active_count() -> usize;
    fn obs_video_active_at(index: usize) -> *mut Mix;
    fn obs_video_active_raw(mix: *mut Mix) -> c_long;
    fn obs_video_active_gpu(mix: *mut Mix) -> c_long;
}

struct Core;

// `c_long` is `i64` on LP64 hosts and `i32` on Windows, so the widening is
// a no-op on some targets only.
#[allow(clippy::useless_conversion)]
impl crate::video_active::Mixes for Core {
    type Mix = *mut Mix;
    fn lock(&mut self) {
        // SAFETY: the live core's mixes_mutex, not held by this thread (the
        // core locks once and unlocks once).
        unsafe { obs_video_active_lock() }
    }
    fn unlock(&mut self) {
        // SAFETY: called once, after `lock`.
        unsafe { obs_video_active_unlock() }
    }
    fn count(&mut self) -> usize {
        // SAFETY: read under mixes_mutex, which `lock` holds.
        unsafe { obs_video_active_count() }
    }
    fn at(&mut self, index: usize) -> *mut Mix {
        // SAFETY: `index < count()` under the same hold of mixes_mutex.
        unsafe { obs_video_active_at(index) }
    }
    fn raw_active(&mut self, mix: &*mut Mix) -> i64 {
        // SAFETY: a listed mix is live while mixes_mutex is held; the adapter
        // loads the count atomically, as the original did.
        i64::from(unsafe { obs_video_active_raw(*mix) })
    }
    fn gpu_encoder_active(&mut self, mix: &*mut Mix) -> i64 {
        // SAFETY: as in `raw_active`.
        i64::from(unsafe { obs_video_active_gpu(*mix) })
    }
}

/// Whether any video mix has a raw consumer or a GPU encoder counted above
/// zero.
///
/// # Safety
/// As for the C function: the core is initialized, its `mixes_mutex` is not
/// held by this thread, and every listed mix is live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_video_active() -> bool {
    crate::video_active::video_active(&mut Core)
}
