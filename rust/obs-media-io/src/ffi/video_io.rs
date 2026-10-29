//! The structs of `libobs/media-io/video-io.h` at the C boundary, and its
//! `static inline` helpers for enum values as they cross it.
//!
//! The enum helpers are compiled into every C caller, so nothing is exported
//! for them. `video_output_*` at the bottom replaces `media-io/video-io.c`.
//! The functions below the structs are for Rust shims that receive the
//! enums as `c_int`s: they convert, call the safe core in
//! [`crate::video_io`], and give a value outside its enum the result the
//! C gives it. A format outside `enum video_format` is not YUV and is named
//! `"None"`, like `VIDEO_FORMAT_NONE`; a colorspace outside
//! `enum video_colorspace` is named `"Unknown"`; a range outside
//! `enum video_range_type` is not `VIDEO_RANGE_DEFAULT`, so
//! `resolve_video_range` returns it unchanged and it is named `"Partial"`.

use core::ffi::{c_char, c_int, c_long, c_void};
use core::ptr;
use core::sync::atomic::{AtomicPtr, Ordering};

use obs_util::base::{LOG_ERROR, LOG_INFO, LOG_WARNING};
use obs_util::util_uint64::mul_div64;

use super::video_frame::video_frame;
use crate::video_frame::VideoFrame;
use crate::video_io::{
    self, MAX_AV_PLANES, VideoFormat, video_colorspace_from_c, video_format_from_c,
    video_range_type_from_c, video_range_type_to_c,
};
use crate::video_output::{
    FrameView, OpenInfo, RawScale, Scaler, ScalerFail, VideoHost, VideoOutput,
};

/// Mirrors `struct video_data`.
#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy)]
pub struct video_data {
    pub data: [*mut u8; MAX_AV_PLANES],
    pub linesize: [u32; MAX_AV_PLANES],
    pub timestamp: u64,
}

/// Mirrors `struct video_output_info`. The enums are `c_int`s.
#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy)]
pub struct video_output_info {
    pub name: *const c_char,

    pub format: c_int,
    pub fps_num: u32,
    pub fps_den: u32,
    pub width: u32,
    pub height: u32,
    pub cache_size: usize,

    pub colorspace: c_int,
    pub range: c_int,
}

/// Mirrors `struct video_scale_info`. The enums are `c_int`s.
#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct video_scale_info {
    pub format: c_int,
    pub width: u32,
    pub height: u32,
    pub range: c_int,
    pub colorspace: c_int,
}

/// `format_is_yuv` for a C `enum video_format` value.
pub fn format_is_yuv(format: c_int) -> bool {
    video_format_from_c(format).is_some_and(video_io::format_is_yuv)
}

/// `get_video_format_name` for a C `enum video_format` value.
pub fn get_video_format_name(format: c_int) -> &'static str {
    video_io::video_format_name(video_format_from_c(format).unwrap_or(VideoFormat::None))
}

/// `get_video_colorspace_name` for a C `enum video_colorspace` value.
pub fn get_video_colorspace_name(cs: c_int) -> &'static str {
    video_colorspace_from_c(cs).map_or("Unknown", video_io::video_colorspace_name)
}

/// `resolve_video_range` for C `enum video_format` and
/// `enum video_range_type` values.
pub fn resolve_video_range(format: c_int, range: c_int) -> c_int {
    let format = video_format_from_c(format).unwrap_or(VideoFormat::None);
    video_range_type_from_c(range).map_or(range, |range| {
        video_range_type_to_c(video_io::resolve_video_range(format, range))
    })
}

/// `get_video_range_name` for C `enum video_format` and
/// `enum video_range_type` values.
pub fn get_video_range_name(format: c_int, range: c_int) -> &'static str {
    let format = video_format_from_c(format).unwrap_or(VideoFormat::None);
    video_range_type_from_c(range)
        .map_or("Partial", |range| video_io::video_range_name(format, range))
}

/// `VIDEO_OUTPUT_SUCCESS`.
pub const VIDEO_OUTPUT_SUCCESS: c_int = 0;
/// `VIDEO_OUTPUT_INVALIDPARAM`.
pub const VIDEO_OUTPUT_INVALIDPARAM: c_int = -1;
/// `VIDEO_OUTPUT_FAIL`.
pub const VIDEO_OUTPUT_FAIL: c_int = -2;

