//! Opaque private adapters; no output or encoder layout crosses the ABI.
use crate::output_size_getters::{self, Access, Axis};
use crate::video_checks::SizeRoute;
use core::ffi::{CStr, c_char, c_int, c_void};
unsafe extern "C" {
    fn obs_output_size_getters_flags(output: *const c_void) -> u32;
    fn obs_output_size_getters_scaled(output: *const c_void, height: bool) -> u32;
    fn obs_output_size_getters_encoder(output: *const c_void, idx: usize) -> *mut c_void;
    fn obs_output_size_getters_video(output: *const c_void) -> *mut c_void;
    fn obs_output_size_getters_name(output: *const c_void) -> *const c_char;
    fn video_output_get_width(video: *const c_void) -> u32;
    fn video_output_get_height(video: *const c_void) -> u32;
    fn blog(level: c_int, format: *const c_char, ...);
}
struct Output(*const c_void);
impl Access for Output {
    fn flags(&self) -> u32 {
        // SAFETY: the core checks NULL before requesting flags.
        unsafe { obs_output_size_getters_flags(self.0) }
    }
    fn scaled(&self, axis: Axis) -> u32 {
        // SAFETY: live output validated by the core.
        unsafe { obs_output_size_getters_scaled(self.0, axis == Axis::Height) }
    }
    fn has_encoder(&self, idx: usize) -> bool {
        // SAFETY: the core validated output and slot bounds.
        unsafe { !obs_output_size_getters_encoder(self.0, idx).is_null() }
    }
    fn read(&self, idx: usize, axis: Axis, source: SizeRoute) -> u32 {
        // SAFETY: the caller keeps output and borrowed media/encoders live;
        // the core validates output/index and requests only the selected source.
        unsafe {
            match (source, axis) {
                (SizeRoute::Encoder, Axis::Width) => {
                    super::encoder_dimensions::obs_encoder_get_width(
                        obs_output_size_getters_encoder(self.0, idx),
                    )
                }
                (SizeRoute::Encoder, Axis::Height) => {
                    super::encoder_dimensions::obs_encoder_get_height(
                        obs_output_size_getters_encoder(self.0, idx),
                    )
                }
                (SizeRoute::Video, Axis::Width) => {
                    video_output_get_width(obs_output_size_getters_video(self.0))
                }
                (SizeRoute::Video, Axis::Height) => {
                    video_output_get_height(obs_output_size_getters_video(self.0))
                }
                _ => 0,
            }
        }
    }
    fn log(&self, name: &'static CStr, null: bool) {
        // SAFETY: formats and function names are static C strings. Non-NULL
        // diagnostics borrow the live output's name through the private adapter.
        unsafe {
            if null {
                blog(400, c"%s: Null 'output' parameter".as_ptr(), name.as_ptr());
            } else {
                blog(
                    200,
                    c"Output '%s': Tried to use %s on a non-video output".as_ptr(),
                    obs_output_size_getters_name(self.0),
                    name.as_ptr(),
                );
            }
        }
    }
}
/// # Safety
/// Non-NULL output and its borrowed encoder/media must remain live and readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_get_width2(output: *const c_void, idx: usize) -> u32 {
    output_size_getters::size(!output.is_null(), Some(idx), Axis::Width, &Output(output))
}
/// # Safety
/// Non-NULL output and its borrowed encoder/media must remain live and readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_get_width(output: *const c_void) -> u32 {
    output_size_getters::size(!output.is_null(), None, Axis::Width, &Output(output))
}
/// # Safety
/// Non-NULL output and its borrowed encoder/media must remain live and readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_get_height2(output: *const c_void, idx: usize) -> u32 {
    output_size_getters::size(!output.is_null(), Some(idx), Axis::Height, &Output(output))
}
/// # Safety
/// Non-NULL output and its borrowed encoder/media must remain live and readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_get_height(output: *const c_void) -> u32 {
    output_size_getters::size(!output.is_null(), None, Axis::Height, &Output(output))
}
