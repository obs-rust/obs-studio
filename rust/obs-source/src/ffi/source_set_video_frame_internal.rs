//! C ABI for `libobs/obs-source-set-video-frame-internal.c.h`:
//! `obs_source_set_video_frame_internal()` (declared in `obs-internal.h`
//! without `EXPORT`, so the shim stays hidden: `rust-hidden.d`), the
//! graphics-thread worker every `obs_source_set_video_frame*()` entry ends
//! in.
//!
//! Runs the safe core ([`crate::set_video_frame_internal`]) over the C-owned
//! source and frame. The preload frame's shapes, destroy, create and copy,
//! `async_mutex` and the `last_frame_ts` write are the adapters of
//! `obs-source-preload-video-internal-fields.c.h` (#5837); the graphics
//! context, `set_async_texture_size()`, `update_async_textures()` and the
//! `async_textures_stale` store are those of
//! `obs-source-set-video-frame-internal-fields.c.h`. Each is the line of the
//! C it stands for, so the frame allocation, the copy, the texture upload
//! and the mutex stay the C's own. A NULL source logs the
//! `obs_source_valid()` line of the public entry,
//! `obs_source_set_video_frame`.
//!
//! No intentional differences from C.

use core::ffi::{CStr, c_char, c_int, c_void};

use crate::ffi::source_frame_copy::obs_source_frame_copy_shape;
use crate::frame_copy::Shape;
use crate::set_video_frame_internal::{self, Host};

unsafe extern "C" {
    fn obs_source_preload_internal_preload_shape(
        source: *const c_void,
        shape: *mut obs_source_frame_copy_shape,
    ) -> bool;
    fn obs_source_preload_internal_frame_shape(frame: *const c_void)
    -> obs_source_frame_copy_shape;
    fn obs_source_preload_internal_destroy_preload(source: *mut c_void);
    fn obs_source_preload_internal_create_preload(source: *mut c_void, frame: *const c_void);
    fn obs_source_preload_internal_copy(source: *mut c_void, frame: *const c_void);
    fn obs_source_preload_internal_lock_async(source: *mut c_void);
    fn obs_source_preload_internal_set_last_frame_ts(source: *mut c_void, frame: *const c_void);
    fn obs_source_preload_internal_unlock_async(source: *mut c_void);
    fn obs_source_set_frame_internal_enter_graphics();
    fn obs_source_set_frame_internal_set_texture_size(source: *mut c_void);
    fn obs_source_set_frame_internal_update_textures(source: *mut c_void);
    fn obs_source_set_frame_internal_mark_fresh(source: *mut c_void);
    fn obs_source_set_frame_internal_leave_graphics();
    fn blog(level: c_int, format: *const c_char, ...);
}

/// `LOG_DEBUG` (`util/base.h`).
const LOG_DEBUG: c_int = 400;

const fn shape(shape: obs_source_frame_copy_shape) -> Shape {
    Shape {
        format: shape.format,
        width: shape.width,
        height: shape.height,
    }
}

/// The live libobs objects the caller hands over.
struct Libobs;

// SAFETY (every method): `source` is the caller's live, non-null source and
// `frame` its live, non-null frame; the graphics and lock adapters pair as
// the core calls them.
impl Host for Libobs {
    type Source = *mut c_void;
    type Frame = *const c_void;
    fn null_source(&self, func: &'static CStr) {
        // SAFETY: static strings match the two %s arguments of the format.
        unsafe {
            blog(
                LOG_DEBUG,
                c"%s: Null '%s' parameter".as_ptr(),
                func.as_ptr(),
                c"source".as_ptr(),
            );
        }
    }
    fn enter_graphics(&self) {
        // SAFETY: no arguments; a no-op without a graphics subsystem.
        unsafe { obs_source_set_frame_internal_enter_graphics() }
    }
    fn preload_shape(&self, source: *mut c_void) -> Option<Shape> {
        let mut out = obs_source_frame_copy_shape::default();
        // SAFETY: a live, non-null source; `out` is a valid shape the
        // adapter writes only when it answers true.
        unsafe { obs_source_preload_internal_preload_shape(source, &raw mut out) }
            .then_some(shape(out))
    }
    fn frame_shape(&self, frame: *const c_void) -> Shape {
        // SAFETY: a live, non-null frame.
        shape(unsafe { obs_source_preload_internal_frame_shape(frame) })
    }
    fn destroy_preload(&self, source: *mut c_void) {
        // SAFETY: a live source; obs_source_frame_destroy() takes NULL.
        unsafe { obs_source_preload_internal_destroy_preload(source) }
    }
    fn create_preload(&self, source: *mut c_void, frame: *const c_void) {
        // SAFETY: a live source and frame.
        unsafe { obs_source_preload_internal_create_preload(source, frame) }
    }
    fn copy_to_preload(&self, source: *mut c_void, frame: *const c_void) {
        // SAFETY: a live source whose preload frame the core kept or just
        // created at the frame's shape, and a live frame.
        unsafe { obs_source_preload_internal_copy(source, frame) }
    }
    fn set_texture_size(&self, source: *mut c_void) {
        // SAFETY: a live source whose preload frame the copy just filled,
        // inside the graphics context.
        unsafe { obs_source_set_frame_internal_set_texture_size(source) }
    }
    fn update_textures(&self, source: *mut c_void) {
        // SAFETY: a live source whose textures were just sized to its
        // preload frame, inside the graphics context.
        unsafe { obs_source_set_frame_internal_update_textures(source) }
    }
    fn mark_textures_fresh(&self, source: *mut c_void) {
        // SAFETY: a live, non-null source.
        unsafe { obs_source_set_frame_internal_mark_fresh(source) }
    }
    fn lock_async(&self, source: *mut c_void) {
        // SAFETY: a live, non-null source.
        unsafe { obs_source_preload_internal_lock_async(source) }
    }
    fn set_last_frame_ts(&self, source: *mut c_void, frame: *const c_void) {
        // SAFETY: a live source with its async_mutex held, and a live frame.
        unsafe { obs_source_preload_internal_set_last_frame_ts(source, frame) }
    }
    fn unlock_async(&self, source: *mut c_void) {
        // SAFETY: a live source whose async_mutex this call locked.
        unsafe { obs_source_preload_internal_unlock_async(source) }
    }
    fn leave_graphics(&self) {
        // SAFETY: pairs with the enter this call made.
        unsafe { obs_source_set_frame_internal_leave_graphics() }
    }
}

/// `obs_source_set_video_frame_internal(source, frame)`.
///
/// # Safety
/// `source` is NULL or a live source; `frame` is NULL or a live frame.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_set_video_frame_internal(
    source: *mut c_void,
    frame: *const c_void,
) {
    set_video_frame_internal::set_video_frame_internal(
        &Libobs,
        (!source.is_null()).then_some(source),
        (!frame.is_null()).then_some(frame),
    );
}
