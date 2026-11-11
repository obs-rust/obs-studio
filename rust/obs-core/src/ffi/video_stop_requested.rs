//! C ABI for `libobs/obs-video-stop-requested.c.h`, the graphics thread's
//! stop check, declared in `libobs/obs-video-stop-requested.h` without
//! `EXPORT` (hidden, `rust-hidden.d`).
//!
//! The C bridge passes the core and its accessors for `mixes_mutex`,
//! `mixes.num` and mix `i`'s `video`, and `video_output_stopped()`: the mix
//! list and its mutex stay C-owned, and nothing here reads `struct obs_core`.
use crate::video_stop_requested::{self, Mixes};
use core::ffi::c_void;

/// `struct obs_core *`, opaque here.
pub type Core = c_void;
/// `video_t *`, opaque here.
pub type Video = c_void;

pub type Lock = unsafe extern "C" fn(*mut Core);
pub type Count = unsafe extern "C" fn(*mut Core) -> usize;
pub type VideoAt = unsafe extern "C" fn(*mut Core, usize) -> *mut Video;
pub type Stopped = unsafe extern "C" fn(*mut Video) -> bool;

struct Bridge {
    core: *mut Core,
    lock: Lock,
    unlock: Lock,
    count: Count,
    video: VideoAt,
    stopped: Stopped,
}

impl Mixes for Bridge {
    type Video = *mut Video;
    fn lock(&mut self) {
        // SAFETY: the bridge's accessor locks this live core's mixes_mutex.
        unsafe { (self.lock)(self.core) }
    }
    fn unlock(&mut self) {
        // SAFETY: the core calls this once, after its own lock().
        unsafe { (self.unlock)(self.core) }
    }
    fn count(&mut self) -> usize {
        // SAFETY: read under mixes_mutex, which lock() holds.
        unsafe { (self.count)(self.core) }
    }
    fn video(&mut self, index: usize) -> *mut Video {
        // SAFETY: index < count, read under the same mixes_mutex hold, so the
        // array is the one count() measured.
        unsafe { (self.video)(self.core, index) }
    }
    fn stopped(&mut self, video: *mut Video) -> bool {
        // SAFETY: a mix's own video output, still held by the locked mix.
        unsafe { (self.stopped)(video) }
    }
}

/// True when every mix's video output has stopped (and for no mixes).
///
/// # Safety
/// `core` is a live core and the accessors are the bridge's (or behave the
/// same) for it: `count` and `video` are called only under `lock`. None of
/// them may unwind.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_video_stop_requested(
    core: *mut Core,
    lock: Lock,
    unlock: Lock,
    count: Count,
    video: VideoAt,
    stopped: Stopped,
) -> bool {
    video_stop_requested::stop_requested(&mut Bridge {
        core,
        lock,
        unlock,
        count,
        video,
        stopped,
    })
}
