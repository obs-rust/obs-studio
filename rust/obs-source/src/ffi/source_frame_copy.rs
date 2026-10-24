//! C ABI for `libobs/obs-source-frame-copy.c.h`, the async frame copy plan
//! and cache decisions, declared in `libobs/obs-source-frame-copy.h`
//! without `EXPORT`.
//!
//! `obs-source.c.h` (still C) calls these with plain values it reads from
//! the frames and the source, and acts on the answers itself:
//! `copy_frame_data()` runs the [`obs_source_frame_copy_plan`] it gets with
//! its own `memcpy()`s; `cache_video()` (under `async_mutex`) clears the
//! cache or refuses the frame per [`obs_source_frame_copy_cache_step`],
//! with the two convert types it got from `get_convert_type()`;
//! `clean_cache()` stores the count and destroys and erases an entry per
//! [`obs_source_frame_copy_cache_clean`]; the `obs_source_output_video()`,
//! `_preload_video()` and `_set_video_frame()` families store
//! [`obs_source_frame_copy_full_range`] or
//! [`obs_source_frame_copy_full_range2`]; `preload_frame_changed()` returns
//! [`obs_source_frame_copy_preload_changed`]. Everything crosses by value:
//! no pointer crosses, `struct obs_source` and `struct obs_source_frame` are
//! never mirrored, and the shims read and write no memory of the caller,
//! take no lock and call nothing in libobs. They stay hidden
//! (`rust-hidden.txt`).
//!
//! No intentional differences from C, except where the C is undefined:
//! `clean_cache()`'s `++unused_count` would overflow a `long` at
//! `LONG_MAX`; the shim wraps. No entry gets there: counts start at 0 and
//! an entry is erased at 5.

use std::ffi::{c_int, c_long};

use crate::frame_copy::{self as core, CacheAge, CacheStep, Key, PLANES, Shape};

/// `struct obs_source_frame_copy_lines` (`obs-source-frame-copy.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(non_camel_case_types)]
pub struct obs_source_frame_copy_lines {
    pub linesize: [u32; PLANES],
}

/// `struct obs_source_frame_plane_copy` (`obs-source-frame-copy.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(non_camel_case_types)]
pub struct obs_source_frame_plane_copy {
    pub whole: bool,
    /// `size_t`.
    pub bytes: usize,
    pub rows: u32,
    pub src_stride: u32,
    pub dst_stride: u32,
}

/// `struct obs_source_frame_copy_plan` (`obs-source-frame-copy.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(non_camel_case_types)]
pub struct obs_source_frame_copy_plan {
    pub color_range: bool,
    /// `size_t`: at most `OBS_SOURCE_FRAME_COPY_PLANES`.
    pub count: usize,
    pub planes: [obs_source_frame_plane_copy; PLANES],
}

/// `struct obs_source_frame_copy_key` (`obs-source-frame-copy.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(non_camel_case_types)]
pub struct obs_source_frame_copy_key {
    pub width: u32,
    pub height: u32,
    /// An `enum convert_type` value.
    pub convert_type: c_int,
}

/// `struct obs_source_frame_cache_age` (`obs-source-frame-copy.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(non_camel_case_types)]
pub struct obs_source_frame_cache_age {
    pub counted: bool,
    /// `long`.
    pub unused_count: c_long,
    pub drop: bool,
}

/// `struct obs_source_frame_copy_shape` (`obs-source-frame-copy.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(non_camel_case_types)]
pub struct obs_source_frame_copy_shape {
    /// An `enum video_format` value.
    pub format: c_int,
    pub width: u32,
    pub height: u32,
}

/// `enum obs_source_frame_cache_step`, as the C's enum is returned (an `int`
/// or `unsigned int`; every value fits both).
#[allow(non_camel_case_types)]
pub type obs_source_frame_cache_step = u32;
/// `OBS_SOURCE_FRAME_CACHE_KEEP`.
pub const OBS_SOURCE_FRAME_CACHE_KEEP: obs_source_frame_cache_step = 0;
/// `OBS_SOURCE_FRAME_CACHE_RESET`.
pub const OBS_SOURCE_FRAME_CACHE_RESET: obs_source_frame_cache_step = 1;
/// `OBS_SOURCE_FRAME_CACHE_OVERFLOW`.
pub const OBS_SOURCE_FRAME_CACHE_OVERFLOW: obs_source_frame_cache_step = 2;