/// `void (*)(void *param, struct video_data *frame)`.
#[allow(non_camel_case_types)]
pub type video_output_callback_t =
    Option<unsafe extern "C" fn(param: *mut c_void, frame: *mut video_data)>;

const MAX_CACHE_SIZE: usize = 16;
const VIDEO_SCALER_SUCCESS: c_int = 0;
const VIDEO_SCALER_BAD_CONVERSION: c_int = -1;
const VIDEO_SCALE_FAST_BILINEAR: c_int = 2;

unsafe extern "C" {
    fn video_scaler_create(
        scaler: *mut *mut c_void,
        dst: *const video_scale_info,
        src: *const video_scale_info,
        scale_type: c_int,
    ) -> c_int;
    fn video_scaler_destroy(scaler: *mut c_void);
    fn video_scaler_scale(
        scaler: *mut c_void,
        output: *mut *mut u8,
        out_linesize: *const u32,
        input: *const *const u8,
        in_linesize: *const u32,
    ) -> bool;
    fn blog(log_level: c_int, format: *const c_char, ...);
    fn profile_store_name(store: *mut c_void, format: *const c_char, ...) -> *const c_char;
    fn obs_get_profiler_name_store() -> *mut c_void;
}

struct FfmpegScaler(*mut c_void);

// SAFETY: the scaler pointer moves with the input, which the input lock
// keeps on one thread at a time. `scale` and `Drop` do not run together.
unsafe impl Send for FfmpegScaler {}

impl Drop for FfmpegScaler {
    fn drop(&mut self) {
        // SAFETY: `video_scaler_destroy` no-ops on null, and this pointer
        // came from `video_scaler_create` or is null.
        unsafe { video_scaler_destroy(self.0) };
    }
}

impl Scaler for FfmpegScaler {
    fn scale(&mut self, src: &FrameView, dst: &mut VideoFrame) -> bool {
        let mut output = [ptr::null_mut(); MAX_AV_PLANES];
        let mut out_linesize = [0u32; MAX_AV_PLANES];
        for plane in 0..MAX_AV_PLANES {
            out_linesize[plane] = dst.linesize(plane);
            if let Some(bytes) = dst.plane_mut(plane) {
                output[plane] = bytes.as_mut_ptr();
            }
        }
        let input = src.data.map(|plane| plane as *const u8);
        // SAFETY: the scaler is live. The plane pointers address the source
        // cache and the destination frame for this call.
        unsafe {
            video_scaler_scale(
                self.0,
                output.as_mut_ptr(),
                out_linesize.as_ptr(),
                input.as_ptr(),
                src.linesize.as_ptr(),
            )
        }
    }
}

fn raw_scale(raw: &RawScale) -> video_scale_info {
    video_scale_info {
        format: raw.format,
        width: raw.width,
        height: raw.height,
        range: raw.range,
        colorspace: raw.colorspace,
    }
}

struct LibobsHost {
    output_name: *const c_char,
    profile_name: AtomicPtr<c_char>,
}

// SAFETY: `output_name` is immutable for the output's life. `profile_name`
// is atomic. The video thread and the API threads share the host.
unsafe impl Send for LibobsHost {}
unsafe impl Sync for LibobsHost {}

impl VideoHost for LibobsHost {
    fn thread_start(&self) {
        // SAFETY: the name is a literal. `profile_store_name` interns its
        // result; a null store returns null, and `profile_start` ignores a
        // null name while the profiler is off.
        unsafe {
            obs_util::ffi::threading::os_set_thread_name(c"video-io: video thread".as_ptr());
            let stored = profile_store_name(
                obs_get_profiler_name_store(),
                c"video_thread(%s)".as_ptr(),
                self.output_name,
            );
            self.profile_name
                .store(stored as *mut c_char, Ordering::Relaxed);
        }
    }

    fn profile_begin(&self) {
        let name = self.profile_name.load(Ordering::Relaxed);
        // SAFETY: the name is null or an interned profiler string.
        unsafe { obs_util::ffi::profiler::profile_start(name) };
    }

    fn profile_end(&self) {
        let name = self.profile_name.load(Ordering::Relaxed);
        // SAFETY: same name `profile_begin` passed.
        unsafe { obs_util::ffi::profiler::profile_end(name) };
    }

