//! C ABI for `libobs/util/source-profiler.c.h`.
//!
//! The four `EXPORT` functions of `util/source-profiler.h` are exported.
//! The eleven hooks `obs.c`, `obs-video.c` and `obs-source.c` call are
//! declared in `obs-internal.h` without `EXPORT`, so their shims stay hidden
//! (`libobs/cmake/rust-hidden.d`). The C file's other non-static names
//! (`ucirclebuf_*`, `source_samples_create`, `hm_*`) are used by nothing
//! else and hidden by `-fvisibility=hidden`; they have no shim.
//!
//! The profiler is one process-wide [`SourceProfiler`], as the C's statics.
//! [`LibobsHost`] is libobs for the safe core: the graphics context through
//! `obs_enter_graphics` / `obs_leave_graphics` (which do what the C's
//! `gs_enter_context(obs->video.graphics)` does, and nothing when there is
//! no graphics, where the C logs two debug lines), the async flag through
//! `obs_source_get_output_flags`, and the rest through the functions the C
//! calls. Cargo tests link stand-ins for them
//! (`obs-c-oracle/oracle/source_profiler_host.c`).
//!
//! Intentional difference: a NULL `timer` out-pointer to
//! `source_profiler_source_render_begin` is not written (C dereferences it).

use core::ffi::{c_char, c_int, c_void};
use core::ptr;

use obs_util::ffi::bmem::{bfree, bmalloc};

use crate::source_profiler::{Fill, Handle, Host, ProfilerResult, RangeData, SourceProfiler};

/// `LOG_WARNING` in `util/base.h`.
const LOG_WARNING: c_int = 200;
/// `OBS_SOURCE_ASYNC_VIDEO` in `obs-source.h`.
const OBS_SOURCE_ASYNC_VIDEO: u32 = (1 << 2) | (1 << 0);
/// `OBS_TASK_GRAPHICS` in `obs.h`.
const OBS_TASK_GRAPHICS: c_int = 1;

/// The profiler scope name; profiler names are compared by pointer, so this
/// is one static string, as the C's.
static COLLECT_NAME: &core::ffi::CStr = c"source_profiler_frame_collect";
static DISJOINT_WARNING: &core::ffi::CStr = c"GPU Timers were disjoint, discarding samples.";

static PROFILER: SourceProfiler = SourceProfiler::new();

/// `profiler_result_t` (`util/source-profiler.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[allow(non_camel_case_types)]
pub struct profiler_result {
    pub tick_avg: u64,
    pub tick_max: u64,
    pub render_avg: u64,
    pub render_max: u64,
    pub render_gpu_avg: u64,
    pub render_gpu_max: u64,
    pub render_sum: u64,
    pub render_gpu_sum: u64,
    pub async_input: f64,
    pub async_rendered: f64,
    pub async_input_best: u64,
    pub async_input_worst: u64,
    pub async_rendered_best: u64,
    pub async_rendered_worst: u64,
}

impl From<ProfilerResult> for profiler_result {
    fn from(r: ProfilerResult) -> Self {
        Self {
            tick_avg: r.tick_avg,
            tick_max: r.tick_max,
            render_avg: r.render_avg,
            render_max: r.render_max,
            render_gpu_avg: r.render_gpu_avg,
            render_gpu_max: r.render_gpu_max,
            render_sum: r.render_sum,
            render_gpu_sum: r.render_gpu_sum,
            async_input: r.async_input,
            async_rendered: r.async_rendered,
            async_input_best: r.async_input_best,
            async_input_worst: r.async_input_worst,
            async_rendered_best: r.async_rendered_best,
            async_rendered_worst: r.async_rendered_worst,
        }
    }
}

/// The head of `struct obs_video_info` (`obs.h`): the profiler reads only
/// the frame rate. Never constructed here; read through the caller's
/// pointer.
#[repr(C)]
#[allow(non_camel_case_types)]
pub struct obs_video_info_head {
    pub graphics_module: *const c_char,
    pub fps_num: u32,
    pub fps_den: u32,
}

type ObsTask = unsafe extern "C" fn(param: *mut c_void);

unsafe extern "C" {
    fn blog(level: c_int, fmt: *const c_char, ...);
    fn os_gettime_ns() -> u64;
    fn profile_start(name: *const c_char);
    fn profile_end(name: *const c_char);
    fn obs_enter_graphics();
    fn obs_leave_graphics();
    fn obs_queue_task(ty: c_int, task: ObsTask, param: *mut c_void, wait: bool);
    fn obs_source_get_output_flags(source: *const c_void) -> u32;
    fn obs_source_get_last_async_ts(source: *const c_void) -> u64;
    fn gs_timer_create() -> *mut c_void;
    fn gs_timer_destroy(timer: *mut c_void);
    fn gs_timer_begin(timer: *mut c_void);
    fn gs_timer_end(timer: *mut c_void);
    fn gs_timer_get_data(timer: *mut c_void, ticks: *mut u64) -> bool;
    fn gs_timer_range_create() -> *mut c_void;
    fn gs_timer_range_destroy(range: *mut c_void);
    fn gs_timer_range_begin(range: *mut c_void);
    fn gs_timer_range_end(range: *mut c_void);
    fn gs_timer_range_get_data(
        range: *mut c_void,
        disjoint: *mut bool,
        frequency: *mut u64,
    ) -> bool;
}

