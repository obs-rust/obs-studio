//! The original C staging cleanup
//! (`libobs/obs-video-unmap-last-surface-reference.c.h`) and the production
//! bridge's slot accessors, over a real private mix.
use core::ffi::c_void;
unsafe extern "C" {
    /// A mix with every slot NULL; NULL when out of memory.
    pub fn oracle_unmap_last_surface_new() -> *mut c_void;
    pub fn oracle_unmap_last_surface_free(video: *mut c_void);
    /// `NUM_CHANNELS`.
    pub fn oracle_unmap_last_surface_channels() -> usize;
    /// `mapped_surfaces[channel]`, read directly.
    pub fn oracle_unmap_last_surface_slot(video: *mut c_void, channel: usize) -> *mut c_void;
    /// Stores `surface` into `mapped_surfaces[channel]`.
    pub fn oracle_unmap_last_surface_set(video: *mut c_void, channel: usize, surface: *mut c_void);
    /// The original cleanup over `video`, unmapping through `unmap`.
    pub fn oracle_unmap_last_surface_original(
        video: *mut c_void,
        unmap: unsafe extern "C" fn(*mut c_void),
    );
    /// The bridge's slot accessor.
    pub fn oracle_unmap_last_surface_mapped(video: *mut c_void, channel: usize) -> *mut c_void;
    /// The bridge's slot reset.
    pub fn oracle_unmap_last_surface_clear(video: *mut c_void, channel: usize);
}
