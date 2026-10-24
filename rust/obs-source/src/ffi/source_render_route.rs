//! C ABI for `libobs/obs-source-render-route.c.h`, the source and filter
//! render routing, declared in `libobs/obs-source-render-route.h` without
//! `EXPORT`.
//!
//! `source_render_video()`, `obs_source_main_render()`, `can_bypass()`,
//! `obs_source_process_filter_begin_with_color_space()`,
//! `obs_source_process_filter_tech_end()` and
//! `obs_source_skip_video_filter()` in `obs-source.c.h` (still C) make
//! every `gs_*` and texrender call, take the refs, call the renders and
//! read every field in their order, pass the values in and act on what
//! comes back. So `struct obs_source` is never mirrored, and these shims
//! take plain values, touch no memory, take no lock and call nothing in
//! libobs. All of them stay hidden (`rust-hidden.txt`).
//!
//! The three enums come back as the C's enums do: their values are 0 to 11,
//! so they are `int` or `unsigned int` and every value fits both. A draw step
//! passed in that is outside the enum comes back unchanged, as the C's
//! `switch` lets it through.
//!
//! One pointer crosses: `obs_source_render_route_tech_name()` returns the
//! name it is given, or a static `"Draw"` for NULL. It never reads through
//! the pointer. `gs_effect_get_technique()` compares the name by its bytes,
//! so the static's address differing from the C literal's does not matter.
//!
//! No other intentional differences from C.

use std::ffi::CStr;
use std::os::raw::{c_char, c_int};

use crate::render_route::{self as core, TechName};

/// `enum obs_source_render_route_filter`.
#[allow(non_camel_case_types)]
pub type obs_source_render_route_filter = u32;

/// `enum obs_source_render_route_draw`.
#[allow(non_camel_case_types)]
pub type obs_source_render_route_draw = u32;

/// `enum obs_source_render_route_tech_end`.
#[allow(non_camel_case_types)]
pub type obs_source_render_route_tech_end = u32;

/// The technique `obs_source_process_filter_tech_end()` draws with when the
/// filter gives none.
pub const DRAW: &CStr = c"Draw";

/// `enum obs_source_render_route_filter obs_source_render_route_filter(bool
/// has_target, bool has_parent)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_render_route_filter(
    has_target: bool,
    has_parent: bool,
) -> obs_source_render_route_filter {
    core::filter_check(has_target, has_parent).value()
}

/// `bool obs_source_render_route_video_reads_flags(int type)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_render_route_video_reads_flags(source_type: c_int) -> bool {
    core::video_reads_flags(source_type)
}

/// `bool obs_source_render_route_video_none(int type, uint32_t flags)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_render_route_video_none(source_type: c_int, flags: u32) -> bool {
    core::video_none(source_type, flags)
}

/// `bool obs_source_render_route_async_reads_flags(int type)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_render_route_async_reads_flags(source_type: c_int) -> bool {
    core::async_reads_flags(source_type)
}

/// `bool obs_source_render_route_async_reads_rendering_filter(int type,
/// uint32_t flags)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_render_route_async_reads_rendering_filter(
    source_type: c_int,
    flags: u32,
) -> bool {
    core::async_reads_rendering_filter(source_type, flags)
}

/// `bool obs_source_render_route_async_update(int type, uint32_t flags, bool
/// rendering_filter)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_render_route_async_update(
    source_type: c_int,
    flags: u32,
    rendering_filter: bool,
) -> bool {
    core::async_update(source_type, flags, rendering_filter)
}

/// `bool obs_source_render_route_reads_enabled(bool has_data)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_render_route_reads_enabled(has_data: bool) -> bool {
    core::reads_enabled(has_data)
}

/// `bool obs_source_render_route_inactive(bool has_data, bool enabled)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_render_route_inactive(has_data: bool, enabled: bool) -> bool {
    core::inactive(has_data, enabled)
}

/// `enum obs_source_render_route_draw obs_source_render_route_draw(bool
/// has_filters)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_render_route_draw(has_filters: bool) -> obs_source_render_route_draw {
    core::draw(has_filters).value()
}

