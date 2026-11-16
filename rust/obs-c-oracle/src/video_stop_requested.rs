//! The original C stop check (`libobs/obs-video-stop-requested-reference.c.h`)
//! and the production bridge's mix list accessors, over a real private core.
use core::ffi::c_void;
unsafe extern "C" {
    /// A core with `count` mixes; NULL when out of memory.
    pub fn oracle_stop_requested_new(count: usize) -> *mut c_void;
    pub fn oracle_stop_requested_free(core: *mut c_void);
    /// Mix `index`'s video token.
    pub fn oracle_stop_requested_mix_video(core: *mut c_void, index: usize) -> *mut c_void;
    /// Whether the core's mixes_mutex is held.
    pub fn oracle_stop_requested_locked(core: *mut c_void) -> bool;
    /// The original stop check over `core`, asking `stopped`.
    pub fn oracle_stop_requested_original(
        core: *mut c_void,
        stopped: unsafe extern "C" fn(*mut c_void) -> bool,
    ) -> bool;
    pub fn oracle_stop_requested_lock(core: *mut c_void);
    pub fn oracle_stop_requested_unlock(core: *mut c_void);
    pub fn oracle_stop_requested_count(core: *mut c_void) -> usize;
    pub fn oracle_stop_requested_video(core: *mut c_void, index: usize) -> *mut c_void;
}