    fn reenable(&self) {
        // SAFETY: no pointers.
        unsafe { obs_util::ffi::profiler::profile_reenable_thread() };
    }

    fn deliver(&self, callback: usize, param: usize, frame: &FrameView) {
        let cb = unsafe {
            std::mem::transmute::<usize, unsafe extern "C" fn(*mut c_void, *mut video_data)>(
                callback,
            )
        };
        let mut data = video_data {
            data: frame.data,
            linesize: frame.linesize,
            timestamp: frame.timestamp,
        };
        // SAFETY: `callback` is the function pointer `connect` stored.
        // `param` is the caller's context. `data` is valid for the call.
        unsafe { cb(param as *mut c_void, &mut data) };
    }

    fn warn_scale_failed(&self) {
        // SAFETY: the format string is a literal.
        unsafe { blog(LOG_WARNING, c"video-io: Could not scale frame!".as_ptr()) };
    }

    fn scaler_error(&self, bad_conversion: bool) {
        let message = if bad_conversion {
            c"video_input_init: Bad scale conversion type"
        } else {
            c"video_input_init: Failed to create scaler"
        };
        // SAFETY: the format string is a literal.
        unsafe { blog(LOG_ERROR, message.as_ptr()) };
    }

    fn log_skipped(&self, skipped: i64, total: i64) {
        if skipped == 0 {
            return;
        }
        let pct = (skipped as f64) / (total as f64) * 100.0;
        // SAFETY: the format string is a literal. `%ld` is `c_long`.
        unsafe {
            blog(
                LOG_INFO,
                c"Video stopped, number of skipped frames due to encoding lag: %ld/%ld (%0.1f%%)"
                    .as_ptr(),
                skipped as c_long,
                total as c_long,
                pct,
            );
        }
    }

    fn create_scaler(&self, dst: &RawScale, src: &RawScale) -> Result<Box<dyn Scaler>, ScalerFail> {
        let dst_info = raw_scale(dst);
        let src_info = raw_scale(src);
        let mut scaler = ptr::null_mut();
        // SAFETY: both info pointers are live stack structs. `scaler` is a
        // writable out-pointer.
        let ret = unsafe {
            video_scaler_create(&mut scaler, &dst_info, &src_info, VIDEO_SCALE_FAST_BILINEAR)
        };
        if ret == VIDEO_SCALER_SUCCESS && !scaler.is_null() {
            Ok(Box::new(FfmpegScaler(scaler)))
        } else if ret == VIDEO_SCALER_BAD_CONVERSION {
            Err(ScalerFail::BadConversion)
        } else {
            Err(ScalerFail::Failed)
        }
    }
}

struct Node {
    info: video_output_info,
    frame_time: u64,
    parent: *mut Node,
    output: Option<VideoOutput>,
}

// SAFETY: `info`, `frame_time`, and `parent` do not change after the node
// is published. `VideoOutput` synchronizes its own fields. A child follows
// `parent` and does not free it.
unsafe impl Send for Node {}
unsafe impl Sync for Node {}

fn root_mut(mut node: *mut Node) -> *mut Node {
    // SAFETY: `node` is a live root or a live child. The parent chain ends
    // at a root, as in C; a cycle would loop, also as in C.
    unsafe {
        while !(*node).parent.is_null() {
            node = (*node).parent;
        }
    }
    node
}

fn with_output<R>(video: *mut c_void, f: impl FnOnce(&VideoOutput) -> R) -> R {
    let root = root_mut(video as *mut Node);
    // SAFETY: a root published by `open` holds the output.
    let output = unsafe { (*root).output.as_ref().expect("root video output") };
    f(output)
}

fn callback_addr(callback: video_output_callback_t) -> usize {
    callback.map_or(0, |callback| callback as usize)
}

fn conversion_of(conversion: *const video_scale_info) -> Option<RawScale> {
    if conversion.is_null() {
        None
    } else {
        // SAFETY: the caller passed a live `video_scale_info`.
        let conversion = unsafe { &*conversion };
        Some(RawScale {
            format: conversion.format,
            width: conversion.width,
            height: conversion.height,
            range: conversion.range,
            colorspace: conversion.colorspace,
        })
    }
}

