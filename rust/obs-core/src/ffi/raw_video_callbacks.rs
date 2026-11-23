//! C ABI for the raw video callback entry points. The main canvas's mix is
//! read through the existing `obs_get_video()` field adapter; libobs' hidden
//! `start_raw_video` / `stop_raw_video` stay C and own the registration, its
//! locking and the mix's `raw_active` count.
use crate::ffi::get_video::Fields;
use crate::raw_video_callbacks::{NOT_SET_UP, Video, add, add2, remove};
use core::ffi::{c_char, c_int, c_void};
pub use obs_media_io::ffi::video_io::video_output_callback_t as Callback;
pub use obs_media_io::ffi::video_io::video_scale_info as Conversion;

/// `LOG_WARNING`.
const LOG_WARNING: c_int = 200;

unsafe extern "C" {
    fn obs_get_video_fields() -> Fields;
    fn start_raw_video(
        video: *mut c_void,
        conversion: *const Conversion,
        frame_rate_divisor: u32,
        callback: Callback,
        param: *mut c_void,
    );
    fn stop_raw_video(video: *mut c_void, callback: Callback, param: *mut c_void);
    fn blog(level: c_int, format: *const c_char, ...);
}

/// External operations, replaceable by the original-C differential recorder.
pub struct Calls {
    pub fields: unsafe extern "C" fn() -> Fields,
    pub start: unsafe extern "C" fn(*mut c_void, *const Conversion, u32, Callback, *mut c_void),
    pub stop: unsafe extern "C" fn(*mut c_void, Callback, *mut c_void),
    pub log: unsafe extern "C" fn(c_int, *const c_char, ...),
}

struct Seam<'a>(&'a mut Calls);

impl Video for Seam<'_> {
    type Video = *mut c_void;
    type Conversion = *const Conversion;
    type Callback = Callback;
    type Param = *mut c_void;

    fn main_video(&mut self) -> Option<*mut c_void> {
        // SAFETY: the initialized core under the callers' contract; its main
        // canvas and that canvas's mix are NULL or live, each read once.
        unsafe {
            let fields = (self.0.fields)();
            if fields.canvas.is_null() {
                return None;
            }
            let mix = (fields.mix)(fields.canvas);
            if mix.is_null() {
                return None;
            }
            Some((fields.video)(mix))
        }
    }

    fn warn_not_set_up(&mut self) {
        // SAFETY: static NUL-terminated format with no conversions.
        unsafe { (self.0.log)(LOG_WARNING, NOT_SET_UP.as_ptr()) };
    }

    fn start(
        &mut self,
        video: *mut c_void,
        conversion: *const Conversion,
        frame_rate_divisor: u32,
        callback: Callback,
        param: *mut c_void,
    ) {
        // SAFETY: the live mix's video and the caller's unchanged arguments.
        unsafe { (self.0.start)(video, conversion, frame_rate_divisor, callback, param) };
    }

    fn stop(&mut self, video: *mut c_void, callback: Callback, param: *mut c_void) {
        // SAFETY: as in `start`.
        unsafe { (self.0.stop)(video, callback, param) };
    }
}

fn libobs() -> Calls {
    Calls {
        fields: obs_get_video_fields,
        start: start_raw_video,
        stop: stop_raw_video,
        log: blog,
    }
}

/// [`obs_add_raw_video_callback`] over supplied external operations.
///
/// # Safety
/// `fields` must describe an initialized core whose main canvas and mix are
/// NULL or live; `start` must accept that mix's video, the conversion (NULL or
/// valid) and the caller's callback/parameter pair.
pub unsafe fn add_with(
    conversion: *const Conversion,
    callback: Callback,
    param: *mut c_void,
    calls: &mut Calls,
) {
    add(&mut Seam(calls), conversion, callback, param);
}

/// [`obs_add_raw_video_callback2`] over supplied external operations.
///
/// # Safety
/// As [`add_with`].
pub unsafe fn add2_with(
    conversion: *const Conversion,
    frame_rate_divisor: u32,
    callback: Callback,
    param: *mut c_void,
    calls: &mut Calls,
) {
    add2(
        &mut Seam(calls),
        conversion,
        frame_rate_divisor,
        callback,
        param,
    );
}

/// [`obs_remove_raw_video_callback`] over supplied external operations.
///
/// # Safety
/// As [`add_with`], with `stop` in place of `start`.
pub unsafe fn remove_with(callback: Callback, param: *mut c_void, calls: &mut Calls) {
    remove(&mut Seam(calls), callback, param);
}

/// # Safety
/// OBS must be initialized. The conversion is NULL or valid for the call; the
/// callback and parameter follow `start_raw_video`'s lifetime and
/// synchronization contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_add_raw_video_callback(
    conversion: *const Conversion,
    callback: Callback,
    param: *mut c_void,
) {
    // SAFETY: the caller's contract is add_with's, over libobs' own calls.
    unsafe { add_with(conversion, callback, param, &mut libobs()) };
}

/// # Safety
/// As [`obs_add_raw_video_callback`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_add_raw_video_callback2(
    conversion: *const Conversion,
    frame_rate_divisor: u32,
    callback: Callback,
    param: *mut c_void,
) {
    // SAFETY: as in obs_add_raw_video_callback.
    unsafe {
        add2_with(
            conversion,
            frame_rate_divisor,
            callback,
            param,
            &mut libobs(),
        )
    };
}

/// # Safety
/// OBS must be initialized; the pair follows `stop_raw_video`'s contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_remove_raw_video_callback(callback: Callback, param: *mut c_void) {
    // SAFETY: as in obs_add_raw_video_callback.
    unsafe { remove_with(callback, param, &mut libobs()) };
}
