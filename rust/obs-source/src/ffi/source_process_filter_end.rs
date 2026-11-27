//! C ABI of `obs_source_process_filter_end()` and
//! `obs_source_process_filter_tech_end()` (`obs.h`, `EXPORT`, in
//! `libobs/cmake/rust-exports.d`). Every read, write and draw stays behind
//! the adapters of `obs-source-process-filter-end-fields.c.h`.
use crate::process_filter_end::{FilterEnd, process_filter_end, process_filter_tech_end};
use core::ffi::{c_char, c_int, c_void};
use core::ptr::NonNull;

unsafe extern "C" {
    fn obs_source_process_filter_end_bypass_active(filter: *const c_void) -> bool;
    fn obs_source_process_filter_end_clear_bypass(filter: *mut c_void);
    fn obs_source_process_filter_end_get_target(filter: *mut c_void) -> *mut c_void;
    fn obs_source_process_filter_end_get_parent(filter: *mut c_void) -> *mut c_void;
    fn obs_source_process_filter_end_filter_flags(filter: *const c_void) -> u32;
    fn obs_source_process_filter_end_set_linear_srgb(linear_srgb: bool) -> bool;
    fn obs_source_process_filter_end_draw_bypass(
        target: *mut c_void,
        effect: *mut c_void,
        tech_name: *const c_char,
    );
    fn obs_source_process_filter_end_texture(filter: *const c_void) -> *mut c_void;
    fn obs_source_process_filter_end_draw_texture(
        tex: *mut c_void,
        effect: *mut c_void,
        width: u32,
        height: u32,
        tech_name: *const c_char,
    );
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

/// A non-NULL technique name as the C passes it, or the shim's `"Draw"`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tech(NonNull<c_char>);

impl Tech {
    /// A non-NULL C name.
    pub fn from_raw(ptr: NonNull<c_char>) -> Self {
        Self(ptr)
    }

    fn new(ptr: *const c_char) -> Option<Self> {
        NonNull::new(ptr.cast_mut()).map(Self)
    }

    fn ptr(self) -> *const c_char {
        self.0.as_ptr()
    }
}

/// The caller's effect as the C passes it (may be NULL); only libobs
/// dereferences it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Effect(*mut c_void);

impl Effect {
    /// The C effect pointer.
    pub fn from_raw(ptr: *mut c_void) -> Self {
        Self(ptr)
    }
}

/// libobs.
#[derive(Clone, Copy, Debug, Default)]
pub struct Libobs;

impl FilterEnd for Libobs {
    type Source = Source;
    type Effect = Effect;
    type Tech = Tech;
    type Texture = NonNull<c_void>;

    fn log_null_filter(&mut self) {
        // SAFETY: static strings match the two %s arguments of the format.
        unsafe {
            blog(
                LOG_DEBUG,
                c"%s: Null '%s' parameter".as_ptr(),
                c"obs_source_process_filter_end".as_ptr(),
                c"filter".as_ptr(),
            );
        }
    }
    fn draw_tech(&self) -> Tech {
        Tech(NonNull::from(c"Draw").cast())
    }
    fn bypass_active(&mut self, filter: Source) -> bool {
        // SAFETY: the caller's live filter.
        unsafe { obs_source_process_filter_end_bypass_active(filter.ptr()) }
    }
    fn clear_bypass(&mut self, filter: Source) {
        // SAFETY: the caller's live filter.
        unsafe { obs_source_process_filter_end_clear_bypass(filter.ptr()) }
    }
    fn target(&mut self, filter: Source) -> Option<Source> {
        // SAFETY: the caller's live filter.
        Source::new(unsafe { obs_source_process_filter_end_get_target(filter.ptr()) })
    }
    fn parent(&mut self, filter: Source) -> Option<Source> {
        // SAFETY: the caller's live filter.
        Source::new(unsafe { obs_source_process_filter_end_get_parent(filter.ptr()) })
    }
    fn output_flags(&mut self, filter: Source) -> u32 {
        // SAFETY: the caller's live filter.
        unsafe { obs_source_process_filter_end_filter_flags(filter.ptr()) }
    }
    fn set_linear_srgb(&mut self, linear_srgb: bool) -> bool {
        // SAFETY: on the graphics thread, where the filter draws.
        unsafe { obs_source_process_filter_end_set_linear_srgb(linear_srgb) }
    }
    fn render_bypass(&mut self, target: Source, effect: Effect, tech: Tech) {
        // SAFETY: the filter's live target, the caller's effect and a
        // NUL-terminated name, on the graphics thread.
        unsafe { obs_source_process_filter_end_draw_bypass(target.ptr(), effect.0, tech.ptr()) }
    }
    fn texture(&mut self, filter: Source) -> Option<NonNull<c_void>> {
        // SAFETY: the caller's live filter, on the graphics thread.
        NonNull::new(unsafe { obs_source_process_filter_end_texture(filter.ptr()) })
    }
    fn render_tex(
        &mut self,
        texture: NonNull<c_void>,
        effect: Effect,
        width: u32,
        height: u32,
        tech: Tech,
    ) {
        // SAFETY: the filter's live texture, the caller's effect and a
        // NUL-terminated name, on the graphics thread.
        unsafe {
            obs_source_process_filter_end_draw_texture(
                texture.as_ptr(),
                effect.0,
                width,
                height,
                tech.ptr(),
            );
        }
    }
}

/// `obs_source_process_filter_tech_end()`.
///
/// # Safety
/// `filter` is NULL or a live filter, `tech_name` NULL or NUL-terminated;
/// called where the original may be (a filter's `video_render`, on the
/// graphics thread, after `obs_source_process_filter_begin()`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_process_filter_tech_end(
    filter: *mut c_void,
    effect: *mut c_void,
    width: u32,
    height: u32,
    tech_name: *const c_char,
) {
    process_filter_tech_end(
        &mut Libobs,
        Source::new(filter),
        Effect(effect),
        width,
        height,
        Tech::new(tech_name),
    );
}

/// `obs_source_process_filter_end()`.
///
/// # Safety
/// As [`obs_source_process_filter_tech_end`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_process_filter_end(
    filter: *mut c_void,
    effect: *mut c_void,
    width: u32,
    height: u32,
) {
    process_filter_end(
        &mut Libobs,
        Source::new(filter),
        Effect(effect),
        width,
        height,
    );
}
