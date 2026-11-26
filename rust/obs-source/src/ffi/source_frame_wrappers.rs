//! C ABI for `libobs/obs-source-frame-wrappers.c.h`:
//! `obs_source_output_video()`, `obs_source_output_video2()`,
//! `obs_source_preload_video()`, `obs_source_preload_video2()`,
//! `obs_source_set_video_frame()` and `obs_source_set_video_frame2()`
//! (`EXPORT` in `obs.h`, `rust-exports.d`).
//!
//! Runs the safe core ([`crate::frame_wrappers`]) over the caller's source
//! and frame: `destroying()` and the three workers stay C, reached through
//! the adapters in `obs-source-frame-wrappers-fields.c.h`, each the call of
//! the C it stands for. The frame each worker gets is a local copy on this
//! stack frame, as in the C.
//!
//! No intentional differences from C.

use core::ffi::c_void;

use crate::frame_wrappers::{self, Frame, Frame2, Host};

unsafe extern "C" {
    fn obs_source_frame_wrappers_destroying(source: *mut c_void) -> bool;
    fn obs_source_frame_wrappers_output(source: *mut c_void, frame: *const Frame);
    fn obs_source_frame_wrappers_preload(source: *mut c_void, frame: *const Frame);
    fn obs_source_frame_wrappers_set(source: *mut c_void, frame: *const Frame);
}

/// The live libobs source the caller hands over.
struct Libobs;

fn ptr(frame: Option<&Frame>) -> *const Frame {
    frame.map_or(core::ptr::null(), core::ptr::from_ref)
}

impl Host for Libobs {
    type Source = *mut c_void;
    fn destroying(&self, source: *mut c_void) -> bool {
        // SAFETY: the caller's source, read as the C reads it (a NULL
        // source is the C's own NULL dereference).
        unsafe { obs_source_frame_wrappers_destroying(source) }
    }
    fn output(&self, source: *mut c_void, frame: Option<&Frame>) {
        // SAFETY: the caller's source; NULL or a frame on this stack.
        unsafe { obs_source_frame_wrappers_output(source, ptr(frame)) }
    }
    fn preload(&self, source: *mut c_void, frame: Option<&Frame>) {
        // SAFETY: the caller's source; NULL or a frame on this stack.
        unsafe { obs_source_frame_wrappers_preload(source, ptr(frame)) }
    }
    fn set(&self, source: *mut c_void, frame: Option<&Frame>) {
        // SAFETY: the caller's source; NULL or a frame on this stack.
        unsafe { obs_source_frame_wrappers_set(source, ptr(frame)) }
    }
}

/// `obs_source_output_video(source, frame)`.
///
/// # Safety
/// `source` is a live source; `frame` is NULL or a valid frame.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_output_video(source: *mut c_void, frame: *const Frame) {
    // SAFETY: NULL or the caller's valid frame, only read.
    let frame = unsafe { frame.as_ref() };
    frame_wrappers::output_video(&Libobs, source, frame);
}

/// `obs_source_output_video2(source, frame)`.
///
/// # Safety
/// `source` is a live source; `frame` is NULL or a valid frame.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_output_video2(source: *mut c_void, frame: *const Frame2) {
    // SAFETY: NULL or the caller's valid frame, only read.
    let frame = unsafe { frame.as_ref() };
    frame_wrappers::output_video2(&Libobs, source, frame);
}

/// `obs_source_preload_video(source, frame)`.
///
/// # Safety
/// `source` is a live source; `frame` is NULL or a valid frame.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_preload_video(source: *mut c_void, frame: *const Frame) {
    // SAFETY: NULL or the caller's valid frame, only read.
    let frame = unsafe { frame.as_ref() };
    frame_wrappers::preload_video(&Libobs, source, frame);
}

/// `obs_source_preload_video2(source, frame)`.
///
/// # Safety
/// `source` is a live source; `frame` is NULL or a valid frame.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_preload_video2(source: *mut c_void, frame: *const Frame2) {
    // SAFETY: NULL or the caller's valid frame, only read.
    let frame = unsafe { frame.as_ref() };
    frame_wrappers::preload_video2(&Libobs, source, frame);
}

/// `obs_source_set_video_frame(source, frame)`.
///
/// # Safety
/// `source` is a live source; `frame` is NULL or a valid frame.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_set_video_frame(source: *mut c_void, frame: *const Frame) {
    // SAFETY: NULL or the caller's valid frame, only read.
    let frame = unsafe { frame.as_ref() };
    frame_wrappers::set_video_frame(&Libobs, source, frame);
}

/// `obs_source_set_video_frame2(source, frame)`.
///
/// # Safety
/// `source` is a live source; `frame` is NULL or a valid frame.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_set_video_frame2(source: *mut c_void, frame: *const Frame2) {
    // SAFETY: NULL or the caller's valid frame, only read.
    let frame = unsafe { frame.as_ref() };
    frame_wrappers::set_video_frame2(&Libobs, source, frame);
}
