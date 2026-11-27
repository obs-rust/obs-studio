//! C ABI of `obs_source_process_filter_begin()` and
//! `obs_source_process_filter_begin_with_color_space()` (`obs.h`, `EXPORT`,
//! in `libobs/cmake/rust-exports.d`). Every read, write and draw stays
//! behind the adapters of `obs-source-process-filter-begin-fields.c.h`.
use crate::process_filter_begin::{
    FilterBegin, process_filter_begin, process_filter_begin_with_color_space,
};
use core::ffi::{c_char, c_int, c_void};
use core::ptr::NonNull;

unsafe extern "C" {
    fn obs_source_process_filter_begin_clear_bypass(filter: *mut c_void);
    fn obs_source_process_filter_begin_get_target(filter: *mut c_void) -> *mut c_void;
    fn obs_source_process_filter_begin_get_parent(filter: *mut c_void) -> *mut c_void;
    fn obs_source_process_filter_begin_name(filter: *const c_void) -> *const c_char;
    fn obs_source_process_filter_begin_flags(source: *const c_void) -> u32;
    fn obs_source_process_filter_begin_base_width(target: *mut c_void) -> u32;
    fn obs_source_process_filter_begin_base_height(target: *mut c_void) -> u32;
    fn obs_source_process_filter_begin_set_allow_direct(filter: *mut c_void, allow_direct: c_int);
    fn obs_source_process_filter_begin_color_space() -> c_int;
    fn obs_source_process_filter_begin_set_bypass(filter: *mut c_void);
    fn obs_source_process_filter_begin_skip(filter: *mut c_void);
    fn obs_source_process_filter_begin_has_texrender(filter: *const c_void) -> bool;
    fn obs_source_process_filter_begin_texrender_format(filter: *const c_void) -> c_int;
    fn obs_source_process_filter_begin_destroy_texrender(filter: *mut c_void);
    fn obs_source_process_filter_begin_create_texrender(filter: *mut c_void, format: c_int);
    fn obs_source_process_filter_begin_texrender_begin(
        filter: *mut c_void,
        cx: u32,
        cy: u32,
        space: c_int,
    ) -> bool;
    fn obs_source_process_filter_begin_draw_setup(cx: c_int, cy: c_int);
    fn obs_source_process_filter_begin_default_render(target: *mut c_void);
    fn obs_source_process_filter_begin_video_render(target: *mut c_void);
    fn obs_source_process_filter_begin_draw_finish(filter: *mut c_void);
    fn blog(level: c_int, format: *const c_char, ...);
}

/// `LOG_INFO`.
const LOG_INFO: c_int = 300;
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

impl FilterBegin for Libobs {
    type Source = Source;

