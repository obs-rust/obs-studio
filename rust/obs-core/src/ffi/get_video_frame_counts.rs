//! `obs_get_video_frame_counts()` over C-owned state: the main canvas's mix,
//! the core's mix list and its mutex, the mix fields the frame-count rule
//! asks about and each mix's `video_t` counters, behind the private adapters
//! of `libobs/obs-get-video-frame-counts-fields.c.h`. Which reads are made,
//! and in which order, is the safe core's
//! ([`crate::get_video_frame_counts::get_video_frame_counts`]); the outputs
//! are written after the mutex is released, and nothing else is written.
use core::ffi::c_void;
use core::ptr::NonNull;

/// `struct obs_core_video_mix`, opaque here.
pub type Mix = c_void;

unsafe extern "C" {
    fn obs_get_video_frame_counts_main_mix() -> *mut Mix;
    fn obs_get_video_frame_counts_lock();
    fn obs_get_video_frame_counts_unlock();
    fn obs_get_video_frame_counts_count() -> usize;
    fn obs_get_video_frame_counts_at(index: usize) -> *mut Mix;
    fn obs_get_video_frame_counts_encoder_only(mix: *const Mix) -> bool;
    fn obs_get_video_frame_counts_same_view(mix: *const Mix, main_mix: *const Mix) -> bool;
    fn obs_get_video_frame_counts_has_video(mix: *const Mix) -> bool;
    fn obs_get_video_frame_counts_total(mix: *const Mix) -> u32;
    fn obs_get_video_frame_counts_skipped(mix: *const Mix) -> u32;
}

struct Core;

impl crate::get_video_frame_counts::FrameCounts for Core {
    type Mix = *mut Mix;
    fn main_mix(&mut self) -> Option<*mut Mix> {
        // SAFETY: the initialized core; its main canvas is NULL or live.
        NonNull::new(unsafe { obs_get_video_frame_counts_main_mix() }).map(NonNull::as_ptr)
    }
    fn lock(&mut self) {
        // SAFETY: the live core's mixes_mutex, not held by this thread (the
        // core locks once and unlocks once).
        unsafe { obs_get_video_frame_counts_lock() }
    }
    fn unlock(&mut self) {
        // SAFETY: called once, after `lock`.
        unsafe { obs_get_video_frame_counts_unlock() }
    }
    fn count(&mut self) -> usize {
        // SAFETY: read under mixes_mutex, which `lock` holds.
        unsafe { obs_get_video_frame_counts_count() }
    }
    fn at(&mut self, index: usize) -> *mut Mix {
        // SAFETY: `index < count()` under the same hold of mixes_mutex.
        unsafe { obs_get_video_frame_counts_at(index) }
    }
    fn encoder_only(&mut self, mix: &*mut Mix) -> bool {
        // SAFETY: a listed mix is live while mixes_mutex is held.
        unsafe { obs_get_video_frame_counts_encoder_only(*mix) }
    }
    fn same_view(&mut self, mix: &*mut Mix, main: &*mut Mix) -> bool {
        // SAFETY: as in `encoder_only`; the main mix is live as in the C,
        // which read its view under the same hold.
        unsafe { obs_get_video_frame_counts_same_view(*mix, *main) }
    }
    fn has_video(&mut self, mix: &*mut Mix) -> bool {
        // SAFETY: as in `encoder_only`.
        unsafe { obs_get_video_frame_counts_has_video(*mix) }
    }
    fn total_frames(&mut self, mix: &*mut Mix) -> u32 {
        // SAFETY: as in `encoder_only`; asked only for a mix with a video.
        unsafe { obs_get_video_frame_counts_total(*mix) }
    }
    fn skipped_frames(&mut self, mix: &*mut Mix) -> u32 {
        // SAFETY: as in `total_frames`.
        unsafe { obs_get_video_frame_counts_skipped(*mix) }
    }
}

/// The summed total and skipped frame counts of the main mix and every
/// encoder-only mix of its view, each written only to a non-NULL output.
///
/// # Safety
/// As for the C function: the core is initialized, its `mixes_mutex` is not
/// held by this thread, its main canvas and every listed mix are live, and
/// `total` and `skipped` are each NULL or valid for one `uint32_t` write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_video_frame_counts(total: *mut u32, skipped: *mut u32) {
    let counts = crate::get_video_frame_counts::get_video_frame_counts(&mut Core);
    // The C wrote total, then skipped: when both name one `uint32_t`,
    // skipped is what stays, so only that output is written (no two `&mut`
    // to one place).
    let total = if total == skipped {
        None
    } else {
        // SAFETY: NULL or valid for a write, by the caller's contract, and
        // distinct from `skipped`.
        unsafe { total.as_mut() }
    };
    // SAFETY: as above.
    let skipped = unsafe { skipped.as_mut() };
    crate::get_video_frame_counts::write_counts(counts, total, skipped);
}