fn handle<T>(p: *const T) -> Handle {
    p.expose_provenance()
}

fn pointer(h: Handle) -> *mut c_void {
    ptr::with_exposed_provenance_mut(h)
}

/// libobs, for the safe core. Every handle it is given came from libobs
/// (a source the caller passed, or a timer or range libobs created).
pub struct LibobsHost;

impl Host for LibobsHost {
    fn now_ns(&self) -> u64 {
        // SAFETY: no arguments.
        unsafe { os_gettime_ns() }
    }
    fn enter_graphics(&self) {
        // SAFETY: no arguments; balanced by leave_graphics.
        unsafe { obs_enter_graphics() }
    }
    fn leave_graphics(&self) {
        // SAFETY: no arguments.
        unsafe { obs_leave_graphics() }
    }
    fn timer_create(&self) -> Handle {
        // SAFETY: no arguments.
        handle(unsafe { gs_timer_create() })
    }
    fn timer_begin(&self, timer: Handle) {
        // SAFETY: a timer from gs_timer_create (or NULL, which it checks).
        unsafe { gs_timer_begin(pointer(timer)) }
    }
    fn timer_end(&self, timer: Handle) {
        // SAFETY: as timer_begin.
        unsafe { gs_timer_end(pointer(timer)) }
    }
    fn timer_get_data(&self, timer: Handle) -> Option<u64> {
        let mut ticks = 0u64;
        // SAFETY: a live timer and a local out-value.
        unsafe { gs_timer_get_data(pointer(timer), &raw mut ticks) }.then_some(ticks)
    }
    fn timer_destroy(&self, timer: Handle) {
        // SAFETY: a timer the profiler owns, destroyed once.
        unsafe { gs_timer_destroy(pointer(timer)) }
    }
    fn range_create(&self) -> Handle {
        // SAFETY: no arguments.
        handle(unsafe { gs_timer_range_create() })
    }
    fn range_begin(&self, range: Handle) {
        // SAFETY: a range from gs_timer_range_create (or NULL, which it checks).
        unsafe { gs_timer_range_begin(pointer(range)) }
    }
    fn range_end(&self, range: Handle) {
        // SAFETY: as range_begin.
        unsafe { gs_timer_range_end(pointer(range)) }
    }
    fn range_get_data(&self, range: Handle) -> RangeData {
        let mut disjoint = false;
        let mut frequency = 0u64;
        // SAFETY: a live range and local out-values.
        unsafe { gs_timer_range_get_data(pointer(range), &raw mut disjoint, &raw mut frequency) };
        RangeData {
            disjoint,
            frequency,
        }
    }
    fn range_destroy(&self, range: Handle) {
        // SAFETY: a range the profiler owns, destroyed once.
        unsafe { gs_timer_range_destroy(pointer(range)) }
    }
    fn is_async_video(&self, source: Handle) -> bool {
        // SAFETY: a source libobs ticked or passed in; it is live until its
        // removal task ran.
        let flags = unsafe { obs_source_get_output_flags(pointer(source)) };
        flags & OBS_SOURCE_ASYNC_VIDEO == OBS_SOURCE_ASYNC_VIDEO
    }
    fn last_async_ts(&self, source: Handle) -> u64 {
        // SAFETY: as is_async_video.
        unsafe { obs_source_get_last_async_ts(pointer(source)) }
    }
    fn warn_disjoint(&self) {
        // SAFETY: a static format with no conversions.
        unsafe { blog(LOG_WARNING, DISJOINT_WARNING.as_ptr()) }
    }
    fn collect_scope_begin(&self) {
        // SAFETY: a static name, as profile_start requires.
        unsafe { profile_start(COLLECT_NAME.as_ptr()) }
    }
    fn collect_scope_end(&self) {
        // SAFETY: the same static name.
        unsafe { profile_end(COLLECT_NAME.as_ptr()) }
    }
}

/// Same contract as C `source_profiler_enable`.
#[unsafe(no_mangle)]
pub extern "C" fn source_profiler_enable(enable: bool) {
    PROFILER.enable(enable);
}

/// Same contract as C `source_profiler_gpu_enable`.
#[unsafe(no_mangle)]
pub extern "C" fn source_profiler_gpu_enable(enable: bool) {
    PROFILER.gpu_enable(enable);
}

