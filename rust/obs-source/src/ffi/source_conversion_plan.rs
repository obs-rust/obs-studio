//! C ABI for `libobs/obs-source-conversion-plan.c.h`, the async video GPU
//! conversion plan, declared in `libobs/obs-source-conversion-plan.h`
//! without `EXPORT`.
//!
//! `obs-source.c.h` (still C) calls [`obs_source_conversion_plan_fill`]
//! with the frame's format, range, transfer and size and a plan on its
//! stack. `init_gpu_conversion()` copies the plan into the source's
//! `async_convert_width`, `async_convert_height`, `async_texture_formats`
//! and `async_channel_count`; `get_convert_type()`,
//! `select_conversion_technique()` and `need_linear_output()` read one field
//! each, and the asserts for a frame that needs no conversion stay there. So
//! `struct obs_source` is never mirrored, and the shim takes no lock, calls
//! nothing in libobs and allocates nothing. It stays hidden
//! (`rust-hidden.txt`).
//!
//! The technique is a pointer to a `'static` C string literal of the safe
//! core, or null: it is never allocated and never freed, as the C's string
//! literals are not.
//!
//! Intentional difference from C: a null `plan` is left alone (the C writes
//! through it). libobs always passes a plan on its stack.

use std::ffi::{CStr, c_char, c_int};
use std::ptr;

use crate::conversion_plan::{self as core, PLANES};

/// `struct obs_source_conversion_plan` (`obs-source-conversion-plan.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub struct obs_source_conversion_plan {
    /// An `enum obs_source_convert_type`.
    pub convert_type: c_int,
    pub channel_count: c_int,
    pub width: [u32; PLANES],
    pub height: [u32; PLANES],
    /// `enum gs_color_format` values.
    pub format: [c_int; PLANES],
    /// A C string literal, or null.
    pub technique: *const c_char,
    pub linear: bool,
}

impl From<core::Plan> for obs_source_conversion_plan {
    fn from(plan: core::Plan) -> Self {
        Self {
            convert_type: plan.convert_type.to_c(),
            // at most PLANES
            channel_count: c_int::try_from(plan.channel_count).unwrap_or(c_int::MAX),
            width: plan.planes.map(|p| p.width),
            height: plan.planes.map(|p| p.height),
            format: plan.planes.map(|p| p.format),
            technique: plan.technique.map_or(ptr::null(), CStr::as_ptr),
            linear: plan.linear,
        }
    }
}

/// `void obs_source_conversion_plan_fill(int format, bool full_range, int
/// trc, uint32_t width, uint32_t height, struct obs_source_conversion_plan
/// *plan)`.
///
/// # Safety
///
/// `plan` is null or points to a `struct obs_source_conversion_plan` that
/// may be written, and that nothing else reads or writes during the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_conversion_plan_fill(
    format: c_int,
    full_range: bool,
    trc: c_int,
    width: u32,
    height: u32,
    plan: *mut obs_source_conversion_plan,
) {
    if plan.is_null() {
        return;
    }
    let filled =
        obs_source_conversion_plan::from(core::plan(format, full_range, trc, width, height));
    // SAFETY: non-null and writable (caller contract); the write does not
    // read the old value, so an uninitialized plan is fine.
    unsafe { plan.write(filled) };
}
