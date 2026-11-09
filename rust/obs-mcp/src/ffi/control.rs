//! Private libobs media/procedure bindings; public C headers stay unchanged.
use super::{Output, Source, sys};
use crate::tools::control::{MediaState, MediaStatus, RenderStats, StageStats};
use obs_callback::ffi::calldata::calldata;
use std::ffi::{CStr, c_char, c_int, c_void};

unsafe extern "C" {
    fn obs_source_get_output_flags(source: *const sys::obs_source_t) -> u32;
    fn obs_source_media_restart(source: *mut sys::obs_source_t);
    fn obs_source_media_get_state(source: *mut sys::obs_source_t) -> c_int;
    fn obs_source_media_get_duration(source: *mut sys::obs_source_t) -> i64;
    fn obs_source_media_get_time(source: *mut sys::obs_source_t) -> i64;
    fn obs_get_total_frames() -> u32;
    fn obs_get_average_frame_time_ns() -> u64;
    fn obs_output_get_proc_handler(output: *const sys::obs_output_t) -> *mut c_void;
    fn proc_handler_call(handler: *mut c_void, name: *const c_char, params: *mut calldata) -> bool;
    fn calldata_get_data(
        data: *const calldata,
        name: *const c_char,
        out: *mut c_void,
        size: usize,
    ) -> bool;
    fn bfree(data: *mut c_void);
}
struct Params(calldata);
impl Params {
    fn new() -> Self {
        Self(calldata {
            stack: std::ptr::null_mut(),
            size: 0,
            capacity: 0,
            fixed: false,
        })
    }
    fn count(&self, name: &CStr) -> Option<u64> {
        let mut value = 0_i64;
        // SAFETY: live calldata, terminated key and correctly sized writable integer.
        let found = unsafe {
            calldata_get_data(
                &raw const self.0,
                name.as_ptr(),
                (&raw mut value).cast(),
                size_of::<i64>(),
            )
        };
        found.then(|| u64::try_from(value).ok()).flatten()
    }
    fn boolean(&self, name: &CStr) -> Option<bool> {
        let mut value = false;
        // SAFETY: live calldata, terminated key and correctly sized writable C bool.
        unsafe {
            calldata_get_data(
                &raw const self.0,
                name.as_ptr(),
                (&raw mut value).cast(),
                size_of::<bool>(),
            )
        }
        .then_some(value)
    }
}
impl Drop for Params {
    fn drop(&mut self) {
        // SAFETY: dynamic calldata storage is owned here and allocated by libobs.
        unsafe { bfree(self.0.stack.cast()) };
    }
}
impl Source {
    pub fn restart_media(&self) -> Result<(), String> {
        // SAFETY: owns a live source reference; the flag is OBS_SOURCE_CONTROLLABLE_MEDIA.
        if unsafe { obs_source_get_output_flags(self.0.as_ptr()) } & (1 << 13) == 0 {
            return Err(format!(
                "source '{}' is not controllable media",
                self.name()
            ));
        }
        // SAFETY: live controllable media source.
        unsafe { obs_source_media_restart(self.0.as_ptr()) };
        Ok(())
    }
    pub fn media(&self) -> MediaStatus {
        // SAFETY: all queries retain the live source reference throughout.
        let (state, duration, position) = unsafe {
            (
                obs_source_media_get_state(self.0.as_ptr()),
                obs_source_media_get_duration(self.0.as_ptr()),
                obs_source_media_get_time(self.0.as_ptr()),
            )
        };
        let state = match state {
            1 => MediaState::Playing,
            2 => MediaState::Opening,
            3 => MediaState::Buffering,
            4 => MediaState::Paused,
            5 => MediaState::Stopped,
            6 => MediaState::Ended,
            7 => MediaState::Error,
            _ => MediaState::None,
        };
        MediaStatus {
            name: self.name(),
            state,
            duration_ms: (duration > 0).then_some(duration),
            position_ms: (state != MediaState::None && position >= 0).then_some(position),
        }
    }
}
impl Output {
    fn procedure(&self, name: &CStr) -> Option<Params> {
        let mut params = Params::new();
        // SAFETY: the output reference pins its procedure handler and callback data;
        // params owns the returned calldata until all values have been copied.
        let found = unsafe {
            let handler = obs_output_get_proc_handler(self.0.as_ptr());
            !handler.is_null() && proc_handler_call(handler, name.as_ptr(), &raw mut params.0)
        };
        found.then_some(params)
    }
    pub fn virtual_audio(&self) -> Option<bool> {
        self.procedure(c"virtualcam_audio")?.boolean(c"active")
    }
    pub fn reset_device_stats(&self) {
        let _ = self.procedure(c"virtualcam_stats_reset");
    }
    pub fn device_stats(&self) -> Option<(StageStats, StageStats)> {
        let data = self.procedure(c"virtualcam_stats")?;
        let video = StageStats {
            live: self.active(),
            generation: data.count(c"generation"),
            latency_min_ns: data.count(c"vcam_min_ns"),
            latency_max_ns: data.count(c"vcam_max_ns"),
            frames_total: data.count(c"vcam_frames"),
            frames_dropped: data.count(c"vcam_dropped"),
            xruns: None,
        };
        let audio = StageStats {
            live: self.virtual_audio().unwrap_or(false),
            generation: data.count(c"generation"),
            latency_min_ns: data.count(c"vmic_min_ns"),
            latency_max_ns: data.count(c"vmic_max_ns"),
            frames_total: data.count(c"vmic_frames"),
            frames_dropped: data.count(c"vmic_dropped"),
            xruns: data.count(c"vmic_xruns"),
        };
        Some((video, audio))
    }
}
pub fn render_stats() -> RenderStats {
    // SAFETY: plain libobs statistics queries, valid even without configured video.
    unsafe {
        RenderStats {
            active_fps: sys::obs_get_active_fps(),
            average_frame_time_ns: obs_get_average_frame_time_ns(),
            render_total_frames: u64::from(obs_get_total_frames()),
            render_skipped_frames: u64::from(sys::obs_get_lagged_frames()),
        }
    }
}