impl From<core::PlaneCopy> for obs_source_frame_plane_copy {
    fn from(copy: core::PlaneCopy) -> Self {
        Self {
            whole: copy.whole,
            bytes: copy.bytes,
            rows: copy.rows,
            src_stride: copy.src_stride,
            dst_stride: copy.dst_stride,
        }
    }
}

impl From<core::CopyPlan> for obs_source_frame_copy_plan {
    fn from(plan: core::CopyPlan) -> Self {
        Self {
            color_range: plan.color_range,
            count: plan.count,
            planes: plan.planes.map(obs_source_frame_plane_copy::from),
        }
    }
}

const fn key(key: obs_source_frame_copy_key) -> Key {
    Key {
        width: key.width,
        height: key.height,
        convert_type: key.convert_type,
    }
}

const fn shape(shape: obs_source_frame_copy_shape) -> Shape {
    Shape {
        format: shape.format,
        width: shape.width,
        height: shape.height,
    }
}

/// `struct obs_source_frame_copy_plan obs_source_frame_copy_plan(int
/// format, bool full_range, uint32_t height, struct
/// obs_source_frame_copy_lines src, struct obs_source_frame_copy_lines
/// dst)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_frame_copy_plan(
    format: c_int,
    full_range: bool,
    height: u32,
    src: obs_source_frame_copy_lines,
    dst: obs_source_frame_copy_lines,
) -> obs_source_frame_copy_plan {
    core::copy_plan(format, full_range, height, src.linesize, dst.linesize).into()
}

/// `enum obs_source_frame_cache_step obs_source_frame_copy_cache_step(size_t
/// queued, struct obs_source_frame_copy_key cache, struct
/// obs_source_frame_copy_key frame)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_frame_copy_cache_step(
    queued: usize,
    cache: obs_source_frame_copy_key,
    frame: obs_source_frame_copy_key,
) -> obs_source_frame_cache_step {
    match core::cache_step(queued, key(cache), key(frame)) {
        CacheStep::Keep => OBS_SOURCE_FRAME_CACHE_KEEP,
        CacheStep::Reset => OBS_SOURCE_FRAME_CACHE_RESET,
        CacheStep::Overflow => OBS_SOURCE_FRAME_CACHE_OVERFLOW,
    }
}

/// `struct obs_source_frame_cache_age obs_source_frame_copy_cache_clean(bool
/// used, long unused_count)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_frame_copy_cache_clean(
    used: bool,
    unused_count: c_long,
) -> obs_source_frame_cache_age {
    match core::clean(used, unused_count) {
        CacheAge::InUse => obs_source_frame_cache_age {
            counted: false,
            unused_count,
            drop: false,
        },
        CacheAge::Counted(count) => obs_source_frame_cache_age {
            counted: true,
            unused_count: count,
            drop: false,
        },
        CacheAge::Drop(count) => obs_source_frame_cache_age {
            counted: true,
            unused_count: count,
            drop: true,
        },
    }
}

/// `bool obs_source_frame_copy_full_range(int format, bool full_range)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_frame_copy_full_range(format: c_int, full_range: bool) -> bool {
    core::full_range(format, full_range)
}

/// `bool obs_source_frame_copy_full_range2(int format, int range)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_frame_copy_full_range2(format: c_int, range: c_int) -> bool {
    core::full_range2(format, range)
}

/// `bool obs_source_frame_copy_preload_changed(bool has_preload, struct
/// obs_source_frame_copy_shape preload, struct obs_source_frame_copy_shape
/// in)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_frame_copy_preload_changed(
    has_preload: bool,
    preload: obs_source_frame_copy_shape,
    frame: obs_source_frame_copy_shape,
) -> bool {
    core::preload_changed(has_preload.then_some(shape(preload)), shape(frame))
}