    fn log_null_filter(&mut self) {
        // SAFETY: static strings match the two %s arguments of the format.
        unsafe {
            blog(
                LOG_DEBUG,
                c"%s: Null '%s' parameter".as_ptr(),
                c"obs_source_process_filter_begin_with_color_space".as_ptr(),
                c"filter".as_ptr(),
            );
        }
    }
    fn clear_bypass(&mut self, filter: Source) {
        // SAFETY: the caller's live filter.
        unsafe { obs_source_process_filter_begin_clear_bypass(filter.ptr()) }
    }
    fn target(&mut self, filter: Source) -> Option<Source> {
        // SAFETY: the caller's live filter.
        Source::new(unsafe { obs_source_process_filter_begin_get_target(filter.ptr()) })
    }
    fn parent(&mut self, filter: Source) -> Option<Source> {
        // SAFETY: the caller's live filter.
        Source::new(unsafe { obs_source_process_filter_begin_get_parent(filter.ptr()) })
    }
    fn log_no_target(&mut self, filter: Source) {
        // SAFETY: the caller's live filter; its name is what the original
        // passed to the same format.
        unsafe {
            blog(
                LOG_INFO,
                c"filter '%s' being processed with no target!".as_ptr(),
                obs_source_process_filter_begin_name(filter.ptr()),
            );
        }
    }
    fn log_no_parent(&mut self, filter: Source) {
        // SAFETY: as log_no_target.
        unsafe {
            blog(
                LOG_INFO,
                c"filter '%s' being processed with no parent!".as_ptr(),
                obs_source_process_filter_begin_name(filter.ptr()),
            );
        }
    }
    fn output_flags(&mut self, source: Source) -> u32 {
        // SAFETY: the filter or its live parent.
        unsafe { obs_source_process_filter_begin_flags(source.ptr()) }
    }
    fn base_width(&mut self, target: Source) -> u32 {
        // SAFETY: the filter's live target.
        unsafe { obs_source_process_filter_begin_base_width(target.ptr()) }
    }
    fn base_height(&mut self, target: Source) -> u32 {
        // SAFETY: the filter's live target.
        unsafe { obs_source_process_filter_begin_base_height(target.ptr()) }
    }
    fn set_allow_direct(&mut self, filter: Source, allow_direct: i32) {
        // SAFETY: the caller's live filter.
        unsafe { obs_source_process_filter_begin_set_allow_direct(filter.ptr(), allow_direct) }
    }
    fn color_space(&mut self) -> i32 {
        // SAFETY: on the graphics thread, where the filter draws.
        unsafe { obs_source_process_filter_begin_color_space() }
    }
    fn set_bypass(&mut self, filter: Source) {
        // SAFETY: the caller's live filter.
        unsafe { obs_source_process_filter_begin_set_bypass(filter.ptr()) }
    }
    fn skip_video_filter(&mut self, filter: Source) {
        // SAFETY: the caller's live filter, on the graphics thread.
        unsafe { obs_source_process_filter_begin_skip(filter.ptr()) }
    }
    fn has_texrender(&mut self, filter: Source) -> bool {
        // SAFETY: the caller's live filter.
        unsafe { obs_source_process_filter_begin_has_texrender(filter.ptr()) }
    }
    fn texrender_format(&mut self, filter: Source) -> i32 {
        // SAFETY: the caller's live filter, which has a texrender.
        unsafe { obs_source_process_filter_begin_texrender_format(filter.ptr()) }
    }
    fn destroy_texrender(&mut self, filter: Source) {
        // SAFETY: the caller's live filter, on the graphics thread.
        unsafe { obs_source_process_filter_begin_destroy_texrender(filter.ptr()) }
    }
    fn create_texrender(&mut self, filter: Source, format: i32) {
        // SAFETY: the caller's live filter, on the graphics thread.
        unsafe { obs_source_process_filter_begin_create_texrender(filter.ptr(), format) }
    }
    fn texrender_begin(&mut self, filter: Source, cx: u32, cy: u32, space: i32) -> bool {
        // SAFETY: the caller's live filter, on the graphics thread.
        unsafe { obs_source_process_filter_begin_texrender_begin(filter.ptr(), cx, cy, space) }
    }
    fn draw_setup(&mut self, cx: i32, cy: i32) {
        // SAFETY: on the graphics thread, inside the texrender begun.
        unsafe { obs_source_process_filter_begin_draw_setup(cx, cy) }
    }
    fn default_render(&mut self, target: Source) {
        // SAFETY: the filter's live target, on the graphics thread.
        unsafe { obs_source_process_filter_begin_default_render(target.ptr()) }
    }
    fn video_render(&mut self, target: Source) {
        // SAFETY: the filter's live target, on the graphics thread.
        unsafe { obs_source_process_filter_begin_video_render(target.ptr()) }
    }
    fn draw_finish(&mut self, filter: Source) {
        // SAFETY: the caller's live filter, its texrender begun.
        unsafe { obs_source_process_filter_begin_draw_finish(filter.ptr()) }
    }
}

/// `obs_source_process_filter_begin_with_color_space()`.
///
/// # Safety
/// `filter` is NULL or a live filter; called where the original may be (a
/// filter's `video_render`, on the graphics thread).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_process_filter_begin_with_color_space(
    filter: *mut c_void,
    format: c_int,
    space: c_int,
    allow_direct: c_int,
) -> bool {
    process_filter_begin_with_color_space(
        &mut Libobs,
        Source::new(filter),
        format,
        space,
        allow_direct,
    )
}

/// `obs_source_process_filter_begin()`.
///
/// # Safety
/// As [`obs_source_process_filter_begin_with_color_space`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_process_filter_begin(
    filter: *mut c_void,
    format: c_int,
    allow_direct: c_int,
) -> bool {
    process_filter_begin(&mut Libobs, Source::new(filter), format, allow_direct)
}