/// `video_output_open`. A null out-pointer or a null `info` returns
/// `VIDEO_OUTPUT_INVALIDPARAM` and writes nothing. C crashes on those.
///
/// # Safety
/// `video` is null or writable. `info` is null or a live `video_output_info`.
/// On success, `info.name` stays valid until close.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn video_output_open(
    video: *mut *mut c_void,
    info: *mut video_output_info,
) -> c_int {
    if video.is_null() || info.is_null() {
        return VIDEO_OUTPUT_INVALIDPARAM;
    }
    // SAFETY: `info` is non-null. The copy does not keep a reference to it.
    let src = unsafe { &*info };
    if src.width == 0 || src.height == 0 || src.fps_num == 0 || src.fps_den == 0 {
        return VIDEO_OUTPUT_INVALIDPARAM;
    }
    let mut stored = *src;
    if stored.cache_size > MAX_CACHE_SIZE {
        stored.cache_size = MAX_CACHE_SIZE;
    }
    let frame_time = mul_div64(
        1_000_000_000,
        u64::from(stored.fps_den),
        u64::from(stored.fps_num),
    );
    let host = Box::new(LibobsHost {
        output_name: stored.name,
        profile_name: AtomicPtr::new(ptr::null_mut()),
    });
    let opened = VideoOutput::open(
        OpenInfo {
            format: stored.format,
            width: stored.width,
            height: stored.height,
            cache_size: stored.cache_size,
            range: stored.range,
            colorspace: stored.colorspace,
        },
        frame_time,
        host,
    );
    match opened {
        Ok(output) => {
            let node = Box::new(Node {
                info: stored,
                frame_time,
                parent: ptr::null_mut(),
                output: Some(output),
            });
            // SAFETY: `video` was checked non-null.
            unsafe { *video = Box::into_raw(node).cast() };
            VIDEO_OUTPUT_SUCCESS
        }
        Err(()) => VIDEO_OUTPUT_FAIL,
    }
}

/// `video_output_close`. On a divisor child this stops the root and frees
/// only the child. C also frees the parent's cache and mutexes.
///
/// # Safety
/// `video` is null, a live root, or a live child. Do not call it from the
/// video thread: the join deadlocks, as `pthread_join` does in C.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn video_output_close(video: *mut c_void) {
    if video.is_null() {
        return;
    }
    let node = video.cast::<Node>();
    let root = root_mut(node);
    // SAFETY: `node` is the handle from open or create. Stop joins the
    // thread before the root is dropped. A child drop does not free the root.
    unsafe {
        if let Some(output) = (*root).output.as_ref() {
            output.stop();
        }
        drop(Box::from_raw(node));
    }
}

/// `video_output_connect`. A null `video` returns false. C crashes in
/// `get_root` before that check.
///
/// # Safety
/// `video` is null or a live handle. `conversion` is null or live.
/// `param` stays valid until disconnect or close.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn video_output_connect(
    video: *mut c_void,
    conversion: *const video_scale_info,
    callback: video_output_callback_t,
    param: *mut c_void,
) -> bool {
    // SAFETY: `connect2` has the same pointer contract with divisor 1.
    unsafe { video_output_connect2(video, conversion, 1, callback, param) }
}

/// `video_output_connect2`. See [`video_output_connect`].
///
/// # Safety
/// Same as [`video_output_connect`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn video_output_connect2(
    video: *mut c_void,
    conversion: *const video_scale_info,
    frame_rate_divisor: u32,
    callback: video_output_callback_t,
    param: *mut c_void,
) -> bool {
    if video.is_null() {
        return false;
    }
    let conversion = conversion_of(conversion);
    let callback = callback_addr(callback);
    with_output(video, |output| {
        output.connect(conversion, frame_rate_divisor, callback, param as usize)
    })
}

/// `video_output_disconnect`.
///
/// # Safety
/// `video` is null or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn video_output_disconnect(
    video: *mut c_void,
    callback: video_output_callback_t,
    param: *mut c_void,
) {
    // SAFETY: `disconnect2` has the same pointer contract.
    unsafe { video_output_disconnect2(video, callback, param) };
}

