//! C ABI for `obs_source_get_color_space()` (`obs.h`, EXPORT), the shell of
//! [`crate::get_color_space`] around the colour space routing.
//!
//! The source's fields are read through the private adapters of
//! `libobs/obs-source-get-color-space-fields.c.h` (live core-source group,
//! no EXPORT), so `struct obs_source` is never mirrored. The pick reads the
//! caller's list as [`super::source_color_space::obs_source_color_space_pick`]
//! does; the callback gets the caller's count and pointer untouched.

use std::ffi::c_void;
use std::os::raw::c_int;

use super::source_color_space::{obs_source_color_space_node, obs_source_color_space_pick};
use crate::color_space::Node;
use crate::get_color_space::{self as shell, Libobs};

unsafe extern "C" {
    fn obs_source_get_color_space_data_valid(source: *const c_void) -> bool;
    fn obs_source_get_color_space_node(source: *const c_void) -> obs_source_color_space_node;
    fn obs_source_get_color_space_filter_parent(source: *const c_void) -> *mut c_void;
    fn obs_source_get_color_space_filter_target(source: *const c_void) -> *mut c_void;
    fn obs_source_get_color_space_async_space(source: *const c_void) -> c_int;
    fn obs_source_get_color_space_callback(
        source: *mut c_void,
        count: usize,
        preferred_spaces: *const c_int,
        space: *mut c_int,
    ) -> bool;
}

/// The caller's `count` and `preferred_spaces`.
#[derive(Clone, Copy)]
struct Preferred {
    count: usize,
    spaces: *const c_int,
}

/// libobs itself. Every source handed to a method other than `data_valid`
/// passed `data_valid` (non-NULL, with data), and the caller keeps the
/// sources of the chain alive for the call, as the C required.
struct Live;

impl Libobs for Live {
    type Source = *mut c_void;
    type Preferred = Preferred;

    fn data_valid(&mut self, source: *mut c_void) -> bool {
        // SAFETY: the adapter checks NULL (logging it) before reading.
        unsafe { obs_source_get_color_space_data_valid(source) }
    }
    fn node(&mut self, source: *mut c_void) -> Node {
        // SAFETY: a valid live source (above).
        let node = unsafe { obs_source_get_color_space_node(source) };
        Node {
            source_type: node.source_type,
            output_flags: node.output_flags,
            has_data: node.has_data,
            enabled: node.enabled,
            has_filter_parent: node.has_filter_parent,
            has_filter_target: node.has_filter_target,
        }
    }
    fn filter_parent(&mut self, source: *mut c_void) -> *mut c_void {
        // SAFETY: a valid live source (above).
        unsafe { obs_source_get_color_space_filter_parent(source) }
    }
    fn filter_target(&mut self, source: *mut c_void) -> *mut c_void {
        // SAFETY: a valid live source (above).
        unsafe { obs_source_get_color_space_filter_target(source) }
    }
    fn async_space(&mut self, source: *mut c_void) -> i32 {
        // SAFETY: a valid live source (above).
        unsafe { obs_source_get_color_space_async_space(source) }
    }
    fn pick(&mut self, video_space: i32, preferred: Preferred) -> i32 {
        // SAFETY: the caller's list, as obs_source_get_color_space() got it.
        unsafe { obs_source_color_space_pick(video_space, preferred.count, preferred.spaces) }
    }
    fn callback(&mut self, source: *mut c_void, preferred: Preferred) -> Option<i32> {
        let mut space: c_int = shell::SRGB;
        // SAFETY: a valid live source (above); the caller's list is passed
        // on untouched, and `space` is a local the adapter may write.
        unsafe {
            obs_source_get_color_space_callback(
                source,
                preferred.count,
                preferred.spaces,
                &raw mut space,
            )
        }
        .then_some(space)
    }
}

/// `enum gs_color_space obs_source_get_color_space(obs_source_t *source,
/// size_t count, const enum gs_color_space *preferred_spaces)`.
///
/// # Safety
///
/// `source` is NULL or a live source whose filter parent/target chain stays
/// alive for the call; `preferred_spaces` points to `count` readable
/// `enum gs_color_space` values (or is NULL with `count` 0).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_get_color_space(
    source: *mut c_void,
    count: usize,
    preferred_spaces: *const c_int,
) -> c_int {
    shell::get(
        &mut Live,
        source,
        Preferred {
            count,
            spaces: preferred_spaces,
        },
    )
}