/// Same contract as C `source_profiler_reset_video`.
///
/// # Safety
///
/// `ovi` points to a `struct obs_video_info` with a nonzero `fps_den`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn source_profiler_reset_video(ovi: *const obs_video_info_head) {
    // SAFETY: the caller's obs_video_info; only its head is read.
    let (num, den) = unsafe { ((*ovi).fps_num, (*ovi).fps_den) };
    PROFILER.reset_video(num, den, &LibobsHost);
}

/// Same contract as C `source_profiler_render_begin`.
#[unsafe(no_mangle)]
pub extern "C" fn source_profiler_render_begin() {
    PROFILER.render_begin(&LibobsHost);
}

/// Same contract as C `source_profiler_render_end`.
#[unsafe(no_mangle)]
pub extern "C" fn source_profiler_render_end() {
    PROFILER.render_end(&LibobsHost);
}

/// Same contract as C `source_profiler_frame_begin`.
#[unsafe(no_mangle)]
pub extern "C" fn source_profiler_frame_begin() {
    PROFILER.frame_begin();
}

/// Same contract as C `source_profiler_frame_collect`.
#[unsafe(no_mangle)]
pub extern "C" fn source_profiler_frame_collect() {
    PROFILER.frame_collect(&LibobsHost);
}

/// Same contract as C `source_profiler_async_frame_received`.
#[unsafe(no_mangle)]
pub extern "C" fn source_profiler_async_frame_received(source: *mut c_void) {
    PROFILER.async_frame_received(handle(source), &LibobsHost);
}

/// Same contract as C `source_profiler_source_tick_start`.
#[unsafe(no_mangle)]
pub extern "C" fn source_profiler_source_tick_start() -> u64 {
    PROFILER.source_tick_start(&LibobsHost)
}

/// Same contract as C `source_profiler_source_tick_end`.
#[unsafe(no_mangle)]
pub extern "C" fn source_profiler_source_tick_end(source: *mut c_void, start: u64) {
    PROFILER.source_tick_end(handle(source), start, &LibobsHost);
}

/// Same contract as C `source_profiler_source_render_begin`: `*timer` is
/// written only while the profiler is enabled.
///
/// # Safety
///
/// `timer` is NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn source_profiler_source_render_begin(timer: *mut *mut c_void) -> u64 {
    let Some((start, created)) = PROFILER.source_render_begin(&LibobsHost) else {
        return 0;
    };
    if !timer.is_null() {
        // SAFETY: checked non-null; the caller's out-pointer.
        unsafe { *timer = pointer(created) };
    }
    start
}

/// Same contract as C `source_profiler_source_render_end`.
#[unsafe(no_mangle)]
pub extern "C" fn source_profiler_source_render_end(
    source: *mut c_void,
    start: u64,
    timer: *mut c_void,
) {
    PROFILER.source_render_end(handle(source), start, handle(timer), &LibobsHost);
}

/// `task_delete_source`, run on the graphics thread.
unsafe extern "C" fn task_delete_source(source: *mut c_void) {
    PROFILER.delete_source(handle(source), &LibobsHost);
}

/// Same contract as C `source_profiler_remove_source`: queues the deletion
/// on the graphics thread.
///
/// # Safety
///
/// Called with libobs running (its graphics task queue). `source` is only a
/// key.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn source_profiler_remove_source(source: *mut c_void) {
    if !PROFILER.should_queue_removal() {
        return;
    }
    // SAFETY: a static task; the source pointer is only a key to it.
    unsafe { obs_queue_task(OBS_TASK_GRAPHICS, task_delete_source, source, false) };
}

/// Same contract as C `source_profiler_fill_result`.
///
/// # Safety
///
/// `result` is NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn source_profiler_fill_result(
    source: *mut c_void,
    result: *mut profiler_result,
) -> bool {
    if result.is_null() {
        return false;
    }
    let (out, found) = match PROFILER.fill_result(handle(source), &LibobsHost) {
        Fill::Disabled => return false,
        Fill::Missing => (profiler_result::default(), false),
        Fill::Found(r) => (profiler_result::from(r), true),
    };
    // SAFETY: checked non-null; the caller's result.
    unsafe { result.write(out) };
    found
}

/// Same contract as C `source_profiler_get_result`: a `bmalloc`'d result,
/// or NULL. Allocated before the fill and freed on failure, as the C does.
///
/// # Safety
///
/// `source` is only a key; the caller frees the result with `bfree`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn source_profiler_get_result(source: *mut c_void) -> *mut profiler_result {
    // SAFETY: a plain allocation of the struct's size.
    let ret = unsafe { bmalloc(size_of::<profiler_result>()) }.cast::<profiler_result>();
    // SAFETY: ret is NULL (fill returns false) or a block of the struct's size.
    if !unsafe { source_profiler_fill_result(source, ret) } {
        // SAFETY: the block allocated above (bfree(NULL) is a no-op).
        unsafe { bfree(ret.cast()) };
        return ptr::null_mut();
    }
    ret
}