/// `enum obs_source_render_route_draw obs_source_render_route_draw_next(enum
/// obs_source_render_route_draw step, bool value)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_render_route_draw_next(
    step: obs_source_render_route_draw,
    value: bool,
) -> obs_source_render_route_draw {
    core::draw_next_value(step, value)
}

/// `bool obs_source_render_route_main_custom_draw(uint32_t flags)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_render_route_main_custom_draw(flags: u32) -> bool {
    core::main_custom_draw(flags)
}

/// `bool obs_source_render_route_main_srgb_aware(uint32_t flags)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_render_route_main_srgb_aware(flags: u32) -> bool {
    core::main_srgb_aware(flags)
}

/// `bool obs_source_render_route_main_reads_filters(bool has_filter_parent)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_render_route_main_reads_filters(has_filter_parent: bool) -> bool {
    core::main_reads_filters(has_filter_parent)
}

/// `bool obs_source_render_route_main_default_effect(uint32_t flags, bool
/// has_filter_parent, bool has_filters)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_render_route_main_default_effect(
    flags: u32,
    has_filter_parent: bool,
    has_filters: bool,
) -> bool {
    core::main_default_effect(flags, has_filter_parent, has_filters)
}

/// `bool obs_source_render_route_bypass_reads_space(bool target_is_parent,
/// int allow_direct, uint32_t filter_flags, uint32_t parent_flags)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_render_route_bypass_reads_space(
    target_is_parent: bool,
    allow_direct: c_int,
    filter_flags: u32,
    parent_flags: u32,
) -> bool {
    core::bypass_reads_space(target_is_parent, allow_direct, filter_flags, parent_flags)
}

/// `bool obs_source_render_route_bypass(bool target_is_parent, int
/// allow_direct, uint32_t filter_flags, uint32_t parent_flags, int space, int
/// current_space)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_render_route_bypass(
    target_is_parent: bool,
    allow_direct: c_int,
    filter_flags: u32,
    parent_flags: u32,
    space: c_int,
    current_space: c_int,
) -> bool {
    core::bypass(
        target_is_parent,
        allow_direct,
        filter_flags,
        parent_flags,
        space,
        current_space,
    )
}

/// `bool obs_source_render_route_size_skip(int cx, int cy)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_render_route_size_skip(cx: c_int, cy: c_int) -> bool {
    core::size_skip(cx, cy)
}

/// `bool obs_source_render_route_texrender_stale(int texrender_format, int
/// format)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_render_route_texrender_stale(
    texrender_format: c_int,
    format: c_int,
) -> bool {
    core::texrender_stale(texrender_format, format)
}

/// `enum obs_source_render_route_draw
/// obs_source_render_route_filter_render(bool target_is_parent, uint32_t
/// parent_flags)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_render_route_filter_render(
    target_is_parent: bool,
    parent_flags: u32,
) -> obs_source_render_route_draw {
    core::filter_render(target_is_parent, parent_flags).value()
}

/// `bool obs_source_render_route_tech_end_linear_srgb(uint32_t
/// filter_flags)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_render_route_tech_end_linear_srgb(filter_flags: u32) -> bool {
    core::tech_end_linear_srgb(filter_flags)
}

/// `const char *obs_source_render_route_tech_name(const char *tech_name)`:
/// `tech_name` itself, or [`DRAW`] for NULL. The pointer is compared with
/// NULL and passed back, never read.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_render_route_tech_name(tech_name: *const c_char) -> *const c_char {
    match core::tech_name(!tech_name.is_null()) {
        TechName::Given => tech_name,
        TechName::Draw => DRAW.as_ptr(),
    }
}

/// `enum obs_source_render_route_tech_end
/// obs_source_render_route_tech_end(bool bypass_active)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_render_route_tech_end(
    bypass_active: bool,
) -> obs_source_render_route_tech_end {
    core::tech_end(bypass_active).value()
}

/// `enum obs_source_render_route_draw obs_source_render_route_skip(bool
/// target_is_parent, uint32_t parent_flags)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_render_route_skip(
    target_is_parent: bool,
    parent_flags: u32,
) -> obs_source_render_route_draw {
    core::skip(target_is_parent, parent_flags).value()
}