/// `video_output_disconnect2`.
///
/// # Safety
/// `video` is null or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn video_output_disconnect2(
    video: *mut c_void,
    callback: video_output_callback_t,
    param: *mut c_void,
) -> bool {
    if video.is_null() {
        return false;
    }
    let callback = callback_addr(callback);
    with_output(video, |output| output.disconnect(callback, param as usize))
}

/// `video_output_align_frame_rate_divisor`: the next frame this input receives
/// at or after `start_timestamp` is the first of a new divisor phase. Returns
/// false when the callback is not connected.
///
/// # Safety
/// `video` is null or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn video_output_align_frame_rate_divisor(
    video: *mut c_void,
    start_timestamp: u64,
    callback: video_output_callback_t,
    param: *mut c_void,
) -> bool {
    if video.is_null() {
        return false;
    }
    let callback = callback_addr(callback);
    with_output(video, |output| {
        output.align_divisor(start_timestamp, callback, param as usize)
    })
}

/// `video_output_active`.
///
/// # Safety
/// `video` is null or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn video_output_active(video: *const c_void) -> bool {
    if video.is_null() {
        return false;
    }
    with_output(video as *mut c_void, VideoOutput::active)
}

/// `video_output_get_info`. Returns this node's info, not the root's.
///
/// # Safety
/// `video` is null or a live handle. The returned pointer lives until
/// close or free of that same handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn video_output_get_info(video: *const c_void) -> *const video_output_info {
    if video.is_null() {
        return ptr::null();
    }
    // SAFETY: live handle. `info` is immutable after publish.
    unsafe { &(*video.cast::<Node>()).info }
}

/// `video_output_lock_frame`. A null `frame` still locks and returns true
/// when a slot is available; C would memcpy through the null pointer.
///
/// # Safety
/// `video` is null or a live handle. `frame` is null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn video_output_lock_frame(
    video: *mut c_void,
    frame: *mut video_frame,
    count: c_int,
    timestamp: u64,
) -> bool {
    if video.is_null() {
        return false;
    }
    let Some(view) = with_output(video, |output| output.lock_frame(count, timestamp)) else {
        return false;
    };
    if !frame.is_null() {
        // SAFETY: `frame` is writable. The pointers stay valid until unlock
        // advances the cache past this slot.
        unsafe {
            ptr::write(
                frame,
                video_frame {
                    data: view.data,
                    linesize: view.linesize,
                },
            );
        }
    }
    true
}

/// `video_output_unlock_frame`.
///
/// # Safety
/// `video` is null or a live handle. A successful lock should precede it.
/// Unlocking a cache of size 0 wraps `available`, as C's `size_t` does.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn video_output_unlock_frame(video: *mut c_void) {
    if video.is_null() {
        return;
    }
    with_output(video, VideoOutput::unlock_frame);
}

/// `video_output_get_frame_time`. This node's copy, not recomputed for a child.
///
/// # Safety
/// `video` is null or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn video_output_get_frame_time(video: *const c_void) -> u64 {
    if video.is_null() {
        return 0;
    }
    // SAFETY: live handle.
    unsafe { (*video.cast::<Node>()).frame_time }
}

/// `video_output_stop`.
///
/// # Safety
/// `video` is null or a live handle. Do not call it from the video thread.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn video_output_stop(video: *mut c_void) {
    if video.is_null() {
        return;
    }
    with_output(video, VideoOutput::stop);
}

/// `video_output_stopped`. A null handle is stopped, as in C.
///
/// # Safety
/// `video` is null or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn video_output_stopped(video: *mut c_void) -> bool {
    if video.is_null() {
        return true;
    }
    with_output(video, VideoOutput::stopped)
}

/// `video_output_get_format`. Reads the root.
///
/// # Safety
/// `video` is null or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn video_output_get_format(video: *const c_void) -> c_int {
    if video.is_null() {
        return VideoFormat::None as c_int;
    }
    // SAFETY: live handle. The root info does not change.
    unsafe { (*root_mut(video as *mut Node)).info.format }
}

/// `video_output_get_width`. Reads the root.
///
/// # Safety
/// `video` is null or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn video_output_get_width(video: *const c_void) -> u32 {
    if video.is_null() {
        return 0;
    }
    unsafe { (*root_mut(video as *mut Node)).info.width }
}

