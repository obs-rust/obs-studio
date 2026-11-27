//! C ABI of `obs_source_skip_video_filter()` (`obs.h`, `EXPORT`, in
//! `libobs/cmake/rust-exports.d`). Every read and draw stays behind the
//! adapters of `obs-source-skip-video-filter-fields.c.h`.
use crate::skip_video_filter::{Skip, skip_video_filter};
use core::ffi::{c_char, c_int, c_void};
use core::ptr::NonNull;

unsafe extern "C" {
    fn obs_source_skip_video_filter_get_target(filter: *mut c_void) -> *mut c_void;
    fn obs_source_skip_video_filter_get_parent(filter: *mut c_void) -> *mut c_void;
    fn obs_source_skip_video_filter_parent_flags(parent: *const c_void) -> u32;
    fn obs_source_skip_video_filter_has_video_render(target: *const c_void) -> bool;
    fn obs_source_skip_video_filter_deinterlacing(target: *const c_void) -> bool;
    fn obs_source_skip_video_filter_draw_default(target: *mut c_void);
    fn obs_source_skip_video_filter_draw_main(target: *mut c_void);
    fn obs_source_skip_video_filter_draw_deinterlace(target: *mut c_void);
    fn obs_source_skip_video_filter_draw_async(target: *mut c_void);
    fn obs_source_skip_video_filter_draw_target(target: *mut c_void);
    fn blog(level: c_int, format: *const c_char, ...);
}

/// `LOG_DEBUG`.
const LOG_DEBUG: c_int = 400;

/// A non-NULL source as the C passes it; only the adapters dereference it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Source(NonNull<c_void>);

impl Source {
    /// A non-NULL C source.
    pub fn from_raw(ptr: NonNull<c_void>) -> Self {
        Self(ptr)
    }

    fn new(ptr: *mut c_void) -> Option<Self> {
        NonNull::new(ptr).map(Self)
    }

    fn ptr(self) -> *mut c_void {
        self.0.as_ptr()
    }
}

/// libobs.
#[derive(Clone, Copy, Debug, Default)]
pub struct Libobs;

impl Skip for Libobs {
    type Source = Source;

    fn log_null_filter(&mut self) {
        // SAFETY: static strings match the two %s arguments of the format.
        unsafe {
            blog(
                LOG_DEBUG,
                c"%s: Null '%s' parameter".as_ptr(),
                c"obs_source_skip_video_filter".as_ptr(),
                c"filter".as_ptr(),
            );
        }
    }
    fn target(&mut self, filter: Source) -> Option<Source> {
        // SAFETY: the caller's live filter.
        Source::new(unsafe { obs_source_skip_video_filter_get_target(filter.ptr()) })
    }
    fn parent(&mut self, filter: Source) -> Option<Source> {
        // SAFETY: the caller's live filter.
        Source::new(unsafe { obs_source_skip_video_filter_get_parent(filter.ptr()) })
    }
    fn output_flags(&mut self, parent: Source) -> u32 {
        // SAFETY: the filter's live parent.
        unsafe { obs_source_skip_video_filter_parent_flags(parent.ptr()) }
    }
    fn has_video_render(&mut self, target: Source) -> bool {
        // SAFETY: the filter's live target.
        unsafe { obs_source_skip_video_filter_has_video_render(target.ptr()) }
    }
    fn deinterlacing(&mut self, target: Source) -> bool {
        // SAFETY: the filter's live target.
        unsafe { obs_source_skip_video_filter_deinterlacing(target.ptr()) }
    }
    fn default_render(&mut self, target: Source) {
        // SAFETY: the filter's live target, on the graphics thread.
        unsafe { obs_source_skip_video_filter_draw_default(target.ptr()) }
    }
    fn main_render(&mut self, target: Source) {
        // SAFETY: the filter's live target, on the graphics thread.
        unsafe { obs_source_skip_video_filter_draw_main(target.ptr()) }
    }
    fn deinterlace_render(&mut self, target: Source) {
        // SAFETY: the filter's live target, on the graphics thread.
        unsafe { obs_source_skip_video_filter_draw_deinterlace(target.ptr()) }
    }
    fn async_video(&mut self, target: Source) {
        // SAFETY: the filter's live target, on the graphics thread.
        unsafe { obs_source_skip_video_filter_draw_async(target.ptr()) }
    }
    fn video_render(&mut self, target: Source) {
        // SAFETY: the filter's live target, on the graphics thread.
        unsafe { obs_source_skip_video_filter_draw_target(target.ptr()) }
    }
}

/// `obs_source_skip_video_filter()`.
///
/// # Safety
/// `filter` is NULL or a live source; called where the original may be
/// (a filter's `video_render`, on the graphics thread, when it draws).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_skip_video_filter(filter: *mut c_void) {
    skip_video_filter(&mut Libobs, Source::new(filter));
}
