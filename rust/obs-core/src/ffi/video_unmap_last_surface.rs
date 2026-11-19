//! C ABI for `libobs/obs-video-unmap-last-surface.c.h`, the staging cleanup
//! at the start of each output stage, declared in
//! `libobs/obs-video-unmap-last-surface.h` without `EXPORT` (hidden,
//! `rust-hidden.d`).
//!
//! The C bridge passes the mix, `NUM_CHANNELS`, its accessors for
//! `mapped_surfaces[c]` and for resetting that slot, and
//! `gs_stagesurface_unmap()`: the slots stay C-owned, nothing here reads
//! `struct obs_core_video_mix`, and no slot is borrowed across the unmap
//! callback, which may read them.
use crate::video_unmap_last_surface::{self, MappedSurfaces};
use core::ffi::c_void;

/// `struct obs_core_video_mix *`, opaque here.
pub type Mix = c_void;
/// `gs_stagesurf_t *`, opaque here.
pub type Surface = c_void;

pub type Mapped = unsafe extern "C" fn(*mut Mix, usize) -> *mut Surface;
pub type Clear = unsafe extern "C" fn(*mut Mix, usize);
pub type Unmap = unsafe extern "C" fn(*mut Surface);

struct Bridge {
    video: *mut Mix,
    channels: usize,
    mapped: Mapped,
    clear: Clear,
    unmap: Unmap,
}

impl MappedSurfaces for Bridge {
    type Surface = *mut Surface;
    fn channels(&self) -> usize {
        self.channels
    }
    fn mapped(&mut self, channel: usize) -> Option<*mut Surface> {
        // SAFETY: channel < channels, a slot of this live mix.
        let surface = unsafe { (self.mapped)(self.video, channel) };
        (!surface.is_null()).then_some(surface)
    }
    fn unmap(&mut self, surface: *mut Surface) {
        // SAFETY: a non-NULL surface the mix holds mapped, unmapped on the
        // graphics thread that called the bridge.
        unsafe { (self.unmap)(surface) }
    }
    fn clear(&mut self, channel: usize) {
        // SAFETY: channel < channels, a slot of this live mix.
        unsafe { (self.clear)(self.video, channel) }
    }
}

/// Unmaps each mapped staging surface of `video` in channel order and resets
/// its slot after the unmap returns.
///
/// # Safety
/// `video` is a live mix with `channels` slots and the accessors are the
/// bridge's (or behave the same) for it; `unmap` accepts every surface a slot
/// holds. None of them may unwind.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_video_unmap_last_surface(
    video: *mut Mix,
    channels: usize,
    mapped: Mapped,
    clear: Clear,
    unmap: Unmap,
) {
    video_unmap_last_surface::unmap_last_surface(&mut Bridge {
        video,
        channels,
        mapped,
        clear,
        unmap,
    });
}