/// `video_output_get_height`. Reads the root.
///
/// # Safety
/// `video` is null or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn video_output_get_height(video: *const c_void) -> u32 {
    if video.is_null() {
        return 0;
    }
    unsafe { (*root_mut(video as *mut Node)).info.height }
}

/// `video_output_get_frame_rate`. Reads the root, so a child's multiplied
/// `fps_den` does not change it. A zero denominator is infinity, as in C.
///
/// # Safety
/// `video` is null or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn video_output_get_frame_rate(video: *const c_void) -> f64 {
    if video.is_null() {
        return 0.0;
    }
    // SAFETY: live handle.
    let info = unsafe { &(*root_mut(video as *mut Node)).info };
    f64::from(info.fps_num) / f64::from(info.fps_den)
}

/// `video_output_get_skipped_frames`. A null handle returns 0. C crashes.
///
/// # Safety
/// `video` is null or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn video_output_get_skipped_frames(video: *const c_void) -> u32 {
    if video.is_null() {
        return 0;
    }
    with_output(video as *mut c_void, VideoOutput::skipped_frames)
}

/// `video_output_get_total_frames`. A null handle returns 0. C crashes.
///
/// # Safety
/// `video` is null or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn video_output_get_total_frames(video: *const c_void) -> u32 {
    if video.is_null() {
        return 0;
    }
    with_output(video as *mut c_void, VideoOutput::total_frames)
}

/// `video_output_inc_texture_encoders`. A null handle does nothing. C crashes.
///
/// # Safety
/// `video` is null or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn video_output_inc_texture_encoders(video: *mut c_void) {
    if video.is_null() {
        return;
    }
    with_output(video, VideoOutput::inc_texture_encoders);
}

/// `video_output_dec_texture_encoders`. A null handle does nothing. C crashes.
///
/// # Safety
/// `video` is null or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn video_output_dec_texture_encoders(video: *mut c_void) {
    if video.is_null() {
        return;
    }
    with_output(video, VideoOutput::dec_texture_encoders);
}

/// `video_output_inc_texture_frames`. A null handle does nothing. C crashes.
///
/// # Safety
/// `video` is null or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn video_output_inc_texture_frames(video: *mut c_void) {
    if video.is_null() {
        return;
    }
    with_output(video, VideoOutput::inc_texture_frames);
}

/// `video_output_inc_texture_skipped_frames`. A null handle does nothing.
/// C crashes.
///
/// # Safety
/// `video` is null or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn video_output_inc_texture_skipped_frames(video: *mut c_void) {
    if video.is_null() {
        return;
    }
    with_output(video, VideoOutput::inc_texture_skipped_frames);
}

/// `video_output_create_with_frame_rate_divisor`. The parent pointer is the
/// argument, not the root. `fps_den` multiplies with `uint32_t` wrap.
/// `frame_time` is copied.
///
/// # Safety
/// `video` is null or a live handle. The result is freed with
/// `video_output_free_frame_rate_divisor`, not `video_output_close`, if the
/// caller wants the C free path. `close` on the child is safe here and is
/// not in C.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn video_output_create_with_frame_rate_divisor(
    video: *mut c_void,
    divisor: u32,
) -> *mut c_void {
    if video.is_null() || divisor == 0 || divisor == 1 {
        return ptr::null_mut();
    }
    let parent = video.cast::<Node>();
    // SAFETY: `parent` is a live handle. The copy does not alias its mutexes;
    // calls go to the root except `get_info` and `get_frame_time`.
    unsafe {
        let mut info = (*parent).info;
        info.fps_den = info.fps_den.wrapping_mul(divisor);
        let child = Box::new(Node {
            info,
            frame_time: (*parent).frame_time,
            parent,
            output: None,
        });
        Box::into_raw(child).cast()
    }
}

/// `video_output_free_frame_rate_divisor`. Null and a root are no-ops.
///
/// # Safety
/// `video` is null, a root, or a child from `create` that has not been freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn video_output_free_frame_rate_divisor(video: *mut c_void) {
    if video.is_null() {
        return;
    }
    let node = video.cast::<Node>();
    // SAFETY: a root has a null parent and is left alone. A child is the
    // box `create` returned.
    unsafe {
        if (*node).parent.is_null() {
            return;
        }
        drop(Box::from_raw(node));
    }
}
